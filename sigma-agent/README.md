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

## Requirements

- Linux (reads `/proc` filesystem)
- sigma-api running with `/api/agent/*` endpoints
