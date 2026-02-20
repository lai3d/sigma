# Σ Sigma

Lightweight VPS fleet management platform for high-turnover VPN infrastructure. Track instances across dozens of small cloud providers, manage IP addresses with carrier labels, and integrate with Prometheus/Grafana for monitoring.

## Features

- **Provider management** — Track cloud platforms with ratings and notes
- **VPS lifecycle** — Provisioning → Active → Retiring → Retired status flow
- **Multi-IP with labels** — Each VPS can have multiple IPs labeled by carrier (电信/联通/移动/教育网/海外/内网/Anycast)
- **Filtering** — Query VPS by status, country, provider, purpose, tags, expiring within N days
- **Prometheus file_sd** — Auto-generate targets with rich labels for Thanos/Prometheus/Grafana
- **Dashboard** — Stats cards, charts by country/status/provider, expiring VPS alerts
- **Web UI** — Full CRUD for providers and VPS, settings page, responsive sidebar layout

## Tech Stack

| Layer | Stack |
|-------|-------|
| Backend | Rust, Axum 0.8, SQLx 0.8, PostgreSQL 16 |
| Frontend | React 19, Vite 7, TypeScript, Tailwind CSS v4, React Query v5 |
| Infra | Docker Compose (dev), Kubernetes + ArgoCD (prod), GitHub Actions (CI) |

## Quick Start

```bash
# Clone and configure
git clone https://github.com/lai3d/sigma.git
cd sigma
cp .env.example .env    # Edit API_KEY if needed

# Start all services
docker compose up -d

# Check status
docker compose ps
```

| Service | URL |
|---------|-----|
| Web UI | http://localhost |
| API | http://localhost:3000/api |
| PostgreSQL | localhost:5432 |

## Project Structure

```
sigma/
├── sigma-api/          # Rust backend (Axum + SQLx + PostgreSQL)
├── sigma-web/          # React frontend (Vite + TypeScript + Tailwind CSS)
├── sigma-cli/          # Rust CLI client (clap + reqwest)
├── sigma-probe/        # IP reachability probe (deployed on China nodes)
├── sigma-agent/        # VPS system agent (auto-register + heartbeat)
├── k8s/                # Kubernetes manifests (ArgoCD-managed)
├── .github/workflows/  # CI: build & push images to GHCR
├── docker-compose.yml  # Local dev orchestration
├── Makefile            # Common commands (make help)
└── DEPLOYMENT.md       # Deployment guide
```

## Make Commands

```bash
make help          # Show all available commands
make dev           # Start dev environment
make logs          # Tail all logs
make logs-api      # Tail API logs
make db-shell      # Open PostgreSQL shell
make db-backup     # Backup database
make test-api      # Health check API
```

## API Overview

### Authentication

Set `API_KEY` env var to enable. Pass via `X-Api-Key` header. If unset, auth is disabled.

### Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/stats` | Dashboard summary |
| GET/POST | `/api/providers` | List / Create provider |
| GET/PUT/DELETE | `/api/providers/{id}` | Get / Update / Delete provider |
| GET/POST | `/api/vps` | List (with filters) / Create VPS |
| GET/PUT/DELETE | `/api/vps/{id}` | Get / Update / Delete VPS |
| POST | `/api/vps/{id}/retire` | Quick retire |
| GET | `/api/prometheus/targets` | Prometheus file_sd JSON |
| POST | `/api/agent/register` | Agent self-registration |
| POST | `/api/agent/heartbeat` | Agent heartbeat with system info |

### VPS Filters (query params)

`status`, `country`, `provider_id`, `purpose`, `tag`, `expiring_within_days`

### Example

```bash
# Create a provider
curl -X POST http://localhost:3000/api/providers \
  -H "Content-Type: application/json" \
  -d '{"name": "Acme Cloud", "country": "US", "website": "https://example.com", "rating": 4}'

# Create a VPS with labeled IPs
curl -X POST http://localhost:3000/api/vps \
  -H "Content-Type: application/json" \
  -d '{
    "hostname": "hk-relay-01",
    "provider_id": "<uuid>",
    "ip_addresses": [
      {"ip": "103.1.2.3", "label": "china-telecom"},
      {"ip": "10.0.0.1", "label": "internal"}
    ],
    "country": "HK",
    "status": "active",
    "purpose": "vpn-relay",
    "tags": ["optimized", "premium"]
  }'

# List active VPS expiring within 7 days
curl "http://localhost:3000/api/vps?status=active&expiring_within_days=7"

# Prometheus targets
curl http://localhost:3000/api/prometheus/targets
```

## Deployment

- **Local/Dev**: `docker compose up -d`
- **Production**: Kubernetes via ArgoCD (GitOps, pull-based)
- **CI**: GitHub Actions builds and pushes images to `ghcr.io/lai3d/sigma/{api,web}`

See [DEPLOYMENT.md](DEPLOYMENT.md) for full guide including ArgoCD setup.

## License

MIT
