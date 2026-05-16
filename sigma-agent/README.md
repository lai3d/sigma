# Sigma Agent

System agent for [Sigma](../README.md) VPS fleet management. Deploys on each VPS to auto-register with the API and send periodic heartbeats with system information.

## How it works

1. **Startup** — auto-discovers hostname, IP addresses, and system info (CPU, RAM, disk, uptime, load)
2. **Register** — `POST /api/agent/register` creates or updates the VPS record (matched by hostname)
3. **Heartbeat loop** — every 60s (configurable), sends `POST /api/agent/heartbeat` with fresh system info

VPS records created by the agent have `provider_id=NULL`. The admin associates the correct provider later via the web UI.

## Install

```bash
# Build from source
cd sigma-agent && cargo build --release

# Or from project root
make agent
```

## Configuration

Config via environment variables or CLI flags (flags override env):

| Env var | CLI flag | Default | Description |
|---------|----------|---------|-------------|
| `SIGMA_API_URL` | `--api-url` | `http://localhost:3000/api` | API base URL |
| `SIGMA_API_KEY` | `--api-key` | — | API key for auth |
| `AGENT_INTERVAL` | `--interval` | `60` | Heartbeat interval (seconds) |
| `AGENT_HOSTNAME` | `--hostname` | auto-detect | Override system hostname |
| `AGENT_SSH_PORT` | `--ssh-port` | `22` | SSH port to report |
| `AGENT_MCP_ENABLED` | `--mcp-enabled` | `false` | Enable MCP (LLM tool) server |
| `AGENT_MCP_BIND` | `--mcp-bind` | `127.0.0.1:9103` | MCP listen address (host:port) |

## Usage

```bash
# Run directly
sigma-agent --api-url http://sigma-api:3000/api --api-key your-key

# Via environment variables
export SIGMA_API_URL=http://sigma-api:3000/api
export SIGMA_API_KEY=your-key
sigma-agent

# Via Docker
docker run -d --name sigma-agent \
  -e SIGMA_API_URL=http://sigma-api:3000/api \
  -e SIGMA_API_KEY=your-key \
  sigma-agent
```

## System info collected

Stored in the VPS record's `extra.system_info` JSON field:

| Field | Source |
|-------|--------|
| `cpu_cores` | `/proc/cpuinfo` |
| `ram_mb` | `/proc/meminfo` (MemTotal) |
| `disk_gb` | `statvfs("/")` |
| `uptime_seconds` | `/proc/uptime` |
| `load_avg` | `/proc/loadavg` (1, 5, 15 min) |

## IP discovery

The agent reads `/proc/net/fib_trie` to find local IP addresses. It filters out:
- Loopback (127.x.x.x)
- Docker bridge (172.17.x.x)

Private IPs are labeled `internal`; public IPs are left unlabeled for the admin to classify.

## Port Scanning & Prometheus Metrics

When `--port-scan` is enabled, the agent periodically scans a configurable port range using `TcpListener::bind()` to detect occupied ports. It also runs `ss -tulnp` to attribute ports to their owning process.

Results are exposed via a Prometheus `/metrics` endpoint:

- `sigma_ports_total` — total ports in scan range
- `sigma_ports_available` — free ports
- `sigma_ports_used{source="..."}` — used ports by process category
- `sigma_port_scan_duration_seconds` — scan timing

### Port Allocation

`POST /ports/allocate` on the metrics port returns N available ports by real-time bind testing. The sigma-api proxies this via `POST /api/vps/{id}/allocate-ports`.

## Envoy xDS Control Plane

When `--xds-enabled` is set, the agent runs a gRPC ADS server that Envoy connects to for dynamic configuration. Implements the standard Envoy xDS v3 SotW protocol.

### Data Flow

```
Web UI → sigma-api (CRUD) → PostgreSQL (config_version bumped on route changes)
                                ↓
sigma-agent (polls API every N seconds, detects version change)
                                ↓
sigma-agent xDS gRPC server → pushes DiscoveryResponse → Envoy
```

Each `envoy_route` generates one **Cluster** (CDS) + one **Listener** (LDS) for Layer 4 TCP proxy. Multiple Envoy instances with different `node.id` values can connect to the same agent simultaneously.

## MCP Tool Surface (LLM-callable control plane)

When `--mcp-enabled` is set, the agent runs a [Model Context Protocol](https://modelcontextprotocol.io) server at `POST /mcp` (JSON-RPC 2.0). This exposes agent capabilities as **tools that an external LLM can call** — e.g., an SRE assistant in `sigma-api` invoking `query_ebpf_traffic` during incident triage, or an automation calling `allocate_ports` when provisioning new Envoy routes.

**Design contract — keep the agent lean.** The MCP server is intentionally light: no LLM, no persistent state, no extra background loops. Each tool wraps data already collected by `port_scan`, `ebpf_traffic`, or `xds`, or proxies a single call to `sigma-api`. Idle resource cost is effectively a listening socket; per-call CPU is bounded by the underlying capability. This keeps the agent within its budget (<1% CPU, <50MB RSS) on 1 vCPU VPS instances. The "AI brain" lives in `sigma-api`, not here.

**Security default — localhost-only.** Binds to `127.0.0.1:9103` by default. Override to `0.0.0.0:9103` (or another address) only behind a network policy.

### Tools

| Tool | Arguments | Returns |
|------|-----------|---------|
| `query_metrics` | none | Host system info + port-scan snapshot |
| `query_ebpf_traffic` | `process?` | Per-process TCP/UDP/RTT/drops/DNS/exec/OOM stats |
| `allocate_ports` | `count` (1-1000) | N free ports from scan range (real-time bind tests) |
| `query_envoy_routes` | `source?` (dynamic/static/all) | Envoy routes for this VPS via sigma-api |
| `query_dns_leaks` | `min_queries?` (default 1) | Processes sending UDP to port 53 |

Tools that depend on capabilities (port scan, eBPF, registration) return a structured `isError` or `enabled=false` payload when their dependency is not configured — they do not break the MCP session.

### Example: list tools

```bash
curl -s -X POST http://127.0.0.1:9103/mcp \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}'
```

### Example: call query_metrics

```bash
curl -s -X POST http://127.0.0.1:9103/mcp \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/call",
       "params":{"name":"query_metrics","arguments":{}}}'
```

### Methods implemented

- `initialize` — protocol handshake (returns server info + capabilities)
- `tools/list` — enumerate tools with JSON schemas
- `tools/call` — invoke a tool by name with arguments
- `notifications/initialized` — acknowledged (no-op)

Unknown methods return JSON-RPC error `-32601` (method not found). Unknown tools return an MCP-shaped `isError: true` payload.

## Comparison with Istio Agent

sigma-agent follows a similar architectural pattern to [istio-agent (pilot-agent)](https://istio.io/latest/docs/ops/deployment/architecture/): a local agent on each node that bridges a central control plane to a local Envoy instance via xDS. Here's how they compare:

### Similarities

| Aspect | sigma-agent | istio-agent |
|--------|-------------|-------------|
| **Role** | Local agent alongside Envoy | Sidecar alongside Envoy |
| **xDS serving** | gRPC ADS server (LDS/CDS) to local Envoy | Proxies/forwards xDS from istiod to local Envoy |
| **Config bridge** | Central API → agent → Envoy | istiod → agent → Envoy |
| **Hot reload** | Detects `config_version` change → push | istiod push → agent forward |
| **Multi-instance** | Multiple Envoy nodes per agent (keyed by `node.id`) | One Envoy per sidecar |

### Differences

| Aspect | sigma-agent | istio-agent |
|--------|-------------|-------------|
| **Scope** | L4 TCP proxy only (LDS + CDS) | Full L4/L7 mesh (LDS, CDS, RDS, EDS, SDS) |
| **Config source** | PostgreSQL via REST API polling | Kubernetes CRDs via istiod push stream |
| **Security** | API key auth, no mTLS between services | Full mTLS with automatic certificate rotation (SDS) |
| **Service discovery** | None — explicit backend host:port per route | Full service discovery via EDS + Kubernetes |
| **Additional duties** | Heartbeat, system metrics, port scanning, port allocation | DNS proxy, health check proxy, certificate management |
| **Target environment** | Standalone VPS instances across providers | Kubernetes pods |
| **Complexity** | ~1500 LOC, single binary | Large codebase, deeply integrated with K8s |

### Architecture Comparison

```
┌─── Istio ──────────────────────────┐    ┌─── Sigma ──────────────────────────┐
│                                    │    │                                    │
│  istiod (central control plane)    │    │  sigma-api (central control plane) │
│    ├─ Pilot (xDS push)             │    │    └─ PostgreSQL (config store)    │
│    ├─ Citadel (cert management)    │    │                                    │
│    └─ Galley (config validation)   │    │                                    │
│         │                          │    │         │                          │
│         │ xDS push stream          │    │         │ REST polling             │
│         ▼                          │    │         ▼                          │
│  ┌─────────────┐                   │    │  ┌─────────────┐                   │
│  │ istio-agent │ (per pod)         │    │  │ sigma-agent │ (per VPS)         │
│  │  ├─ SDS     │                   │    │  │  ├─ xDS     │                   │
│  │  ├─ DNS     │                   │    │  │  ├─ metrics │                   │
│  │  └─ health  │                   │    │  │  └─ portscan│                   │
│  └──────┬──────┘                   │    │  └──────┬──────┘                   │
│         │ xDS (LDS/CDS/RDS/EDS/SDS)│    │         │ xDS (LDS/CDS)           │
│         ▼                          │    │         ▼                          │
│  ┌─────────────┐                   │    │  ┌─────────────┐                   │
│  │   Envoy     │ (full L7 mesh)    │    │  │   Envoy     │ (L4 TCP proxy)   │
│  └─────────────┘                   │    │  └─────────────┘                   │
└────────────────────────────────────┘    └────────────────────────────────────┘
```

### Summary

sigma-agent extracts the core pattern from istio-agent — **local xDS server bridging central config to Envoy** — but strips away the service mesh complexity (mTLS, L7 routing, service discovery, Kubernetes integration). For the use case of managing TCP port forwarding across a fleet of standalone VPS instances, this minimal approach is sufficient.

## Requirements

- Linux (reads `/proc` filesystem)
- sigma-api running with `/api/agent/*` endpoints
- `iproute2` package in Docker image (provides `ss` for port scanning)
- xDS: Envoy configured with ADS bootstrap pointing to agent's gRPC port
