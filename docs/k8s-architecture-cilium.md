# Sigma K8s Architecture — Cilium Gateway

Cilium — eBPF-powered CNI + Gateway API + Network Policy + Hubble observability, all in one

```mermaid
graph TB
    Internet((Internet))

    subgraph K8s["Kubernetes Cluster (namespace: sigma)"]

        subgraph eBPF["eBPF Data Plane (kernel)"]
            XDP["XDP fast path"]
            TC["tc (L3/L4)"]
            SocketLB["socket LB"]
            L7["L7 (Envoy)"]
            Conntrack["conntrack"]
            NAT["NAT<br/><i>no kube-proxy</i>"]
        end

        subgraph GatewayAPI["Gateway API Resources (Cilium-managed)"]
            GWClass["GatewayClass"]
            GW["Gateway :443/:80"]
            RouteWeb["HTTPRoute /*"]
            RouteApi["HTTPRoute /api/*"]
        end

        subgraph CiliumGW["Cilium Gateway (Envoy-based LoadBalancer)"]
            GW1["Gateway Pod #1<br/><i>Cilium + Envoy L7</i>"]
            GW2["Gateway Pod #2<br/><i>Cilium + Envoy L7</i>"]
        end

        subgraph CiliumDS["Cilium Agent (DaemonSet)"]
            CAgent["cilium-agent<br/><i>every node</i>"]
            Hubble["Hubble<br/><i>observability</i>"]
        end

        CNP["CiliumNetworkPolicy<br/><i>L3/L4/L7 identity-based<br/>DNS-aware, no iptables</i>"]

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

    Internet --> CiliumGW
    GatewayAPI -.-> CiliumGW
    GW1 & GW2 -->|"/* (static)"| WebSvc
    GW1 & GW2 -->|"/api/*"| ApiSvc
    WebSvc <-.->|"eBPF (identity)"| ApiSvc
    Api1 & Api2 --> Redis
    Api1 & Api2 -->|":5432"| Pooler
    PgB1 & PgB2 -->|"rw :5432"| Primary
    AgentA & AgentB & AgentN -->|"/api/agent/*"| ApiSvc
    ProbeA & ProbeB -->|"/api/ip-checks"| ApiSvc
```

## Cilium Advantages

- **eBPF** — no iptables, no kube-proxy
- **CNI + Gateway + Policy unified** — single platform
- **Gateway API (native)** — first-class support
- **Identity-based network policy** — L3/L4/L7 with DNS-aware rules
- **Hubble flow observability** — real-time traffic visibility
- **WireGuard encryption** — optional transparent encryption
- **No sidecar overhead** — kernel-level networking
- **Bandwidth Manager (EDT)** — fair queuing
- **Sigma synergy** — both sigma-agent and Cilium use eBPF
- **XDP DDoS mitigation** — kernel bypass for fast-path drops

## Connection Flow

```
Internet → Cilium Gateway (eBPF + Envoy L7)
  ├─ /* (static)  → eBPF socket LB → sigma-web Pod
  └─ /api/*       → eBPF socket LB → sigma-api Pod
                      ├─ Redis (rate limiting)
                      └─ PgBouncer (pooled)
                           └─ PG Primary (200 max)
```
