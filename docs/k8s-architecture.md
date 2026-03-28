# Sigma K8s Deployment Architecture

Nginx Ingress + CloudNativePG HA + PgBouncer Pool + Multi-Replica Services

```mermaid
graph TB
    Internet((Internet))

    subgraph K8s["Kubernetes Cluster (namespace: sigma)"]

        subgraph Ingress["Nginx Ingress"]
            IG["nginx ingress<br/>sigma.yourdomain.com"]
        end

        subgraph WebSvc["sigma-web-service :80 (ClusterIP)"]
            Web1["sigma-web #1<br/><i>nginx:alpine</i>"]
            Web2["sigma-web #2<br/><i>nginx:alpine</i>"]
        end

        subgraph ApiSvc["sigma-api-service :3000 (ClusterIP)"]
            Api1["sigma-api #1<br/><i>Rust / Axum</i>"]
            Api2["sigma-api #2<br/><i>Rust / Axum</i>"]
        end

        Redis[("Redis<br/>:6379<br/><i>rate limiting + cache</i>")]

        subgraph Pooler["sigma-db-pooler-rw (ClusterIP)"]
            PgB1["PgBouncer #1<br/><i>txn mode, pool=20</i>"]
            PgB2["PgBouncer #2<br/><i>txn mode, pool=20</i>"]
        end

        subgraph CNPG["CloudNativePG Cluster — PostgreSQL 16.6"]
            Primary["sigma-db-1<br/>PRIMARY (RW)<br/><i>max_conn: 200</i>"]
            Replica1["sigma-db-2<br/>REPLICA (RO)<br/><i>streaming repl</i>"]
            Replica2["sigma-db-3<br/>REPLICA (RO)<br/><i>streaming repl</i>"]
            Primary -->|replication| Replica1
            Primary -->|replication| Replica2
        end

        ConfigMap["ConfigMap: sigma-config<br/><i>LISTEN_PORT, REDIS_URL,<br/>RATE_LIMIT, NOTIFY_BEFORE</i>"]
        Secret["Secret: sigma-secrets<br/><i>DATABASE_URL, API_KEY,<br/>TELEGRAM_BOT_TOKEN</i>"]
    end

    subgraph External["External (VPS nodes, not in K8s)"]
        AgentA["sigma-agent<br/><i>VPS Node A</i>"]
        AgentB["sigma-agent<br/><i>VPS Node B</i>"]
        AgentN["sigma-agent<br/><i>VPS Node ...</i>"]
        ProbeA["sigma-probe<br/><i>China Node</i>"]
        ProbeB["sigma-probe<br/><i>China Node</i>"]
    end

    Internet --> IG
    IG -->|"/* (static)"| WebSvc
    IG -->|"/api/*"| ApiSvc
    Api1 & Api2 -->|rate limit| Redis
    Api1 & Api2 -->|":5432"| Pooler
    PgB1 & PgB2 -->|"rw :5432"| Primary
    AgentA & AgentB & AgentN -->|"/api/agent/*"| ApiSvc
    ProbeA & ProbeB -->|"/api/ip-checks"| ApiSvc
```

## Connection Flow

```
Internet → Nginx Ingress (L7)
  ├─ /* (static)  → sigma-web Pods (nginx:alpine)
  └─ /api/*       → sigma-api Pods (Rust/Axum)
                      ├─ Redis (rate limiting)
                      └─ PgBouncer (pool=20, txn mode)
                           └─ PostgreSQL Primary (max_conn=200)
                                ├─ Replica #2 (streaming)
                                └─ Replica #3 (streaming)

External:
  sigma-agent (VPS A, B, ...) → /api/agent/heartbeat, /api/agent/register
  sigma-probe (China nodes)   → /api/ip-checks
```

## Key Specs

| Component | Replicas | Resources |
|-----------|----------|-----------|
| sigma-web | 2 | nginx:alpine |
| sigma-api | 2 | Rust / Axum |
| PgBouncer | 2 | max_client_conn=100, pool=20 |
| PostgreSQL | 1 primary + 2 replicas | 10Gi PVC each, max_conn=200, shared_buf=256MB |
| Redis | 1 | rate limiting + cache |
