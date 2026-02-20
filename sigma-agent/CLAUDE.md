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
    ├── main.rs        # Entry point: register + heartbeat loop + spawn scan/metrics
    ├── config.rs      # Configuration: env vars + CLI flags (clap)
    ├── client.rs      # HTTP client (reuses sigma-probe pattern)
    ├── models.rs      # API request/response types
    ├── system.rs      # Linux system info collection (/proc, statvfs)
    ├── port_scan.rs   # Port scanning: TcpListener::bind test + ss parsing
    └── metrics.rs     # Prometheus metrics HTTP server (/metrics)
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

## Dependencies

- Reuses sigma-probe HTTP client pattern (SigmaClient with X-Api-Key auth)
- Requires sigma-api with /api/agent/* endpoints
- Linux-only for system info collection (reads /proc filesystem)
- `iproute2` package required in Docker image (provides `ss` command for port scan)
