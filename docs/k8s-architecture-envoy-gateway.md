# Sigma K8s Architecture — Envoy Gateway

Gateway API (GatewayClass + Gateway + HTTPRoute) — Envoy as data plane, unified with sigma-agent xDS stack

```mermaid
graph TB
    Internet((Internet))

    subgraph K8s["Kubernetes Cluster (namespace: sigma)"]

        subgraph GatewayAPI["Gateway API Resources"]
            GWClass["GatewayClass"]
            GW["Gateway :443/:80"]
            RouteWeb["HTTPRoute /*"]
            RouteApi["HTTPRoute /api/*"]
        end

        subgraph EGSystem["envoy-gateway-system namespace"]
            EGCP["envoy-gateway<br/><i>Control Plane</i>"]
            subgraph EGData["Data Plane (LoadBalancer)"]
                EP1["Envoy Proxy #1"]
                EP2["Envoy Proxy #2"]
            end
            EGCP -.->|xDS| EP1 & EP2
        end

        subgraph WebSvc["sigma-web-service :80 (ClusterIP)"]
            Web1["sigma-web #1<br/><i>nginx:alpine</i>"]
            Web2["sigma-web #2<br/><i>nginx:alpine</i>"]
        end

        subgraph ApiSvc["sigma-api-service :3000 (ClusterIP)"]
            Api1["sigma-api #1<br/><i>Rust / Axum</i>"]
            Api2["sigma-api #2<br/><i>Rust / Axum</i>"]
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

    Internet --> EGData
    GatewayAPI -.-> EGCP
    EP1 & EP2 -->|"/* (static)"| WebSvc
    EP1 & EP2 -->|"/api/*"| ApiSvc
    Api1 & Api2 --> Redis
    Api1 & Api2 -->|":5432"| Pooler
    PgB1 & PgB2 -->|"rw :5432"| Primary
    AgentA & AgentB & AgentN -->|"/api/agent/*"| ApiSvc
    ProbeA & ProbeB -->|"/api/ip-checks"| ApiSvc
```

## Why Envoy Gateway?

- **Gateway API (GA)** — standard Kubernetes API for traffic management
- **Envoy data plane** — high-performance L7 proxy
- **TLS via cert-manager** — automatic certificate management
- **Rate limit built-in** — native rate limiting support
- **WASM extensibility** — extend with WebAssembly filters
- **sigma-agent xDS + EG = all Envoy stack** — unified Envoy ecosystem

## Connection Flow

```
Internet → Envoy Proxy (L7)
  ├─ /* (static)  → sigma-web Pod
  └─ /api/*       → sigma-api Pod
                      ├─ Redis (rate limiting)
                      └─ PgBouncer (pooled)
                           └─ PG Primary (200 max)
```
