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
├── ebpf-programs/         # Pre-compiled eBPF bytecode (populated by Docker build)
└── src/
    ├── main.rs            # Entry point: register + heartbeat loop + spawn scan/metrics/xds/config-sync/ebpf
    ├── config.rs          # Configuration: env vars + CLI flags (clap)
    ├── client.rs          # HTTP client (GET + POST with X-Api-Key auth)
    ├── models.rs          # API request/response types
    ├── system.rs          # Linux system info collection (/proc, statvfs)
    ├── port_scan.rs       # Port scanning: TcpListener::bind test + ss parsing
    ├── metrics.rs         # Prometheus metrics HTTP server (/metrics)
    ├── ebpf_traffic.rs    # eBPF traffic monitoring: loader, harvester, container resolution (feature-gated)
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
| `AGENT_ENVOY_CONFIG_PATH` | `--envoy-config-path` | `/etc/envoy/envoy.yaml` | Config paths (comma-separated, supports glob) |
| `AGENT_ENVOY_CONFIG_SYNC` | `--envoy-config-sync` | `false` | Enable static config sync |
| `AGENT_ENVOY_CONFIG_SYNC_INTERVAL` | `--envoy-config-sync-interval` | `60` | File poll interval (seconds) |
| `AGENT_ENVOY_CONFIG_EXCLUDE` | `--envoy-config-exclude` | (none) | Glob pattern to exclude files (e.g. `*dynamic*`) |
| `AGENT_HOST_PROC` | `--host-proc` | `/proc` | Host /proc mount path for process attribution |
| `AGENT_EBPF_TRAFFIC` | `--ebpf-traffic` | `false` | Enable eBPF TCP traffic monitoring |
| `AGENT_EBPF_TRAFFIC_INTERVAL` | `--ebpf-traffic-interval` | `30` | eBPF traffic stats collection interval (seconds) |
| `AGENT_EBPF_TRAFFIC_MAX_ENTRIES` | `--ebpf-traffic-max-entries` | `8192` | BPF map max entries (unique PIDs) |

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
(envoy, sshd, nginx, node_exporter, xray, other, unknown).

Process attribution uses a three-tier strategy:
1. `ss -tulnp` — standard, works if container has direct `/proc` access
2. `nsenter -t 1 -m -- ss -tulnp` — runs ss in host mount namespace (needs `SYS_ADMIN` or `privileged`)
3. Direct `/proc/<pid>/fd/` scanning — works with host-mounted `/proc` (needs `SYS_PTRACE`)

Classification is case-insensitive: both `xray` and `XrayR` map to the `xray` category.

Results are exposed via a Prometheus-compatible `/metrics` endpoint on `--metrics-port`:
- `sigma_ports_total` — total ports in scan range
- `sigma_ports_available` — free ports
- `sigma_ports_in_use` — occupied ports in scan range (LISTEN + TIME_WAIT + other states; `total - available`)
- `sigma_ports_used{source="..."}` — LISTEN ports by process category (system-wide)
- `sigma_ports_other_detail{process="..."}` — breakdown of "other" category by actual process name
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
  privileged: true      # Full /proc/<pid>/fd access for process attribution
  volumes:
    - /proc:/host/proc:ro  # Host proc mount (fallback for process resolution)
  environment:
    AGENT_HOST_PROC: /host/proc
```

**Why `privileged: true`**: Docker's default seccomp/AppArmor profiles block reading
`/proc/<pid>/fd/` for host processes even with `SYS_PTRACE`. Without it, most ports show
as "unknown". The `privileged` flag removes these restrictions so `ss -tulnp` can fully
resolve process names. Alternative (less permissive): `cap_add: [SYS_PTRACE, SYS_ADMIN]`
with `security_opt: [apparmor:unconfined]`.

**Docker Desktop limitation**: The `allocate-ports` API proxy won't work locally because
containers on bridge network can't reach the agent on host network. This works in production
where API and agent run on different machines connected via public IPs.

## Grafana Dashboard

A pre-built dashboard is available at `grafana/dashboards/port-scan.json` with:
- Stat cards (total/available/in-use ports, envoy count, utilization %, scan duration)
- Time series charts (available vs in-use over time, LISTEN ports by source)
- Donut chart (LISTEN port breakdown by source)
- Host summary table (total/available/in-use/envoy/xray/nginx per host, usage % gauge)

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

Supports glob patterns for many Envoy static config files. Use `--envoy-config-exclude`
to skip the dynamic (xDS-managed) Envoy config:

```bash
AGENT_ENVOY_CONFIG_PATH=/envoy-configs/layer4*.yaml
AGENT_ENVOY_CONFIG_EXCLUDE=*dynamic*
```

Each matched file creates a separate envoy node in sigma:
- `layer4.yaml` → node `static-myhost-layer4`
- `layer4-01.yaml` → node `static-myhost-layer4-01`
- `layer4-shield-01.yaml` → node `static-myhost-layer4-shield-01`
- `layer4-dynamic.yaml` → excluded by `*dynamic*` pattern

Also supports comma-separated literal paths:
```bash
AGENT_ENVOY_CONFIG_PATH=/etc/envoy/envoy.yaml,/etc/envoy/envoy-relay.yaml
```

Infrastructure clusters (`xds_cluster`, `sigma_xds`, `*_xds`) are automatically
skipped during parsing.

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

## eBPF Traffic Monitoring

When `--ebpf-traffic` is enabled, the agent uses eBPF kprobes to monitor TCP and UDP activity per process.
This includes TCP bytes sent/received (`tcp_sendmsg`/`tcp_recvmsg`), UDP bytes sent/received
(`udp_sendmsg`/`udp_recvmsg`), retransmit events (`tcp_retransmit_skb`), connection tracking
(`tcp_v4_connect`/`tcp_close`/`inet_csk_accept`), and TCP RTT/latency tracking
(`tcp_rcv_established` — reads `srtt_us` from `tcp_sock` via `bpf_probe_read_kernel`).
This is feature-gated behind the `ebpf-traffic` cargo feature (compiled in via Docker by default).

### Configuration

| Env var | CLI flag | Default | Description |
|---------|----------|---------|-------------|
| `AGENT_EBPF_TRAFFIC` | `--ebpf-traffic` | `false` | Enable eBPF traffic monitoring |
| `AGENT_EBPF_TRAFFIC_INTERVAL` | `--ebpf-traffic-interval` | `30` | Stats collection interval (seconds) |
| `AGENT_EBPF_TRAFFIC_MAX_ENTRIES` | `--ebpf-traffic-max-entries` | `8192` | BPF map max entries (unique PIDs) |

### Prometheus Metrics

Exposed on the existing `/metrics` endpoint when enabled:

```
# HELP sigma_traffic_bytes_sent_total TCP bytes sent by process (eBPF)
# TYPE sigma_traffic_bytes_sent_total gauge
sigma_traffic_bytes_sent_total{hostname="relay-01",process="envoy",container=""} 1234567
sigma_traffic_bytes_sent_total{hostname="relay-01",process="xray",container="abc123def456"} 987654

# HELP sigma_traffic_bytes_recv_total TCP bytes received by process (eBPF)
# TYPE sigma_traffic_bytes_recv_total gauge
sigma_traffic_bytes_recv_total{hostname="relay-01",process="envoy",container=""} 2345678

# HELP sigma_traffic_udp_bytes_sent_total UDP bytes sent by process (eBPF)
# TYPE sigma_traffic_udp_bytes_sent_total gauge
sigma_traffic_udp_bytes_sent_total{hostname="relay-01",process="xray",container="abc123def456"} 5678901
sigma_traffic_udp_bytes_sent_total{hostname="relay-01",process="wireguard",container=""} 3456789

# HELP sigma_traffic_udp_bytes_recv_total UDP bytes received by process (eBPF)
# TYPE sigma_traffic_udp_bytes_recv_total gauge
sigma_traffic_udp_bytes_recv_total{hostname="relay-01",process="xray",container="abc123def456"} 4567890
sigma_traffic_udp_bytes_recv_total{hostname="relay-01",process="wireguard",container=""} 2345678

# HELP sigma_tcp_retransmits_total TCP retransmit events by process (eBPF)
# TYPE sigma_tcp_retransmits_total gauge
sigma_tcp_retransmits_total{hostname="relay-01",process="envoy",container=""} 42

# HELP sigma_tcp_connections_active Current active TCP connections by process (eBPF)
# TYPE sigma_tcp_connections_active gauge
sigma_tcp_connections_active{hostname="relay-01",process="envoy",container=""} 15

# HELP sigma_tcp_connections_total Total TCP connections opened by process (eBPF)
# TYPE sigma_tcp_connections_total counter
sigma_tcp_connections_total{hostname="relay-01",process="envoy",container=""} 1234

# HELP sigma_tcp_rtt_avg_us Average TCP round-trip time in microseconds by process (eBPF)
# TYPE sigma_tcp_rtt_avg_us gauge
sigma_tcp_rtt_avg_us{hostname="relay-01",process="envoy",container=""} 12500

# HELP sigma_tcp_rtt_min_us Minimum TCP round-trip time in microseconds by process (eBPF)
# TYPE sigma_tcp_rtt_min_us gauge
sigma_tcp_rtt_min_us{hostname="relay-01",process="envoy",container=""} 800

# HELP sigma_tcp_rtt_max_us Maximum TCP round-trip time in microseconds by process (eBPF)
# TYPE sigma_tcp_rtt_max_us gauge
sigma_tcp_rtt_max_us{hostname="relay-01",process="envoy",container=""} 95000
```

RTT metrics are only emitted for processes with active TCP RTT data. The `srtt_us` field offset
within `tcp_sock` is defined as `SRTT_US_OFFSET` (744 for Linux 6.x x86_64) and may need
adjustment for different kernel versions. If the read fails, the probe safely returns without
updating — no crash, just missing RTT data.

Labels: `process` = resolved from `/proc/<pid>/comm`, `container` = Docker/containerd ID (first 12 hex chars) or empty.

### Requirements

- **Kernel**: Linux 5.10+ with BTF (BPF Type Format) support
- **Docker**: `privileged: true`, `pid: host`, `network_mode: host`
- **Host proc**: mount `/proc:/host/proc:ro` and set `AGENT_HOST_PROC=/host/proc`

### Crate Structure

```
sigma-agent-ebpf-common/   # Shared #[repr(C)] types (no_std, no deps)
sigma-agent-ebpf/           # eBPF kernel programs (nightly, bpfel-unknown-none target)
sigma-agent/src/ebpf_traffic.rs  # Userspace: loader, harvester, container ID resolution
```

The Dockerfile uses a multi-stage build: nightly toolchain compiles eBPF programs to BPF bytecode,
then stable toolchain builds the agent with `--features ebpf-traffic`, embedding the bytecode via
`include_bytes!()`.

### Graceful Degradation

If the kernel doesn't support BTF or eBPF programs fail to load, the agent logs a warning and
continues without traffic metrics. The feature is fully optional — building without `--features
ebpf-traffic` produces a binary with zero eBPF dependencies.

## Dependencies

- Reuses sigma-probe HTTP client pattern (SigmaClient with X-Api-Key auth)
- Requires sigma-api with /api/agent/* and /api/envoy-* endpoints
- Linux-only for system info collection (reads /proc filesystem)
- `iproute2` package required in Docker image (provides `ss` command for port scan)
- xDS: `tonic` 0.12, `prost` 0.13, `xds-api` 0.2 (pre-compiled Envoy proto bindings)
- Static config sync: `serde_yaml` 0.9 (parse envoy.yaml)
- eBPF traffic (optional): `aya` 0.13, `aya-log` 0.2, `aya-ebpf` 0.1 (kernel programs)
