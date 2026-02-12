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
    ├── main.rs      # Entry point: register + heartbeat loop
    ├── config.rs    # Configuration: env vars + CLI flags (clap)
    ├── client.rs    # HTTP client (reuses sigma-probe pattern)
    ├── models.rs    # API request/response types
    └── system.rs    # Linux system info collection (/proc, statvfs)
```

## Configuration

| Env var | CLI flag | Default | Description |
|---------|----------|---------|-------------|
| `SIGMA_API_URL` | `--api-url` | `http://localhost:3000/api` | API base URL |
| `SIGMA_API_KEY` | `--api-key` | (none) | API key for auth |
| `AGENT_INTERVAL` | `--interval` | `60` | Heartbeat interval (seconds) |
| `AGENT_HOSTNAME` | `--hostname` | (auto-detect) | Override hostname |
| `AGENT_SSH_PORT` | `--ssh-port` | `22` | SSH port to report |

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

# Or via Docker:
docker run -d --name sigma-agent \
  -e SIGMA_API_URL=http://sigma-api:3000/api \
  -e SIGMA_API_KEY=your-key \
  sigma-agent
```

## Dependencies

- Reuses sigma-probe HTTP client pattern (SigmaClient with X-Api-Key auth)
- Requires sigma-api with /api/agent/* endpoints
- Linux-only for system info collection (reads /proc filesystem)
