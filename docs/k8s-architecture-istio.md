# Sigma K8s Architecture — Istio Ingress Gateway

Istio Service Mesh + Gateway API — Envoy sidecar proxies, mTLS, observability built-in

```mermaid
graph TB
    Internet((Internet))

    subgraph K8s["Kubernetes Cluster (namespace: sigma)"]

        subgraph IstioSystem["istio-system namespace"]
            Istiod["istiod<br/><i>Control Plane</i>"]
            subgraph IstioGW["istio-ingressgateway (LoadBalancer)"]
                GW1["Gateway Pod #1<br/><i>Envoy Proxy</i>"]
                GW2["Gateway Pod #2<br/><i>Envoy Proxy</i>"]
            end
        end

        subgraph GatewayAPI["Gateway API Resources"]
            GWClass["GatewayClass"]
            GWRes["Gateway :443/:80"]
            RouteWeb["HTTPRoute /*"]
            RouteApi["HTTPRoute /api/*"]
        end

        subgraph WebSvc["sigma-web-service :80 (ClusterIP)"]
            Web1["sigma-web #1<br/><i>nginx:alpine</i><br/>+ envoy sidecar"]
            Web2["sigma-web #2<br/><i>nginx:alpine</i><br/>+ envoy sidecar"]
        end

        subgraph ApiSvc["sigma-api-service :3000 (ClusterIP)"]
            Api1["sigma-api #1<br/><i>Rust / Axum</i><br/>+ envoy sidecar"]
            Api2["sigma-api #2<br/><i>Rust / Axum</i><br/>+ envoy sidecar"]
        end

        Redis[("Redis :6379<br/><i>rate limiting + cache</i>")]

        subgraph Pooler["sigma-db-pooler-rw (ClusterIP)"]
            PgB1["PgBouncer #1<br/><i>txn mode, pool=20</i>"]
            PgB2["PgBouncer #2<br/><i>txn mode, pool=20</i>"]
        end

        subgraph CNPG["CloudNativePG Cluster — PostgreSQL 16.6"]
            Primary["sigma-db-1<br/>PRIMARY (RW)<br/><i>max_conn: 200</i>"]
            Replica1["sigma-db-2<br/>REPLICA (RO)"]
            Replica2["sigma-db-3<br/>REPLICA (RO)"]
            Primary -->|streaming repl| Replica1
            Primary -->|streaming repl| Replica2
        end
    end

    subgraph External["External (VPS nodes)"]
        AgentA["sigma-agent<br/><i>VPS Node A</i>"]
        AgentB["sigma-agent<br/><i>VPS Node B</i>"]
        AgentN["sigma-agent ..."]
        ProbeA["sigma-probe<br/><i>China Node</i>"]
        ProbeB["sigma-probe<br/><i>China Node</i>"]
    end

    Internet --> IstioGW
    Istiod -.->|xDS config push| GW1 & GW2
    Istiod -.->|sidecar config| Web1 & Web2 & Api1 & Api2
    GatewayAPI -.-> IstioGW
    GW1 & GW2 -->|"/* (static)"| WebSvc
    GW1 & GW2 -->|"/api/*"| ApiSvc
    WebSvc <-.->|"mTLS (auto)"| ApiSvc
    Api1 & Api2 --> Redis
    Api1 & Api2 -->|":5432"| Pooler
    PgB1 & PgB2 -->|"rw :5432"| Primary
    AgentA & AgentB & AgentN -->|"/api/agent/*"| ApiSvc
    ProbeA & ProbeB -->|"/api/ip-checks"| ApiSvc
```

## Istio Mesh Benefits

- **mTLS everywhere** — automatic mutual TLS between all pods
- **Sidecar auto-inject** — envoy sidecar injected into every pod
- **L7 traffic policies** — fine-grained routing and retries
- **Distributed tracing** — Jaeger / Zipkin integration
- **Metrics** — Kiali service mesh dashboard
- **Circuit breaking** — protect downstream services
- **Canary / traffic split** — progressive rollouts
- **Full Envoy stack** — gateway + sidecar + sigma-agent xDS

## Connection Flow

```
Internet → Istio Gateway (Envoy L7)
  ├─ /* (static)  → envoy sidecar → sigma-web Pod
  └─ /api/*       → envoy sidecar → sigma-api Pod
                      ├─ Redis (rate limiting)
                      └─ PgBouncer (pooled)
                           └─ PG Primary (200 max)
```
