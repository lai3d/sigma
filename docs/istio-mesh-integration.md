# Istio Mesh Integration: VPS ↔ Xboard Traffic Visibility in Kiali

## Goal

VPS 上跑 XrayR，K8s（airport-prod EKS）里跑 Xboard。希望在 Kiali 上看到 **per-VPS → Xboard** 的流量拓扑，包括请求量、错误率、延迟。

## 方案评估结论

**WorkloadEntry 方案无法实现 per-VPS 流量区分。** 原因见下方详细分析。

当前 Kiali 已经能看到 `istio-ingressgateway → xboard → mysql/redis` 的完整内部拓扑，但外部 VPS 流量全部聚合在 ingressgateway 节点上，无法区分来自哪台 VPS。

## 尝试过的方案：K8s 侧 WorkloadEntry

### 思路

不在 VPS 上装任何 Istio 组件（零风险），仅在 K8s 侧创建 WorkloadEntry 记录每台 VPS 的公网 IP。期望 Xboard sidecar 通过源 IP 匹配 WorkloadEntry，让 Kiali 识别流量来源。

### 实际测试

在 airport-prod 集群创建了以下资源：

```yaml
# ServiceAccount
apiVersion: v1
kind: ServiceAccount
metadata:
  name: edge-vm
  namespace: airport-prod

# WorkloadGroup (per region)
apiVersion: networking.istio.io/v1
kind: WorkloadGroup
metadata:
  name: edge-nodes-jp
  namespace: airport-prod
spec:
  metadata:
    labels:
      app: edge-node
      region: jp
  template:
    serviceAccount: edge-vm
    network: external

# ServiceEntry (聚合所有 edge-node)
apiVersion: networking.istio.io/v1
kind: ServiceEntry
metadata:
  name: edge-nodes
  namespace: airport-prod
spec:
  hosts:
  - edge-nodes.airport-prod.mesh
  location: MESH_INTERNAL
  ports:
  - number: 8080       # 占位，VPS 只是流量源不是目的地
    name: http
    protocol: HTTP
  resolution: STATIC
  workloadSelector:
    labels:
      app: edge-node

# WorkloadEntry (per VPS)
apiVersion: networking.istio.io/v1
kind: WorkloadEntry
metadata:
  name: edge-jp-004
  namespace: airport-prod
  labels:
    app: edge-node
spec:
  address: <VPS_PUBLIC_IP>
  labels:
    app: edge-node
    region: jp
    node: jp-004
  serviceAccount: edge-vm
  network: external
```

### 结果

Kiali 中 `edge-nodes.airport-prod.mesh` 出现为一个孤立的 ServiceEntry 节点（三角形图标），**但没有任何流量边连接到它**。VPS 的流量仍然全部显示为 `istio-ingressgateway → xboard`。

### 根本原因：Ingress Gateway 遮蔽了源 IP

WorkloadEntry 的源 IP 匹配工作在 **TCP/网络层**。但 VPS 到 Xboard 的实际流量路径是：

```
VPS (XrayR)
  → AWS NLB (proxy protocol)
    → istio-ingressgateway pod
      → Xboard pod sidecar (入站)
        → Xboard app container
```

Xboard sidecar 看到的 TCP 源 IP 是 **ingressgateway 的 pod IP**（如 `10.x.x.x`），不是 VPS 的公网 IP。

虽然 NLB 配置了 proxy protocol、ingress gateway 配置了 `X-Forwarded-For` 头，真实 VPS IP 保留在了 **HTTP 头**里 — 但 Istio 做 WorkloadEntry 匹配用的是**网络层源 IP**，不是 HTTP 头。

```
TCP 源 IP:        10.x.x.x (ingressgateway pod)  ← Istio 用这个做匹配
HTTP X-Forwarded-For: <VPS_PUBLIC_IP>              ← 应用层可见，Istio 不用
```

所以 WorkloadEntry 永远匹配不到，Kiali 只能看到 `ingressgateway → xboard`。

### 为什么直连也不可行

要让 WorkloadEntry 匹配生效，VPS 需要**绕过 ingress gateway 直连 Xboard pod IP**。这要求：
- VPS 能路由到 K8s pod 网络（VPC peering + pod CIDR 路由）
- 或者走 east-west gateway + mTLS（需要 VPS 上装 istio-agent）

前者增加网络暴露面，后者回到了"VPS 装 Istio"的老路。

## 为什么不在 VPS 上装 Istio

| 风险 | 说明 |
|------|------|
| iptables 劫持用户流量 | istio-agent 默认配 iptables 拦截所有出入站流量，影响 VPN 用户连接 |
| Envoy 处理加密隧道协议 | Envoy 不认识 V2Ray/Trojan 等协议，会导致连接失败 |
| 性能开销 | 大量用户流量全过 Envoy 增加延迟 |
| 运维复杂度 | 月抛 VPS 上维护 istio-agent + Envoy + iptables 成本高 |

**结论**：VPS 上不装 Istio 组件，用户流量零风险。

## 当前状态：Kiali 已有的可见性

即使没有 per-VPS 区分，airport-prod 的 Kiali 已经能看到：

```
istio-ingressgateway → airport-xboard → rds-external-mysql
                     → airport-xboard → airport-xboard-redis-with-pv
                     → airport-xboard-subscribe-api → ...
                     → airport-xboard-lite-api → ...
```

K8s 内部的服务间调用链完整可见。

## 替代方案（待评估）

如果需要 per-VPS 维度的可观测性，可以考虑以下方向：

### 方案 A：应用层指标（推荐）

在 Xboard 或 Nginx ingress 层，利用 `X-Forwarded-For` 头导出 Prometheus 指标，按 VPS IP 分组。在 Grafana 中展示 per-VPS 请求量/错误率/延迟。

- 优点：不依赖 Istio mesh，纯应用层方案
- 缺点：不在 Kiali 拓扑图上，需要单独 Grafana dashboard

### 方案 B：VPS 侧 Prometheus 指标

sigma-agent 已有 `/metrics` endpoint。可以让 XrayR 或 sigma-agent 上报 API 调用指标（对 Xboard 的请求量/延迟/错误），由中心化 Prometheus 抓取。

- 优点：指标来自源头，最准确
- 缺点：需要 sigma-agent 改造，VPS 上增加指标采集

### 方案 C：Envoy access log 分析

利用 ingress gateway 的 access log，按 `X-Forwarded-For` 头提取 VPS IP，通过 Loki/Promtail 聚合。

- 优点：不改任何应用代码
- 缺点：基于日志而非指标，实时性和查询性能不如 Prometheus

## Infrastructure

```
┌─── central-platform EKS ──┐     ┌─── airport-prod EKS ──────────────┐
│ sigma-api                 │     │ xboard + Envoy sidecar             │
│ sigma-web                 │     │ xboard-subscribe-api               │
│ postgres, redis           │     │ xboard-lite-api                    │
│                           │     │ redis, mysql (RDS)                 │
│ (管理面)                   │     │                                    │
└───────────────────────────┘     │ istiod + Kiali + Prometheus        │
                                  │ istio-ingressgateway (NLB + PP)    │
         sigma-agent              │ istio-eastwestgateway              │
           register/heartbeat ──→ │                                    │
           (走 central-platform)  └────────────────────────────────────┘
                                             ▲
                                             │ HTTPS via NLB
                                             │ (ingress gateway 终结 TLS)
                                ┌────────────┼────────────┐
                                │            │            │
                           ┌────────┐  ┌────────┐  ┌────────┐
                           │ VPS JP │  │ VPS HK │  │ VPS US │
                           │ XrayR  │  │ XrayR  │  │ XrayR  │
                           │ sigma- │  │ sigma- │  │ sigma- │
                           │ agent  │  │ agent  │  │ agent  │
                           │        │  │        │  │        │
                           │ 无Istio │  │ 无Istio │  │ 无Istio │
                           └────────┘  └────────┘  └────────┘
```

## 流量路径

XrayR 每 60s 轮询 Xboard API（配置拉取、心跳、流量上报）：

```
XrayR (VPS)
  → HTTPS POST /api/v1/server/...
  → DNS 解析 → NLB (proxy protocol 保留真实 IP)
  → istio-ingressgateway (终结 TLS, 设置 X-Forwarded-For)
  → Xboard pod sidecar (入站, 只看到 gateway pod IP)
  → Xboard app container
  → response 原路返回
```

## 可见性对照

| 已有 (Kiali) | 缺失 | 可通过替代方案获得 |
|-------------|------|------------------|
| ingressgateway → xboard 请求量/错误/延迟 | per-VPS 流量区分 | 方案 A/B/C |
| xboard → mysql/redis 内部调用链 | VPS 出站侧指标 | 方案 B |
| K8s 内服务间拓扑 | VPS 之间的流量 | N/A |
| 整体错误率、延迟分布 | 单个 VPS 的错误/延迟 | 方案 A/B/C |
