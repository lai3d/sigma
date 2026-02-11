# Sigma Probe

IP reachability probe for [Sigma](../README.md) VPS fleet management. Deploys on probe nodes (typically inside China) to detect GFW IP blocking by actively checking VPS IPs via ICMP, TCP, and HTTP.

## How it works

1. **Fetch** — pulls all active VPS from `GET /api/vps?status=active` (paginated)
2. **Check** — for each IP, runs configured check types concurrently (semaphore-limited)
3. **Report** — posts each result to `POST /api/ip-checks` with source, latency, and success/fail
4. **Sleep** — waits for the configured interval, then repeats

## Install

```bash
# Build from source
cd sigma-probe && cargo build --release

# Or from project root
make probe
```

## Configuration

Config via environment variables or CLI flags (flags override env):

| Env var | CLI flag | Default | Description |
|---------|----------|---------|-------------|
| `SIGMA_API_URL` | `--api-url` | `http://localhost:3000/api` | API base URL |
| `SIGMA_API_KEY` | `--api-key` | — | API key for auth |
| `PROBE_SOURCE` | `--source` | (required) | Probe location ID (e.g. `cn-beijing`) |
| `PROBE_INTERVAL` | `--interval` | `300` | Seconds between probe cycles |
| `PROBE_TYPES` | `--check-types` | `icmp,tcp` | Comma-separated check types |
| `PROBE_TCP_TIMEOUT` | `--tcp-timeout` | `5` | TCP connect timeout (seconds) |
| `PROBE_HTTP_TIMEOUT` | `--http-timeout` | `10` | HTTP request timeout (seconds) |
| `PROBE_CONCURRENCY` | `--concurrency` | `20` | Max concurrent checks |

## Usage

```bash
# Run directly
sigma-probe --source cn-beijing --api-url http://sigma-api:3000/api --api-key your-key

# Via environment variables
export SIGMA_API_URL=http://sigma-api:3000/api
export SIGMA_API_KEY=your-key
sigma-probe --source cn-beijing --check-types icmp,tcp,http

# Via Docker Compose (from project root)
docker compose up -d probe
```

## Check types

| Type | Method | Success condition |
|------|--------|-------------------|
| `icmp` | `surge-ping` ICMP echo (3s timeout) | Got pong reply |
| `tcp` | `TcpStream::connect` to `ip:ssh_port` (5s timeout) | Connection established |
| `http` | `reqwest GET https://{ip}/` (10s timeout, accepts invalid certs) | Any HTTP response (including errors) |

## Requirements

- sigma-api running with `/api/vps` and `/api/ip-checks` endpoints
- `CAP_NET_RAW` for ICMP checks (set via `cap_add: NET_RAW` in docker-compose)
