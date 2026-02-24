# Istio Mesh Integration: VPS ↔ Xboard Traffic Visibility in Kiali

## Goal

VPS 上跑 XrayR，K8s（airport-prod EKS）里跑 Xboard。在 Kiali 上看到 **XrayR → Xboard** 的流量拓扑，包括请求量、错误率、延迟。

Sigma 自身部署在 central-platform EKS，不需要加入 mesh。

## Infrastructure

```
┌─── central-platform EKS ──┐     ┌─── airport-prod EKS ──────┐
│ sigma-api                 │     │ xboard (panel + API)       │
│ sigma-web                 │     │ mysql / mariadb            │
│ postgres, redis           │     │ redis                      │
│                           │     │                            │
│ (管理面，不入 mesh)        │     │ istiod + Kiali + Prometheus│
└───────────────────────────┘     │ east-west gateway          │
                                  │                            │
         sigma-agent              │ WorkloadEntry (per VPS)    │
           register/heartbeat ──→ │                            │
           (走 central-platform)  └──────────┬─────────────────┘
                                             │ mTLS + xDS
                                ┌────────────┼────────────┐
                                ▼            ▼            ▼
                           ┌────────┐  ┌────────┐  ┌────────┐
                           │ VPS JP │  │ VPS HK │  │ VPS US │
                           │        │  │        │  │        │
                           │ XrayR  │  │ XrayR  │  │ XrayR  │
                           │ istio- │  │ istio- │  │ istio- │
                           │ agent  │  │ agent  │  │ agent  │
                           │ Envoy  │  │ Envoy  │  │ Envoy  │
                           └────────┘  └────────┘  └────────┘
```

**Key point**: Istio 只装在 airport-prod 集群。VPS 作为 VM workload 加入 airport-prod 的 mesh。XrayR → Xboard 的 HTTP 流量经过 mesh Envoy，Kiali 就能看到。

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

## Implementation Plan

### Phase 1: airport-prod 集群安装 Istio

```bash
# 在 airport-prod EKS 上安装 Istio，启用 VM support
istioctl install --set profile=default \
  --set values.pilot.env.PILOT_ENABLE_WORKLOAD_ENTRY_AUTOREGISTRATION=true \
  --set meshConfig.defaultConfig.proxyMetadata.ISTIO_META_DNS_CAPTURE='true' \
  --set values.global.meshID=airport-mesh \
  --set values.global.multiCluster.clusterName=airport-prod \
  --set values.global.network=airport-network
```

### Phase 2: 安装 East-West Gateway

VPS 在外网，需要通过 gateway 连接 istiod 和 mesh 内服务：

```bash
# 生成 east-west gateway 配置
samples/multicluster/gen-eastwest-gateway.sh \
  --network airport-network | istioctl install -y -f -

# 暴露 istiod 给 VM
kubectl apply -f samples/multicluster/expose-istiod.yaml

# 验证 gateway 拿到外部 IP
kubectl -n istio-system get svc istio-eastwestgateway
```

### Phase 3: 安装 Kiali + Prometheus

```bash
kubectl apply -f samples/addons/prometheus.yaml
kubectl apply -f samples/addons/kiali.yaml
kubectl apply -f samples/addons/grafana.yaml    # optional
```

### Phase 4: Xboard namespace 开启 sidecar 注入

```yaml
apiVersion: v1
kind: Namespace
metadata:
  name: airport-prod   # or whatever namespace xboard is in
  labels:
    istio-injection: enabled
```

重启 Xboard pod 让 sidecar 注入：

```bash
kubectl -n airport-prod rollout restart deployment xboard
```

验证 Xboard pod 变成 `2/2`（app + istio-proxy）。

### Phase 5: 定义 VPS WorkloadGroup + ServiceAccount

```yaml
# airport-prod-istio/workload-group.yaml
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
    network: vm-network
```

### Phase 6: 给每台 VPS 生成 Istio 引导文件

对每台跑 XrayR 的 VPS：

```bash
# 在可以访问 airport-prod 集群的机器上执行
istioctl x workload entry configure \
  --name vps-jp-01 \
  --group xrayr \
  --namespace airport-prod \
  --clusterID airport-prod \
  --externalIP <VPS_PUBLIC_IP> \
  --autoregister \
  -o /tmp/vps-jp-01/
```

这会生成：
```
/tmp/vps-jp-01/
├── cluster.env          # ISTIO_SERVICE_CIDR, POD_NAMESPACE 等
├── istio-token          # JWT token 用于 istiod 认证
├── mesh.yaml            # mesh 配置
├── root-cert.pem        # mesh CA 根证书
└── hosts                # istiod DNS 映射
```

### Phase 7: VPS 上安装 istio-agent + Envoy

在每台 VPS 上：

```bash
# 1. 安装 Istio sidecar（下载对应版本）
curl -LO https://github.com/istio/istio/releases/download/1.24.0/istio-sidecar.deb
dpkg -i istio-sidecar.deb

# 2. 把 Phase 6 生成的文件拷到 VPS
scp /tmp/vps-jp-01/* root@<VPS_IP>:/etc/certs/    # root-cert.pem, istio-token
scp /tmp/vps-jp-01/cluster.env root@<VPS_IP>:/var/lib/istio/envoy/
scp /tmp/vps-jp-01/mesh.yaml root@<VPS_IP>:/etc/istio/config/
cat /tmp/vps-jp-01/hosts >> /etc/hosts              # istiod DNS

# 3. 启动 istio-agent（它会自动启动 Envoy）
systemctl enable istio
systemctl start istio

# 4. 验证
istioctl proxy-status   # 应该看到 vps-jp-01 出现
```

启动后 istio-agent 会：
- 连接 airport-prod 的 istiod（通过 east-west gateway）
- 获取 mTLS 证书
- 启动 Envoy sidecar
- 自动创建 `WorkloadEntry`（因为 `--autoregister`）

### Phase 8: XrayR 配置改走 mesh

```yaml
# XrayR config.yml
# Before — 直连 Xboard:
ApiHost: "https://panel.example.com"

# After — 走本地 Envoy sidecar（Istio 透明代理）:
# 不需要改！Istio 的 iptables 规则会自动拦截 outbound 流量
# 只要 XrayR 的目标域名能被 mesh 解析

# 如果 Xboard 在 mesh 内用 ClusterIP service:
ApiHost: "http://xboard.airport-prod.svc.cluster.local"
# istio-agent 的 DNS proxy 会解析这个域名
# Envoy sidecar 自动加 mTLS，路由到 airport-prod 集群
```

## 自动化：sigma-agent 管理 Istio onboarding

手动给每台 VPS 装 istio-agent 不现实（月抛 VPS）。sigma-agent 可以自动化：

```
VPS 启动
  → sigma-agent 启动
  → POST /api/agent/register (central-platform sigma-api)
  → sigma-api 调 airport-prod K8s API:
      istioctl x workload entry configure → 生成引导文件
  → sigma-agent 下载引导文件
  → sigma-agent 安装并启动 istio-agent
  → istio-agent 连接 airport-prod istiod → mesh 就绪
  → XrayR 流量自动经过 mesh Envoy → Kiali 可见

VPS 退役
  → POST /api/vps/{id}/retire
  → sigma-api 删除 airport-prod 的 WorkloadEntry
  → Kiali 中该节点消失
```

### sigma-agent 新增配置

| Env var | CLI flag | Default | Description |
|---------|----------|---------|-------------|
| `AGENT_ISTIO_ENABLED` | `--istio-enabled` | `false` | 启用 Istio mesh 集成 |
| `AGENT_ISTIO_BOOTSTRAP_URL` | `--istio-bootstrap-url` | — | 从 sigma-api 下载引导文件的 URL |

### sigma-api 新增端点

```
GET /api/agent/istio-bootstrap?hostname={hostname}
```

sigma-api 需要能访问 airport-prod 集群的 K8s API（跨集群访问，通过 kubeconfig 或 IAM role）。

## Prometheus 采集 VPS Envoy 指标

Istio Envoy 在 VPS 上暴露 `:15090/stats/prometheus`。airport-prod 的 Prometheus 需要能 scrape 这些 VPS。

### 方案 A: Prometheus Remote Write（推荐）

VPS 上跑一个轻量 Prometheus agent（或 OTEL collector），remote write 到 airport-prod 的 Prometheus：

```yaml
# VPS 上的 prometheus agent config
remote_write:
  - url: https://prometheus.airport-prod.example.com/api/v1/write
scrape_configs:
  - job_name: istio-envoy
    static_configs:
      - targets: ['localhost:15090']
    metrics_path: /stats/prometheus
```

### 方案 B: 利用 sigma 的 file_sd

扩展 sigma-api 的 `GET /api/prometheus/targets`，输出 VPS Envoy metrics 端口（15090）的 targets，airport-prod Prometheus 通过 file_sd 直接 scrape VPS。

前提：VPS 的 15090 端口对 Prometheus 可达。

## 完整流量路径

```
XrayR (VPS)
  → localhost:15001 (Envoy outbound listener, iptables 拦截)
  → Envoy sidecar (mTLS wrap)
  → east-west gateway (airport-prod EKS, 公网 IP)
  → Xboard pod's Envoy sidecar (mTLS unwrap)
  → Xboard app container (:80)
  → response 原路返回

指标上报:
  VPS Envoy → :15090/stats/prometheus → Prometheus → Kiali query
  Xboard sidecar Envoy → Prometheus (K8s 内 scrape) → Kiali query

Kiali 聚合两端指标 → 画出 xrayr → xboard 的边 + 指标
```

## Summary

| 组件 | 部署位置 | 作用 |
|------|---------|------|
| istiod | airport-prod EKS | 控制面：xDS、证书签发 |
| Kiali + Prometheus | airport-prod EKS | 可视化流量拓扑 |
| east-west gateway | airport-prod EKS | VPS 入 mesh 的入口 |
| Xboard + sidecar | airport-prod EKS | 面板 API，Envoy sidecar 上报指标 |
| istio-agent + Envoy | 每台 VPS | 连接 istiod，拦截 XrayR 流量 |
| XrayR | 每台 VPS | 代理节点，调 Xboard API |
| sigma-agent | 每台 VPS | 自动化 istio-agent 安装和生命周期 |
| sigma-api | central-platform EKS | 管理面，自动创建/删除 WorkloadEntry |
