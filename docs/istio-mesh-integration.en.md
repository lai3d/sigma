# Istio Mesh Integration: VPS ↔ Xboard Traffic Visibility in Kiali

## Goal

VPS nodes run XrayR, K8s (airport-prod EKS) runs Xboard. The goal was to see **per-VPS → Xboard** traffic topology in Kiali, including request rate, error rate, and latency.

## Conclusion

**The WorkloadEntry approach cannot distinguish per-VPS traffic.** See detailed analysis below.

Kiali already shows the full internal topology `istio-ingressgateway → xboard → mysql/redis`, but all external VPS traffic is aggregated under the ingressgateway node — individual VPS sources cannot be identified.

## Attempted Approach: K8s-side WorkloadEntry

### Idea

Install zero Istio components on VPS (zero risk to user traffic). Only create WorkloadEntry resources on the K8s side to record each VPS's public IP. The expectation was that Xboard's sidecar would match source IPs to WorkloadEntries, allowing Kiali to identify traffic sources.

### Test Setup

The following resources were created in the airport-prod cluster:

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

# ServiceEntry (aggregates all edge-nodes)
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
  - number: 8080       # placeholder — VPS is traffic source, not destination
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

### Result

In Kiali, `edge-nodes.airport-prod.mesh` appeared as an isolated ServiceEntry node (triangle icon), **but no traffic edges connected to it**. VPS traffic still showed entirely as `istio-ingressgateway → xboard`.

### Root Cause: Ingress Gateway Masks Source IP

WorkloadEntry source IP matching operates at the **TCP/network layer**. However, the actual traffic path from VPS to Xboard is:

```
VPS (XrayR)
  → AWS NLB (proxy protocol)
    → istio-ingressgateway pod
      → Xboard pod sidecar (inbound)
        → Xboard app container
```

The TCP source IP that Xboard's sidecar sees is the **ingressgateway's pod IP** (e.g. `10.x.x.x`), not the VPS public IP.

Although the NLB is configured with proxy protocol and the ingress gateway sets `X-Forwarded-For` headers, the real VPS IP is preserved only in **HTTP headers** — but Istio performs WorkloadEntry matching using the **network-layer source IP**, not HTTP headers.

```
TCP source IP:        10.x.x.x (ingressgateway pod)  ← Istio uses this for matching
HTTP X-Forwarded-For: <VPS_PUBLIC_IP>                  ← visible at app layer, ignored by Istio
```

Therefore, WorkloadEntry never matches, and Kiali can only show `ingressgateway → xboard`.

### Why Direct Connectivity Is Not Feasible

For WorkloadEntry matching to work, VPS would need to **bypass the ingress gateway and connect directly to Xboard pod IPs**. This requires either:
- VPS being able to route to the K8s pod network (VPC peering + pod CIDR routes)
- Using the east-west gateway + mTLS (requires istio-agent on VPS)

The former increases network exposure. The latter brings us back to installing Istio on VPS.

## Why Not Install Istio on VPS

| Risk | Details |
|------|---------|
| iptables hijacks user traffic | istio-agent configures iptables to intercept all inbound/outbound traffic by default, breaking VPN user connections |
| Envoy handling encrypted tunnel protocols | Envoy doesn't understand V2Ray/Trojan protocols, causing connection failures |
| Performance overhead | High-volume user traffic routed through Envoy adds latency |
| Operational complexity | Maintaining istio-agent + Envoy + iptables on disposable (~monthly) VPS is costly |

**Conclusion**: No Istio components on VPS. Zero risk to user traffic.

## Current State: What Kiali Already Shows

Even without per-VPS identification, Kiali in airport-prod already provides:

```
istio-ingressgateway → airport-xboard → rds-external-mysql
                     → airport-xboard → airport-xboard-redis-with-pv
                     → airport-xboard-subscribe-api → ...
                     → airport-xboard-lite-api → ...
```

Full visibility into K8s-internal service-to-service call chains.

## Alternative Approaches (To Be Evaluated)

If per-VPS observability is needed, the following directions are worth considering:

### Option A: Application-Layer Metrics (Recommended)

Extract Prometheus metrics from the Xboard or Nginx ingress layer using the `X-Forwarded-For` header, grouped by VPS IP. Display per-VPS request rate / error rate / latency in Grafana.

- Pros: No dependency on Istio mesh, pure application-layer solution
- Cons: Not visible in Kiali topology, requires a separate Grafana dashboard

### Option B: VPS-Side Prometheus Metrics

sigma-agent already has a `/metrics` endpoint. XrayR or sigma-agent could export API call metrics (request rate / latency / errors to Xboard), scraped by a centralized Prometheus.

- Pros: Metrics from the source, most accurate
- Cons: Requires sigma-agent changes, additional metric collection on VPS

### Option C: Envoy Access Log Analysis

Use the ingress gateway's access logs, extract VPS IP from the `X-Forwarded-For` header, and aggregate via Loki/Promtail.

- Pros: No application code changes required
- Cons: Log-based rather than metric-based, lower real-time performance and query efficiency compared to Prometheus

## Infrastructure

```
┌─── central-platform EKS ──┐     ┌─── airport-prod EKS ──────────────┐
│ sigma-api                 │     │ xboard + Envoy sidecar             │
│ sigma-web                 │     │ xboard-subscribe-api               │
│ postgres, redis           │     │ xboard-lite-api                    │
│                           │     │ redis, mysql (RDS)                 │
│ (control plane)           │     │                                    │
└───────────────────────────┘     │ istiod + Kiali + Prometheus        │
                                  │ istio-ingressgateway (NLB + PP)    │
         sigma-agent              │ istio-eastwestgateway              │
           register/heartbeat ──→ │                                    │
           (via central-platform) └────────────────────────────────────┘
                                             ▲
                                             │ HTTPS via NLB
                                             │ (ingress gateway terminates TLS)
                                ┌────────────┼────────────┐
                                │            │            │
                           ┌────────┐  ┌────────┐  ┌────────┐
                           │ VPS JP │  │ VPS HK │  │ VPS US │
                           │ XrayR  │  │ XrayR  │  │ XrayR  │
                           │ sigma- │  │ sigma- │  │ sigma- │
                           │ agent  │  │ agent  │  │ agent  │
                           │        │  │        │  │        │
                           │No Istio│  │No Istio│  │No Istio│
                           └────────┘  └────────┘  └────────┘
```

## Traffic Path

XrayR polls the Xboard API every 60s (config pull, heartbeat, traffic reporting):

```
XrayR (VPS)
  → HTTPS POST /api/v1/server/...
  → DNS resolution → NLB (proxy protocol preserves real IP)
  → istio-ingressgateway (terminates TLS, sets X-Forwarded-For)
  → Xboard pod sidecar (inbound, only sees gateway pod IP)
  → Xboard app container
  → response returns via same path
```

## Visibility Matrix

| Available (Kiali) | Missing | Achievable via Alternatives |
|-------------------|---------|-----------------------------|
| ingressgateway → xboard request rate / errors / latency | per-VPS traffic breakdown | Option A/B/C |
| xboard → mysql/redis internal call chain | VPS outbound-side metrics | Option B |
| K8s-internal service topology | Inter-VPS traffic | N/A |
| Overall error rate and latency distribution | Per-VPS error / latency | Option A/B/C |
