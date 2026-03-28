# Sigma K8s Architecture — Traefik

Traefik Proxy — Cloud-native edge router with automatic HTTPS, dashboard, and Gateway API support

```mermaid
graph TB
    Internet((Internet))
    LE["Let's Encrypt (ACME)<br/><i>auto TLS provisioning</i>"]

    subgraph K8s["Kubernetes Cluster (namespace: sigma)"]

        subgraph TraefikConfig["Traefik Routing Configuration"]
            IR1["IngressRoute /*"]
            IR2["IngressRoute /api/*"]
            TLS["TLS (auto ACME)"]
            MW["Middleware (rate)"]
        end

        Dashboard["Traefik Dashboard<br/><i>:8080/dashboard</i>"]

        subgraph TraefikSvc["traefik-service (LoadBalancer :443 :80)"]
            T1["Traefik Proxy v3 #1<br/><i>L7 routing + ACME<br/>+ dashboard</i>"]
            T2["Traefik Proxy v3 #2<br/><i>L7 routing + ACME<br/>+ dashboard</i>"]
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

    Internet --> TraefikSvc
    LE -.->|cert sync| T1 & T2
    TraefikConfig -.-> TraefikSvc
    T1 & T2 -.-> Dashboard
    T1 & T2 -->|"/* (static)"| WebSvc
    T1 & T2 -->|"/api/*"| ApiSvc
    Api1 & Api2 --> Redis
    Api1 & Api2 -->|":5432"| Pooler
    PgB1 & PgB2 -->|"rw :5432"| Primary
    AgentA & AgentB & AgentN -->|"/api/agent/*"| ApiSvc
    ProbeA & ProbeB -->|"/api/ip-checks"| ApiSvc
```

## Traefik Features

- **Auto HTTPS** — Let's Encrypt ACME (HTTP-01 / DNS-01)
- **IngressRoute CRD** — more expressive than annotations
- **Gateway API support (v3)** — standard K8s routing
- **Built-in dashboard :8080** — routers, services, middlewares
- **Middleware chains** — rate-limit, auth, headers, redirect
- **Canary / weighted round-robin** — progressive rollouts
- **Metrics (Prometheus)** — built-in metrics exporter
- **TCP/UDP routing** — SSH, DNS, etc.
- **K3s default ingress** — lightweight, single binary

## Considerations

- No service mesh / mTLS (ingress only)
- Advanced features require Traefik Enterprise
- Less raw performance vs Envoy at very high RPS
- CRD sprawl (IngressRoute, Middleware, TLSOption...)
- No sidecar model — edge-only proxy

## Connection Flow

```
Internet → Traefik Proxy (L7 + TLS)
  ├─ /* (static)  → ClusterIP → sigma-web Pod
  └─ /api/*       → ClusterIP → sigma-api Pod
                      ├─ Redis (rate limiting)
                      └─ PgBouncer (pooled)
                           └─ PG Primary (200 max)
```
