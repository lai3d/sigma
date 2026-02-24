# Istio Service Mesh Integration for VPS Fleet

## Goal

Let all managed VPS instances join the Istio service mesh so that **Kiali shows the full traffic topology** between VPS workloads and Kubernetes services — including XrayR ↔ Xboard API calls, VPN relay chains, and monitoring flows.

## Real-World Scenario: XrayR ↔ Xboard

The primary use case driving this integration:

```
┌─── VPS (dozens, across providers) ─────────┐
│                                            │
│  XrayR (proxy node)                        │
│    ├─ serves user traffic (V2Ray/Trojan)   │
│    └─ periodically calls Xboard API:       │
│         GET  /api/v1/server/config         │
│         POST /api/v1/server/push           │
│         POST /api/v1/alive                 │
│                                            │
└──────────────────┬─────────────────────────┘
                   │ HTTP/HTTPS (periodic polling)
                   ▼
┌─── Kubernetes Cluster ─────────────────────┐
│                                            │
│  Xboard (panel + API)                      │
│    ├─ serves admin web UI                  │
│    ├─ node management API                  │
│    └─ user/traffic accounting              │
│                                            │
│  MySQL / MariaDB                           │
│  Redis                                     │
│                                            │
└────────────────────────────────────────────┘
```

**What Kiali would show:**

```
Kiali Service Graph:

  ┌──────────────┐  HTTP /api/v1/*   ┌──────────────┐
  │ xrayr        │──────────────────→│ xboard       │
  │ (30 VPS)     │  heartbeat+push   │ (K8s pod)    │
  │ JP,HK,US,DE  │                   └──────┬───────┘
  └──────────────┘                          │ TCP
         ↑                                  ▼
    user traffic                    ┌──────────────┐
   (not in mesh)                    │ mysql        │
                                    │ (K8s pod)    │
                                    └──────────────┘
```

- Every XrayR node appears as a workload under the `xrayr` service
- The HTTP edge shows request rate, error rate, latency of API polling
- Click the edge → see per-node breakdown (which VPS is failing, which is slow)
- Alert on: XrayR node stops calling Xboard (node down), error rate spike, latency degradation

## What Kiali Needs

Kiali builds its service graph from **Envoy telemetry metrics** stored in Prometheus. For a VPS to appear in Kiali:

1. VPS must have a `WorkloadEntry` in K8s (gives it a mesh identity)
2. VPS must run `istio-agent` + `Envoy` (managed by Istio, not sigma-agent xDS)
3. Envoy on VPS must report **standard Istio metrics** (`istio_requests_total`, `istio_tcp_sent_bytes_total`, etc.) to the mesh Prometheus
4. Envoy must use **mTLS certificates** issued by istiod (so traffic is attributed correctly)

When all of this is in place, Kiali sees VPS workloads the same as K8s pods:

```
Kiali Graph View:

  ┌─────────────────────────────────────────────────────────────┐
  │                                                             │
  │   [vpn-entry/jp-01] ──TCP──→ [vpn-relay/hk-03]             │
  │          │                        │                         │
  │          │                        ▼                         │
  │          │               [vpn-exit/us-05]                   │
  │          │                                                  │
  │          └──HTTP──→ [sigma-api (K8s)] ──→ [postgres (K8s)]  │
  │                                                             │
  │   Legend: ██ K8s workload   ░░ VM workload                  │
  └─────────────────────────────────────────────────────────────┘
```

## Architecture Overview

```
┌───── Kubernetes Cluster ──────────────────────────────────────────┐
│                                                                   │
│  istiod                 Prometheus        Kiali                   │
│  ├─ xDS (Pilot)         ├─ scrapes        ├─ queries Prometheus   │
│  ├─ CA (Citadel)        │  all Envoy      ├─ builds service graph │
│  └─ config validation   │  /stats/prom    └─ shows VPS + K8s     │
│                         │                                         │
│  East-West Gateway ◄────┼── VPS traffic enters mesh here          │
│  (istio-egressgateway)  │                                         │
│                         │                                         │
│  sigma-api (with sidecar) ◄─── heartbeat/register from VPS       │
│  sigma-web (with sidecar)                                         │
│  postgres, redis                                                  │
│                                                                   │
│  WorkloadEntry (auto-created per VPS by sigma-api)                │
│  ├─ vpn-exit-us-05      network=provider-a                       │
│  ├─ vpn-relay-hk-03     network=provider-b                       │
│  └─ vpn-entry-jp-01     network=provider-c                       │
│                                                                   │
└───────────────────────────┬───────────────────────────────────────┘
                            │ mTLS + xDS
              ┌─────────────┼─────────────┐
              ▼             ▼             ▼
         ┌─────────┐  ┌─────────┐  ┌─────────┐
         │ VPS JP  │  │ VPS HK  │  │ VPS US  │
         │         │  │         │  │         │
         │ sigma-  │  │ sigma-  │  │ sigma-  │
         │ agent   │  │ agent   │  │ agent   │
         │  ↓      │  │  ↓      │  │  ↓      │
         │ istio-  │  │ istio-  │  │ istio-  │
         │ agent   │  │ agent   │  │ agent   │
         │  ↓      │  │  ↓      │  │  ↓      │
         │ Envoy   │  │ Envoy   │  │ Envoy   │
         │ (Istio) │  │ (Istio) │  │ (Istio) │
         └─────────┘  └─────────┘  └─────────┘
```

## Implementation Plan

### Phase 1: K8s Cluster Preparation

#### 1.1 Install Istio with VM Support

```bash
# Install Istio with mesh expansion enabled
istioctl install --set profile=default \
  --set values.pilot.env.PILOT_ENABLE_WORKLOAD_ENTRY_AUTOREGISTRATION=true \
  --set meshConfig.defaultConfig.proxyMetadata.ISTIO_META_DNS_CAPTURE='true' \
  --set values.global.meshID=sigma-mesh \
  --set values.global.multiCluster.clusterName=sigma-k8s \
  --set values.global.network=k8s-network
```

#### 1.2 Install East-West Gateway

VPS instances are on external networks. They reach istiod and K8s services through an east-west gateway:

```bash
# Expose istiod and mesh services to VMs
samples/multicluster/gen-eastwest-gateway.sh \
  --network k8s-network | istioctl install -y -f -

# Expose istiod to external VMs
kubectl apply -f samples/multicluster/expose-istiod.yaml
```

#### 1.3 Install Kiali + Prometheus + Grafana

```bash
kubectl apply -f samples/addons/prometheus.yaml
kubectl apply -f samples/addons/kiali.yaml
kubectl apply -f samples/addons/grafana.yaml
kubectl apply -f samples/addons/jaeger.yaml   # optional: distributed tracing
```

#### 1.4 Enable Sidecar Injection for sigma Namespace

```yaml
# k8s/namespace.yaml
apiVersion: v1
kind: Namespace
metadata:
  name: sigma
  labels:
    istio-injection: enabled   # <-- add this
```

This adds Envoy sidecars to sigma-api, sigma-web, postgres — so Kiali sees their traffic too.

### Phase 2: Define VPS Workload Groups

Create a `WorkloadGroup` per VPS purpose. These are templates for VPS workloads:

```yaml
# k8s/istio/workload-groups.yaml
apiVersion: networking.istio.io/v1beta1
kind: WorkloadGroup
metadata:
  name: xrayr
  namespace: sigma
spec:
  metadata:
    labels:
      app: xrayr
      version: v1
    annotations:
      sigma.io/managed: "true"
  template:
    serviceAccount: sigma-vm
    network: ""          # filled per-VPS by sigma-api
---
apiVersion: networking.istio.io/v1beta1
kind: WorkloadGroup
metadata:
  name: vpn-exit
  namespace: sigma
spec:
  metadata:
    labels:
      app: vpn-exit
      version: v1
    annotations:
      sigma.io/managed: "true"
  template:
    serviceAccount: sigma-vm
    network: ""
---
apiVersion: networking.istio.io/v1beta1
kind: WorkloadGroup
metadata:
  name: vpn-relay
  namespace: sigma
spec:
  metadata:
    labels:
      app: vpn-relay
      version: v1
    annotations:
      sigma.io/managed: "true"
  template:
    serviceAccount: sigma-vm
    network: ""
---
apiVersion: networking.istio.io/v1beta1
kind: WorkloadGroup
metadata:
  name: vpn-entry
  namespace: sigma
spec:
  metadata:
    labels:
      app: vpn-entry
      version: v1
    annotations:
      sigma.io/managed: "true"
  template:
    serviceAccount: sigma-vm
    network: ""
---
# ServiceAccount for all VM workloads
apiVersion: v1
kind: ServiceAccount
metadata:
  name: sigma-vm
  namespace: sigma
```

### Phase 3: sigma-api Creates WorkloadEntry Automatically

When sigma-agent registers a VPS, sigma-api creates a `WorkloadEntry` via the K8s API:

#### 3.1 New API Behavior on Agent Register

```
POST /api/agent/register
  → existing: create/update VPS record
  → new: also create WorkloadEntry in K8s
```

The WorkloadEntry sigma-api creates per VPS:

```yaml
apiVersion: networking.istio.io/v1beta1
kind: WorkloadEntry
metadata:
  name: vps-{hostname}
  namespace: sigma
  labels:
    app: {purpose}          # vpn-exit, vpn-relay, vpn-entry, etc.
    version: v1
    country: {country}
    provider: {provider_name}
    sigma.io/vps-id: {vps_uuid}
spec:
  address: {public_ip}
  labels:
    app: {purpose}
    version: v1
    country: {country}
  serviceAccount: sigma-vm
  network: provider-{provider_id}   # each provider = separate network
```

#### 3.2 VPS Retire Cleans Up

```
POST /api/vps/{id}/retire
  → existing: mark VPS as retired
  → new: delete the WorkloadEntry from K8s
```

#### 3.3 sigma-api Needs K8s Client

Add `kube` crate dependency to sigma-api:

```toml
# sigma-api/Cargo.toml
[dependencies]
kube = { version = "0.98", features = ["runtime", "client", "derive"] }
k8s-openapi = { version = "0.24", features = ["latest"] }
```

New module `sigma-api/src/routes/istio.rs` or inline in agent registration:

```rust
// Pseudo-code for WorkloadEntry management
async fn create_workload_entry(vps: &Vps, provider_name: &str) -> Result<()> {
    let client = kube::Client::try_default().await?;
    let api: Api<WorkloadEntry> = Api::namespaced(client, "sigma");

    let we = WorkloadEntry {
        metadata: ObjectMeta {
            name: Some(format!("vps-{}", vps.hostname)),
            namespace: Some("sigma".into()),
            labels: Some(BTreeMap::from([
                ("app".into(), vps.purpose.clone()),
                ("country".into(), vps.country.clone()),
                ("sigma.io/vps-id".into(), vps.id.to_string()),
            ])),
            ..Default::default()
        },
        spec: WorkloadEntrySpec {
            address: vps.ip_addresses[0].ip.clone(),
            labels: BTreeMap::from([
                ("app".into(), vps.purpose.clone()),
                ("version".into(), "v1".into()),
            ]),
            service_account: "sigma-vm".into(),
            network: format!("provider-{}", vps.provider_id.unwrap_or_default()),
            ..Default::default()
        },
    };

    api.create(&PostParams::default(), &we).await?;
    Ok(())
}
```

### Phase 4: sigma-agent Bootstraps Istio on VPS

#### 4.1 New API Endpoint for Istio Bootstrap

```
GET /api/agent/istio-bootstrap?hostname={hostname}
```

Returns a tarball / JSON containing:
- `mesh.yaml` — mesh config
- `istio-token` — JWT for istiod authentication
- `root-cert.pem` — mesh root CA
- `cluster.env` — environment variables
- `hosts` — /etc/hosts additions for istiod DNS

sigma-api generates these by calling `istioctl x workload entry configure` internally, or by directly constructing the files from known mesh parameters.

#### 4.2 sigma-agent Lifecycle

```
sigma-agent startup:
  1. Register with sigma-api (existing)
  2. Download Istio bootstrap files (new)
  3. Install istio-agent binary if not present (new)
  4. Write bootstrap config to /etc/istio/ (new)
  5. Start/restart istio-agent process (new)
  6. istio-agent starts Envoy with istiod-managed xDS
  7. Continue heartbeat loop (existing)

sigma-agent VPS retire:
  1. Stop istio-agent + Envoy (new)
  2. sigma-api deletes WorkloadEntry (new)
```

#### 4.3 Config Changes

New sigma-agent config flags:

| Env var | CLI flag | Default | Description |
|---------|----------|---------|-------------|
| `AGENT_ISTIO_ENABLED` | `--istio-enabled` | `false` | Enable Istio mesh integration |
| `AGENT_ISTIO_AGENT_PATH` | `--istio-agent-path` | `/usr/local/bin/pilot-agent` | Path to istio-agent binary |

When `--istio-enabled` is set:
- sigma-agent's own xDS server is **disabled** (istio-agent takes over Envoy management)
- sigma-agent manages the `istio-agent` process instead
- Envoy is started by istio-agent, not by the user's bootstrap config

### Phase 5: Service Definitions for Kiali

For Kiali to label traffic correctly, define `ServiceEntry` resources for VPS-hosted services:

```yaml
# k8s/istio/service-entries.yaml

# XrayR nodes — the primary VPS workload
# Each XrayR instance polls Xboard API for config and pushes traffic stats
apiVersion: networking.istio.io/v1beta1
kind: ServiceEntry
metadata:
  name: xrayr-nodes
  namespace: sigma
spec:
  hosts:
  - xrayr.sigma.mesh       # virtual hostname for mesh routing
  location: MESH_INTERNAL
  ports:
  - number: 443
    name: tls
    protocol: TLS
  - number: 10000-30000
    name: proxy-ports
    protocol: TCP
  resolution: STATIC
  workloadSelector:
    labels:
      app: xrayr
---
apiVersion: networking.istio.io/v1beta1
kind: ServiceEntry
metadata:
  name: vpn-exit-nodes
  namespace: sigma
spec:
  hosts:
  - vpn-exit.sigma.mesh
  location: MESH_INTERNAL
  ports:
  - number: 443
    name: tls
    protocol: TLS
  - number: 10000-30000
    name: vpn-ports
    protocol: TCP
  resolution: STATIC
  workloadSelector:
    labels:
      app: vpn-exit
---
apiVersion: networking.istio.io/v1beta1
kind: ServiceEntry
metadata:
  name: vpn-relay-nodes
  namespace: sigma
spec:
  hosts:
  - vpn-relay.sigma.mesh
  location: MESH_INTERNAL
  ports:
  - number: 443
    name: tls
    protocol: TLS
  - number: 10000-30000
    name: vpn-ports
    protocol: TCP
  resolution: STATIC
  workloadSelector:
    labels:
      app: vpn-relay
---
apiVersion: networking.istio.io/v1beta1
kind: ServiceEntry
metadata:
  name: vpn-entry-nodes
  namespace: sigma
spec:
  hosts:
  - vpn-entry.sigma.mesh
  location: MESH_INTERNAL
  ports:
  - number: 443
    name: tls
    protocol: TLP
  - number: 10000-30000
    name: vpn-ports
    protocol: TCP
  resolution: STATIC
  workloadSelector:
    labels:
      app: vpn-entry
```

These `ServiceEntry` + `workloadSelector` bind to the `WorkloadEntry` instances by label, making Kiali show them as proper services with named edges.

### Phase 6: Prometheus Scraping VPS Envoy Metrics

Istio's Envoy on VPS exposes metrics at `:15090/stats/prometheus`. Prometheus needs to scrape them.

#### Option A: Prometheus Federation (Recommended)

VPS Envoy metrics → scraped by local Prometheus on VPS → federated to central Prometheus in K8s.

sigma-agent already runs a `/metrics` endpoint — extend it to also scrape and forward Envoy's metrics. Or simpler: configure the central Prometheus to scrape VPS Envoy directly:

#### Option B: Direct Remote Scrape

```yaml
# prometheus/prometheus.yml (in K8s)
# sigma-api exposes a targets endpoint that includes Envoy metrics ports
scrape_configs:
  - job_name: 'istio-vm-envoy'
    # Use sigma's existing Prometheus file_sd integration
    file_sd_configs:
      - files:
        - /etc/prometheus/sigma_envoy_targets.json
    metrics_path: /stats/prometheus
    scheme: http
```

sigma-api's `GET /api/prometheus/targets` can be extended to output Envoy metrics targets (port 15090) for all mesh-enrolled VPS instances.

## What You'll See in Kiali

Once all phases are complete:

### Service Graph
```
┌──────────────────────────────────────────────────────────────────┐
│  Kiali Service Graph (namespace: sigma)                          │
│                                                                  │
│  ┌───────────────┐  HTTP /api/v1/*  ┌────────────────┐          │
│  │ xrayr         │────────────────→ │ xboard         │          │
│  │ (30 workloads)│  heartbeat+push  │ (K8s pod)      │          │
│  │ JP,HK,US,DE  │                  └───────┬────────┘          │
│  └───────────────┘                          │ TCP               │
│                                             ▼                    │
│  ┌───────────────┐              ┌────────────────┐              │
│  │ vpn-relay     │──── TCP ────→│ vpn-exit       │              │
│  │ (5 workloads) │              │ (8 workloads)  │              │
│  │ HK, SG, MY   │              │ US, DE, NL     │              │
│  └───────┬───────┘              └────────────────┘              │
│          │                                                       │
│          │ HTTP                                                   │
│          ▼                                                       │
│  ┌───────────────┐     TCP      ┌────────────────┐              │
│  │ sigma-api     │─────────────→│ postgres       │              │
│  │ (K8s pod)     │              │ (K8s pod)      │              │
│  └───────────────┘              └────────────────┘              │
│                                                                  │
│  ██ Healthy   ░░ Degraded   ▓▓ Failing                          │
└──────────────────────────────────────────────────────────────────┘
```

### Workload View
- Each VPS appears as a separate workload under its service (xrayr, vpn-relay, vpn-exit)
- Labels show country, provider
- XrayR → Xboard: HTTP metrics (request rate, error rate, latency per endpoint)
- VPN relay → exit: TCP metrics (bytes sent/received, connection count)

### Traffic Animation
- Real-time traffic flow animation between services
- Green/yellow/red edges based on error rates
- Click any edge to see per-workload breakdown (which VPS node is slow/failing)

### Operational Value
- **XrayR node down**: Kiali shows the node stops sending traffic to Xboard → alert
- **API error spike**: Xboard returns 5xx → visible as red edge in graph
- **Latency degradation**: XrayR → Xboard polling slows down → Kiali shows P99 increase
- **Traffic imbalance**: one XrayR node handling 10x more user traffic → visible in graph

## Migration Path: sigma-agent xDS → Istio xDS

The transition is per-VPS and can be done gradually:

| Phase | xDS Source | Envoy Managed By | In Kiali? |
|-------|-----------|-------------------|-----------|
| Current | sigma-agent | User (bootstrap YAML) | No |
| Transition | istio-agent (istiod) | istio-agent | Yes |

Steps per VPS:
1. Enable `--istio-enabled` on sigma-agent
2. sigma-agent stops its own xDS server
3. sigma-agent downloads Istio bootstrap, starts istio-agent
4. istio-agent starts Envoy with istiod config
5. Existing envoy_routes in sigma DB → migrate to Istio `VirtualService`/`DestinationRule`

Envoy routes previously managed by sigma's xDS become Istio CRDs:

```yaml
# Example 1: XrayR traffic to Xboard (HTTP API)
# XrayR on VPS calls Xboard in K8s for config/heartbeat/traffic push
# With Istio, this traffic flows through mesh Envoy → mTLS → visible in Kiali
apiVersion: networking.istio.io/v1beta1
kind: DestinationRule
metadata:
  name: xboard-mtls
  namespace: sigma
spec:
  host: xboard.sigma.svc.cluster.local
  trafficPolicy:
    tls:
      mode: ISTIO_MUTUAL
---
# Example 2: VPN relay port forwarding (TCP)
# What was: envoy_route { listen_port: 10001, backend_host: "1.2.3.4", backend_port: 443 }
# Becomes:
apiVersion: networking.istio.io/v1beta1
kind: VirtualService
metadata:
  name: route-port-10001
  namespace: sigma
spec:
  hosts:
  - vpn-relay.sigma.mesh
  tcp:
  - match:
    - port: 10001
    route:
    - destination:
        host: vpn-exit.sigma.mesh
        port:
          number: 443
```

### XrayR Configuration Change

XrayR's `config.yml` needs to point to the mesh-internal hostname for Xboard:

```yaml
# Before (direct IP/domain):
ApiHost: "https://panel.example.com"

# After (mesh routing via Istio sidecar):
ApiHost: "http://xboard.sigma.svc.cluster.local"
# mTLS is handled transparently by Envoy sidecar
# Traffic visible in Kiali as xrayr → xboard
```

The Envoy sidecar on the VPS intercepts outbound HTTP to `xboard.sigma.svc.cluster.local`, wraps it in mTLS, and routes it through the east-west gateway into the K8s cluster. Kiali sees the full path.

## File Summary

New/modified files for this integration:

```
k8s/
├── namespace.yaml                    # add istio-injection label
├── istio/
│   ├── workload-groups.yaml          # WorkloadGroup per VPS purpose
│   ├── service-entries.yaml          # ServiceEntry for VPS services
│   ├── east-west-gateway.yaml        # expose istiod to VMs
│   └── peer-authentication.yaml      # mTLS policy
sigma-api/
├── Cargo.toml                        # add kube, k8s-openapi deps
├── src/routes/agent.rs               # create/delete WorkloadEntry on register/retire
└── src/routes/istio.rs               # GET /api/agent/istio-bootstrap
sigma-agent/
├── src/main.rs                       # add --istio-enabled flag, manage istio-agent process
├── src/istio.rs                      # download bootstrap, start/stop istio-agent
└── src/config.rs                     # new config flags
```
