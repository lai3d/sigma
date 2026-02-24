# Istio Mesh Integration: VPS ↔ Xboard Traffic Visibility in Kiali

## Goal

VPS 上跑 XrayR，K8s（airport-prod EKS）里跑 Xboard。在 Kiali 上看到 **XrayR → Xboard** 的流量拓扑，包括请求量、错误率、延迟。

**核心原则：VPS 上不装任何 Istio 组件**。用户 VPN 流量零风险。所有可观测性来自 K8s 侧 Xboard 的 Envoy sidecar。

## Infrastructure

```
┌─── central-platform EKS ──┐     ┌─── airport-prod EKS ──────────┐
│ sigma-api                 │     │ xboard + Envoy sidecar         │
│ sigma-web                 │     │ mysql / mariadb                │
│ postgres, redis           │     │ redis                          │
│                           │     │                                │
│ (管理面，不入 mesh)        │     │ istiod + Kiali + Prometheus    │
└───────────────────────────┘     │                                │
                                  │ WorkloadEntry (per VPS,        │
         sigma-agent              │   仅 IP 记录，VPS 上不装东西)   │
           register/heartbeat ──→ │                                │
           (走 central-platform)  └────────────────────────────────┘
                                             ▲
                                             │ HTTP 直连 (无 Envoy)
                                ┌────────────┼────────────┐
                                │            │            │
                           ┌────────┐  ┌────────┐  ┌────────┐
                           │ VPS JP │  │ VPS HK │  │ VPS US │
                           │        │  │        │  │        │
                           │ XrayR  │  │ XrayR  │  │ XrayR  │
                           │ sigma- │  │ sigma- │  │ sigma- │
                           │ agent  │  │ agent  │  │ agent  │
                           │        │  │        │  │        │
                           │ 无Envoy │  │ 无Envoy │  │ 无Envoy │
                           │无iptables│ │无iptables│ │无iptables│
                           └────────┘  └────────┘  └────────┘
```

## How It Works

XrayR 直连 Xboard（和现在一样），不经过任何代理。可观测性完全来自 **K8s 侧**：

1. Xboard pod 的 **Envoy sidecar** 看到所有入站请求的**源 IP**
2. 源 IP 匹配 **WorkloadEntry**（sigma-api 自动创建）→ Kiali 知道「这个请求来自 xrayr/vps-jp-01」
3. Kiali 画出 xrayr → xboard 的流量拓扑

```
XrayR (VPS, 103.x.x.x)
  → HTTP POST /api/v1/server/push
  → Xboard pod's Envoy sidecar (入站)
      → sidecar 记录: source_ip=103.x.x.x → 匹配 WorkloadEntry vps-jp-01
      → 指标上报 Prometheus: source_workload=vps-jp-01, app=xrayr
  → Xboard app container
  → response 原路返回

Prometheus → Kiali query → 画出 xrayr → xboard 的边 + 指标
```

### 为什么不在 VPS 上装 Istio

| 风险 | 说明 |
|------|------|
| iptables 劫持用户 VPN 流量 | istio-agent 默认配 iptables 拦截所有出入站流量，配错一点用户全断 |
| Envoy 处理 V2Ray/Trojan 加密隧道 | Envoy 不认识这些协议，会导致连接失败 |
| 性能开销 | 几万用户流量全过 Envoy 增加延迟 |
| 运维复杂度 | 月抛 VPS 上维护 istio-agent + Envoy + iptables 成本高 |

**结论**：只在 K8s 侧做，VPS 完全不动，零风险。

## What Kiali Shows

```
┌──────────────────────────────────────────────────────────────┐
│  Kiali Service Graph (namespace: airport-prod)                │
│                                                              │
│  ┌───────────────┐  HTTP /api/v1/*   ┌──────────────┐       │
│  │ xrayr         │─────────────────→ │ xboard       │       │
│  │ (30 VPS nodes)│  config/push/     │ (K8s pod)    │       │
│  │ JP,HK,US,DE  │  alive            └──────┬───────┘       │
│  └───────────────┘                         │ TCP            │
│                                            ▼                │
│                                    ┌──────────────┐         │
│                                    │ mysql        │         │
│                                    │ (K8s pod)    │         │
│                                    └──────────────┘         │
│                                                              │
│  Click xrayr → xboard edge:                                 │
│    Request rate: 120 req/min                                 │
│    Error rate: 0.2%                                          │
│    P99 latency: 45ms                                         │
│    Per-workload breakdown: JP-01: 4req/m, HK-03: 4req/m ... │
└──────────────────────────────────────────────────────────────┘
```

- XrayR 每个 VPS 节点是一个 workload，聚合在 `xrayr` service 下
- 点击边可以看每个节点的请求量/错误/延迟
- 节点掉线（停止发 heartbeat/push）→ Kiali 显示流量消失
- Xboard 返回 5xx → 边变红

### 看得到 vs 看不到

| 能看到 | 看不到 |
|--------|--------|
| XrayR → Xboard 请求量、错误率、延迟 | VPS 出站侧指标（VPS 没有 Envoy） |
| 每个 VPS 节点的请求明细 | VPS 之间的流量（relay chain 等） |
| Xboard → MySQL/Redis 内部调用链 | 用户 → XrayR 的连接数（不在 mesh 内） |

对运营来说够用 — 你关心的是哪个节点在调 Xboard、有没有报错、延迟多少，这些全能看到。

## Implementation Plan

### Phase 1: airport-prod 集群安装 Istio

```bash
istioctl install --set profile=default \
  --set values.global.meshID=airport-mesh \
  --set values.global.multiCluster.clusterName=airport-prod
```

不需要 VM support 相关配置（VPS 上不装 istio-agent），只需要标准 Istio。

### Phase 2: 安装 Kiali + Prometheus

```bash
kubectl apply -f samples/addons/prometheus.yaml
kubectl apply -f samples/addons/kiali.yaml
kubectl apply -f samples/addons/grafana.yaml    # optional
```

### Phase 3: Xboard namespace 开启 sidecar 注入

```yaml
apiVersion: v1
kind: Namespace
metadata:
  name: airport-prod
  labels:
    istio-injection: enabled
```

重启 Xboard 相关 pod 让 sidecar 注入：

```bash
kubectl -n airport-prod rollout restart deployment xboard
# mysql, redis 也可以注入，这样 Kiali 能看到 xboard → mysql 的边
```

验证 pod 变成 `2/2`（app + istio-proxy）。

### Phase 4: 创建 ServiceAccount + WorkloadGroup

```yaml
# airport-prod/istio/xrayr-workload.yaml
apiVersion: v1
kind: ServiceAccount
metadata:
  name: xrayr-vm
  namespace: airport-prod
---
apiVersion: networking.istio.io/v1beta1
kind: WorkloadGroup
metadata:
  name: xrayr
  namespace: airport-prod
spec:
  metadata:
    labels:
      app: xrayr
      version: v1
  template:
    serviceAccount: xrayr-vm
```

### Phase 5: 为每台 VPS 创建 WorkloadEntry

每台跑 XrayR 的 VPS 对应一个 WorkloadEntry（只是 K8s 里的一条记录，VPS 上什么都不装）：

```yaml
apiVersion: networking.istio.io/v1beta1
kind: WorkloadEntry
metadata:
  name: vps-jp-01
  namespace: airport-prod
  labels:
    app: xrayr
    country: jp
spec:
  address: 103.x.x.x          # VPS 公网 IP
  labels:
    app: xrayr
    version: v1
    country: jp
  serviceAccount: xrayr-vm
```

**这是关键**：Xboard 的 Envoy sidecar 收到来自 `103.x.x.x` 的请求时，查 WorkloadEntry 得知源是 `xrayr/vps-jp-01`，上报给 Prometheus，Kiali 就能画出拓扑。

### Phase 6: 定义 ServiceEntry

```yaml
# airport-prod/istio/xrayr-service.yaml
apiVersion: networking.istio.io/v1beta1
kind: ServiceEntry
metadata:
  name: xrayr-nodes
  namespace: airport-prod
spec:
  hosts:
  - xrayr.airport-prod.mesh
  location: MESH_INTERNAL
  ports:
  - number: 443
    name: https
    protocol: TCP
  resolution: STATIC
  workloadSelector:
    labels:
      app: xrayr
```

ServiceEntry + workloadSelector 把所有 WorkloadEntry（label `app=xrayr`）聚合成一个 service，Kiali 显示为一个 `xrayr` 节点，展开看每个 VPS workload。

## 自动化：sigma-api 管理 WorkloadEntry

手动为每台 VPS 创建/删除 WorkloadEntry 不现实（月抛 VPS）。sigma-api 自动化：

```
VPS 启动
  → sigma-agent POST /api/agent/register (central-platform)
  → sigma-api 创建 VPS 记录
  → sigma-api 调 airport-prod K8s API → 创建 WorkloadEntry
  → Xboard sidecar 开始识别该 VPS 的流量 → Kiali 可见

VPS 退役
  → POST /api/vps/{id}/retire
  → sigma-api 删除 airport-prod 的 WorkloadEntry
  → Kiali 中该节点消失
```

### sigma-api 改造

需要 airport-prod 集群的 K8s 访问权限：

```toml
# sigma-api/Cargo.toml 新增
[dependencies]
kube = { version = "0.98", features = ["runtime", "client", "derive"] }
k8s-openapi = { version = "0.24", features = ["latest"] }
```

新增配置：

| Env var | Description |
|---------|-------------|
| `AIRPORT_KUBECONFIG` | airport-prod 集群的 kubeconfig 路径（或用 IAM role for service account 跨集群访问） |
| `AIRPORT_NAMESPACE` | WorkloadEntry 创建的 namespace（默认 `airport-prod`） |

在 agent register 和 VPS retire handler 中添加 WorkloadEntry 的创建/删除逻辑。

### VPS 侧改动

**无。** sigma-agent 不需要任何改动。XrayR 不需要任何改动。VPS 上没有新进程、没有 iptables 变更、用户流量完全不受影响。

## Checklist

- [ ] airport-prod EKS 安装 Istio (`istioctl install`)
- [ ] 安装 Kiali + Prometheus (`kubectl apply -f samples/addons/`)
- [ ] Xboard namespace 开启 `istio-injection: enabled`
- [ ] 重启 Xboard pod，验证 sidecar 注入 (2/2)
- [ ] 创建 ServiceAccount `xrayr-vm`
- [ ] 创建 WorkloadGroup `xrayr`
- [ ] 创建 ServiceEntry `xrayr-nodes`
- [ ] 为现有 VPS 批量创建 WorkloadEntry（可写脚本从 sigma-api 拉 VPS 列表）
- [ ] 验证 Kiali 显示 xrayr → xboard 流量
- [ ] sigma-api 添加 K8s client，自动管理 WorkloadEntry 生命周期

## Summary

| 组件 | 部署位置 | 改动 |
|------|---------|------|
| istiod + Kiali + Prometheus | airport-prod EKS | 新装 |
| Xboard + Envoy sidecar | airport-prod EKS | 加 sidecar |
| WorkloadEntry (per VPS) | airport-prod EKS | sigma-api 自动创建 |
| XrayR | 每台 VPS | **不改** |
| sigma-agent | 每台 VPS | **不改** |
| 用户 VPN 流量 | 每台 VPS | **不受影响** |
| sigma-api | central-platform EKS | 新增 K8s client 管理 WorkloadEntry |
