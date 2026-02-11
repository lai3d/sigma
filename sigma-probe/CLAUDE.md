# sigma-probe — IP Reachability Probe

## What is this?

sigma-probe is a standalone Rust binary that runs on probe nodes (typically inside China) to actively check IP reachability of VPS instances managed by sigma. It periodically fetches the active VPS list from the sigma API, performs ICMP/TCP/HTTP checks against each IP, and reports results back via `POST /api/ip-checks`.

## Architecture

```
sigma-probe/
├── Cargo.toml
├── Dockerfile
├── CLAUDE.md
└── src/
    ├── main.rs          # Entry point: arg parsing, timed probe loop
    ├── config.rs        # Configuration: env vars + CLI flags (clap)
    ├── client.rs        # HTTP client (reuses sigma-cli pattern)
    ├── models.rs        # API types subset (Vps, IpEntry, CreateIpCheck)
    └── checker.rs       # ICMP / TCP / HTTP check implementations
```

## Configuration

| Env var | CLI flag | Default | Description |
|---------|----------|---------|-------------|
| `SIGMA_API_URL` | `--api-url` | `http://localhost:3000/api` | API base URL |
| `SIGMA_API_KEY` | `--api-key` | (none) | API key for auth |
| `PROBE_SOURCE` | `--source` | (required) | Probe location ID (e.g. `cn-beijing`) |
| `PROBE_INTERVAL` | `--interval` | `300` | Seconds between probe cycles |
| `PROBE_TYPES` | `--check-types` | `icmp,tcp` | Comma-separated check types |
| `PROBE_TCP_TIMEOUT` | `--tcp-timeout` | `5` | TCP connect timeout (seconds) |
| `PROBE_HTTP_TIMEOUT` | `--http-timeout` | `10` | HTTP request timeout (seconds) |
| `PROBE_CONCURRENCY` | `--concurrency` | `20` | Max concurrent checks |

## Check Types

- **icmp** — `surge-ping` ICMP echo, requires `CAP_NET_RAW`
- **tcp** — `TcpStream::connect` to `ip:ssh_port`
- **http** — `reqwest GET https://{ip}/` with invalid cert acceptance

## Build & Run

```bash
cargo build --release
./target/release/sigma-probe --source cn-beijing --api-url http://sigma-api:3000/api --api-key your-key

# Or via Docker Compose (from project root):
docker compose up -d probe
```

## Dependencies

- Reuses sigma-cli HTTP client pattern (`SigmaClient` with `X-Api-Key` auth)
- Requires sigma-api to be running with `/api/vps` and `/api/ip-checks` endpoints
- ICMP checks require `CAP_NET_RAW` (set via `cap_add: NET_RAW` in docker-compose)
