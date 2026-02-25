# sigma-agent — VPS System Agent

## What is this?

sigma-agent is a standalone Rust binary deployed on each VPS instance. It auto-discovers
the hostname and IP addresses, registers with the sigma API, and sends periodic heartbeats
with system information (CPU, RAM, disk, uptime, load average). If the VPS already exists
in sigma (matched by hostname), it updates the record. If not, it creates a new VPS record
with provider_id=NULL (admin assigns the provider later via web UI).

## Architecture

```
sigma-agent/
├── Cargo.toml
├── Dockerfile
├── CLAUDE.md
└── src/
    ├── main.rs            # Entry point: register + heartbeat loop + spawn scan/metrics/xds/config-sync
    ├── config.rs          # Configuration: env vars + CLI flags (clap)
    ├── client.rs          # HTTP client (GET + POST with X-Api-Key auth)
    ├── models.rs          # API request/response types
    ├── system.rs          # Linux system info collection (/proc, statvfs)
    ├── port_scan.rs       # Port scanning: TcpListener::bind test + ss parsing
    ├── metrics.rs         # Prometheus metrics HTTP server (/metrics)
    ├── envoy_config.rs    # Parse envoy.yaml static_resources → static route entries
    ├── xds.rs             # gRPC ADS server: config polling, push to Envoy clients
    └── xds_resources.rs   # Builds xDS Cluster + Listener protos from envoy routes
```

## Configuration

| Env var | CLI flag | Default | Description |
|---------|----------|---------|-------------|
| `SIGMA_API_URL` | `--api-url` | `http://localhost:3000/api` | API base URL |
| `SIGMA_API_KEY` | `--api-key` | (none) | API key for auth |
| `AGENT_INTERVAL` | `--interval` | `60` | Heartbeat interval (seconds) |
| `AGENT_HOSTNAME` | `--hostname` | (auto-detect) | Override hostname |
| `AGENT_SSH_PORT` | `--ssh-port` | `22` | SSH port to report |
| `AGENT_METRICS_PORT` | `--metrics-port` | `9102` | Prometheus metrics port (0=disable) |
| `AGENT_PORT_SCAN` | `--port-scan` | `false` | Enable port scanning |
| `AGENT_PORT_SCAN_RANGE` | `--port-scan-range` | `10000-30000` | Port scan range (START-END) |
| `AGENT_PORT_SCAN_INTERVAL` | `--port-scan-interval` | `60` | Port scan interval (seconds) |
| `AGENT_XDS_ENABLED` | `--xds-enabled` | `false` | Enable xDS gRPC server |
| `AGENT_XDS_PORT` | `--xds-port` | `18000` | xDS gRPC listen port |
| `AGENT_XDS_POLL_INTERVAL` | `--xds-poll-interval` | `10` | xDS config poll interval (seconds) |
| `AGENT_ENVOY_CONFIG_PATH` | `--envoy-config-path` | `/etc/envoy/envoy.yaml` | Comma-separated Envoy config file paths |
| `AGENT_ENVOY_CONFIG_SYNC` | `--envoy-config-sync` | `false` | Enable static config sync |
| `AGENT_ENVOY_CONFIG_SYNC_INTERVAL` | `--envoy-config-sync-interval` | `60` | File poll interval (seconds) |

## IP Discovery

1. Reads local IPs from `/proc/net/fib_trie` (filters out loopback and Docker bridge IPs)
2. If no public IP is found locally (common on NAT'd VPS), falls back to external lookup:
   - Tries `icanhazip.com` → `ifconfig.me` → `api.ipify.org` in sequence (5s timeout each)
   - First successful response is added to the IP list

## System Info Collected

- cpu_cores: from /proc/cpuinfo
- ram_mb: from /proc/meminfo (MemTotal)
- disk_gb: from statvfs("/")
- uptime_seconds: from /proc/uptime
- load_avg: from /proc/loadavg (1, 5, 15 min)

## API Endpoints Used

- POST /api/agent/register — initial registration with full system info + IPs
- POST /api/agent/heartbeat — periodic update with system info
- GET /api/envoy-nodes?vps_id=...&status=active — fetch envoy nodes for this VPS (xDS)
- GET /api/envoy-routes?envoy_node_id=...&status=active&source=dynamic — fetch dynamic routes for a node (xDS)
- POST /api/envoy-routes/sync-static — sync static routes parsed from envoy.yaml

## Build & Run

```bash
cargo build --release
./target/release/sigma-agent --api-url http://sigma-api:3000/api --api-key your-key

# Or via Docker (mount /etc/hostname so agent reports host's hostname):
docker run -d --name sigma-agent \
  -v /etc/hostname:/etc/host_hostname:ro \
  -e SIGMA_API_URL=http://sigma-api:3000/api \
  -e SIGMA_API_KEY=your-key \
  sigma-agent
```

## Port Scanning & Prometheus Metrics

When `--port-scan` is enabled, the agent periodically scans the configured port range using
`TcpListener::bind("0.0.0.0", port)` to detect occupied ports (catches TIME_WAIT etc. that
`ss` may miss). It also runs `ss -tulnp` to attribute occupied ports to their owning process
(envoy, sshd, nginx, node_exporter, other, unknown).

Results are exposed via a Prometheus-compatible `/metrics` endpoint on `--metrics-port`:
- `sigma_ports_total` — total ports in scan range
- `sigma_ports_available` — free ports
- `sigma_ports_used{source="..."}` — used ports by process category
- `sigma_port_scan_duration_seconds` — scan timing

Known sources are always emitted (even at 0) for stable Grafana time series.

### Port Allocation

The agent also exposes `POST /ports/allocate` on the same metrics port. Given a `count` (1-100),
it returns N available ports from the configured scan range by real-time bind testing.
This is stateless — no reservation — the caller (e.g. Envoy config) should bind immediately.

The sigma-api proxies this via `POST /api/vps/{id}/allocate-ports`, looking up the VPS IP and
agent metrics_port from the heartbeat system_info.

### ss Parser

The `parse_ss_line()` function handles both `ss -tlnp` (no Netid column) and `ss -tulnp`
(with Netid column like `tcp`/`udp`) by dynamically finding the LISTEN position in the first
two fields.

## Docker Deployment

The agent requires host-level access for accurate port scanning and process attribution:

```yaml
agent:
  network_mode: host    # Scan host ports, not container's own
  pid: host             # See process names across containers
  cap_add:
    - SYS_PTRACE        # Required for ss -p to read /proc/<pid>/fd
```

**Docker Desktop limitation**: The `allocate-ports` API proxy won't work locally because
containers on bridge network can't reach the agent on host network. This works in production
where API and agent run on different machines connected via public IPs.

## Grafana Dashboard

A pre-built dashboard is available at `grafana/dashboards/port-scan.json` with:
- Stat cards (total/available/used ports, utilization %, scan duration)
- Time series charts (port availability, usage by process over time)
- Pie chart (port usage breakdown by source)
- Host summary table (multi-host overview)

## Envoy xDS Control Plane

When `--xds-enabled` is set, the agent runs a gRPC ADS (Aggregated Discovery Service) server
that Envoy instances connect to for dynamic configuration. This implements the standard Envoy
xDS v3 SotW (State of the World) protocol using `tonic` + `xds-api` crate.

### Data Flow

```
Web UI → sigma-api (CRUD) → PostgreSQL (config_version bumped on route changes)
                                ↓
sigma-agent (polls API every --xds-poll seconds, detects version change)
                                ↓
sigma-agent xDS gRPC server → pushes DiscoveryResponse → Envoy
```

### How It Works

1. Agent registers with sigma-api, obtains its VPS UUID
2. Poll loop fetches ALL active envoy_nodes for this VPS via `GET /api/envoy-nodes?vps_id=...`
3. For each node, compares `config_version` — if changed, fetches routes via `GET /api/envoy-routes?envoy_node_id=...`
4. Each active route generates one **Cluster** (CDS) + one **Listener** (LDS) for Layer 4 TCP proxy
5. Maintains per-node config snapshots keyed by `node_id`
6. When an Envoy client connects, its `node.id` from the DiscoveryRequest is matched to the right config
7. Multiple Envoy instances with different node_ids can connect to the same agent simultaneously

### Resource Mapping

Each `envoy_route` row maps to:
- **Cluster**: `cluster-{route.name}` — upstream with `backend_host:backend_port`, cluster type
  mapped from `cluster_type` (static/strict_dns/logical_dns), optional proxy protocol v1/v2
  transport socket
- **Listener**: `listener-{route.name}` — binds `0.0.0.0:{listen_port}`, filter chain with
  `envoy.filters.network.tcp_proxy` pointing to the cluster

### Proto Types

The `xds-api` crate provides pre-compiled Envoy proto bindings (no protoc needed). Types not
included in xds-api (`TcpProxy`, `ProxyProtocolUpstreamTransport`, `RawBuffer`) are defined as
minimal `prost::Message` structs in `xds_resources.rs`.

**Important**: xds-api generates its own `google.protobuf.Any` and `Duration` types
(`xds_api::pb::google::protobuf::*`) — these are distinct from `prost_types::Any`/`Duration`.
All xDS code must use the xds-api versions.

### Envoy Bootstrap Config

Envoy instances need a minimal bootstrap pointing to the agent's xDS server:

```yaml
node:
  id: "layer4-01"
  cluster: "sigma"
dynamic_resources:
  ads_config:
    api_type: GRPC
    transport_api_version: V3
    grpc_services:
      - envoy_grpc:
          cluster_name: xds_cluster
  lds_config: { ads: {} }
  cds_config: { ads: {} }
static_resources:
  clusters:
    - name: xds_cluster
      type: STATIC
      connect_timeout: 5s
      load_assignment:
        cluster_name: xds_cluster
        endpoints:
          - lb_endpoints:
              - endpoint:
                  address:
                    socket_address: { address: 127.0.0.1, port_value: 18000 }
      typed_extension_protocol_options:
        envoy.extensions.upstreams.http.v3.HttpProtocolOptions:
          "@type": type.googleapis.com/envoy.extensions.upstreams.http.v3.HttpProtocolOptions
          explicit_http_version:
            http2_protocol_options: {}
```

## Envoy Static Config Sync

When `--envoy-config-sync` is enabled, the agent parses the local `envoy.yaml` file and syncs
any `static_resources` routes to sigma-api. This lets operators see **all** routes in the UI —
both dynamic (managed via API/UI) and static (from config files) — with clear `source` labels.

### How It Works

1. On startup, parses each path in `--envoy-config-path` (comma-separated, default `/etc/envoy/envoy.yaml`)
2. Each file gets its own envoy node: `static-{hostname}` for `envoy.yaml`, `static-{hostname}-{stem}` for other names
3. Extracts listeners + clusters from `static_resources`, skipping `xds_cluster`
4. POSTs parsed routes to `POST /api/envoy-routes/sync-static` with `source=static`
5. API upserts by `(envoy_node_id, listen_port)` and deletes stale static routes
6. Polls each file mtime every `--envoy-config-sync-interval` seconds; re-syncs on change

### Multiple Envoy Instances

To sync multiple Envoy static configs, pass comma-separated paths:

```bash
AGENT_ENVOY_CONFIG_PATH=/etc/envoy/envoy.yaml,/etc/envoy/envoy-relay.yaml,/etc/envoy/envoy-exit.yaml
```

Each file creates a separate envoy node in sigma:
- `/etc/envoy/envoy.yaml` → node `static-myhost`
- `/etc/envoy/envoy-relay.yaml` → node `static-myhost-envoy-relay`
- `/etc/envoy/envoy-exit.yaml` → node `static-myhost-envoy-exit`

### Parsed Fields

From each listener + its referenced cluster:
- `listen_port` — from `listeners[].address.socket_address.port_value`
- `cluster` name — from `envoy.filters.network.tcp_proxy` filter config
- `backend_host`/`backend_port` — from `clusters[].load_assignment` endpoints
- `cluster_type` — mapped from STRICT_DNS/STATIC/LOGICAL_DNS to lowercase
- `connect_timeout_secs` — parsed from duration string (e.g. "5s")
- `proxy_protocol` — detected from transport_socket config (0/1/2)

### xDS Isolation

The xDS poll loop adds `&source=dynamic` to its route query, so static routes are never
pushed back to Envoy via xDS (Envoy already has them in its config file).

## Dependencies

- Reuses sigma-probe HTTP client pattern (SigmaClient with X-Api-Key auth)
- Requires sigma-api with /api/agent/* and /api/envoy-* endpoints
- Linux-only for system info collection (reads /proc filesystem)
- `iproute2` package required in Docker image (provides `ss` command for port scan)
- xDS: `tonic` 0.12, `prost` 0.13, `xds-api` 0.2 (pre-compiled Envoy proto bindings)
- Static config sync: `serde_yaml` 0.9 (parse envoy.yaml)
