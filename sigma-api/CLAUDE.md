# Σ Sigma — Project Context

## What is this?

Sigma is a lightweight VPS fleet management platform for a VPN infrastructure project. The operator manages VPS instances across **dozens of small cloud providers** in many countries. VPS turnover is high — instances typically last ~1 month before being replaced.

## Repo Structure

```
sigma/
├── sigma-api/          # Rust backend (Axum + SQLx + PostgreSQL)
├── sigma-web/          # React frontend (Vite + TypeScript + Tailwind CSS)
├── k8s/                # Kubernetes deployment configs
├── docker-compose.yml  # Dev/staging orchestration (db + api + web)
├── Makefile            # Common commands
└── DEPLOYMENT.md       # Deployment guide
```

## Key Design Decisions

- **Rust + Axum + SQLx + PostgreSQL** — single binary deployment, low resource usage
- **React + Vite + TypeScript + Tailwind CSS** — professional web UI for non-tech operators
- **PostgreSQL** — leverages JSONB, TEXT[], GIN indexes for IP and tag queries
- **No ORM magic** — raw SQL via sqlx::query_as for full control and transparency
- **Runtime migrations** — using `sqlx::migrate::Migrator::new()` instead of compile-time `migrate!()` macro
- **Simple API key auth** — via `X-Api-Key` header, optional (disabled if API_KEY env not set)
- **Prometheus file_sd integration** — core integration point with Thanos/Prometheus/Grafana stack
- **Docker Compose (dev)** → **Kubernetes (production)**

## Data Model

- **Provider** — cloud platform vendor (DMIT, BandwagonHost, RackNerd, etc.)
- **VPS** — individual server instance with lifecycle: provisioning → active → retiring → retired
- **ip_checks** — table for tracking IP reachability from China (not yet implemented in API)

### VPS fields of note:
- `ip_addresses` — JSONB array of `{ip, label}` objects. Labels: `china-telecom`, `china-unicom`, `china-mobile`, `china-cernet`, `overseas`, `internal`, `anycast`
- `purpose` — enum: vpn-exit, vpn-relay, vpn-entry, monitor, management
- `tags` — TEXT[] array with GIN index, for flexible categorization (e.g. cn-optimized, iplc, cmhi)
- `extra` — JSONB for arbitrary metadata
- `monitoring_enabled` + `node_exporter_port` — controls whether this VPS appears in Prometheus targets

## Current State

### Backend (sigma-api) — Done:
- [x] Provider CRUD: `/api/providers`
- [x] VPS CRUD with filtering: `/api/vps` (filter by status, country, provider, purpose, tag, expiring_within_days)
- [x] IP addresses with carrier/type labels (JSONB: `{ip, label}`)
- [x] Quick retire endpoint: `POST /api/vps/{id}/retire`
- [x] Prometheus file_sd output: `GET /api/prometheus/targets`
- [x] Dashboard stats: `GET /api/stats`
- [x] Dockerfile (multi-stage, rust:latest → debian:bookworm-slim)

### Frontend (sigma-web) — Done:
- [x] Dashboard: stats cards, charts (by country/status/provider), expiring VPS table
- [x] VPS list: filterable table (status/purpose/provider/country/tag), retire/delete actions
- [x] VPS form: create/edit with dynamic IP list + label selector (color-coded)
- [x] Provider list + create/edit dialog
- [x] Settings page (API key config in localStorage)
- [x] Layout with sidebar navigation
- [x] Dockerfile (Node 20 → nginx:alpine, API reverse proxy)

### Deployment — Done:
- [x] Root-level docker-compose.yml (db + api + web)
- [x] Kubernetes configs (k8s/ directory)
- [x] GitHub Actions CI/CD workflow
- [x] Makefile with common commands
- [x] DEPLOYMENT.md guide

### Roadmap (not yet implemented):
- [ ] Telegram/webhook notifications for expiring VPS
- [x] CLI client (`sigma-cli/` — Rust binary using clap + reqwest)
- [x] Bulk import/export (CSV/JSON)
- [x] IP reachability check API (`/api/ip-checks` — CRUD, summary, purge)
- [ ] Ansible dynamic inventory output (`GET /api/ansible/inventory`)
- [ ] Cost tracking and reporting per provider/country/month
- [ ] Auto-deploy node_exporter on new VPS via SSH
- [ ] OpenAPI/Swagger spec generation
- [x] Pagination on list endpoints
- [ ] Rate limiting
- [ ] Tests

## Tech Stack

### Backend
- Rust 1.88+ (uses `rust:latest` in Docker)
- Axum 0.8, Tokio, SQLx 0.8 (with `json`, `rust_decimal` features)
- PostgreSQL 16

### Frontend
- Vite 7, React 19, TypeScript
- Tailwind CSS v4 (via `@tailwindcss/vite` plugin)
- React Router v7, TanStack React Query v5
- React Hook Form, Axios, Recharts, Lucide React

## Code Conventions

- Each route module exposes `pub fn router() -> Router<AppState>`
- Error handling via `AppError` enum → JSON error responses
- All handlers return `Result<Json<T>, AppError>`
- IP addresses stored as JSONB `[{ip, label}]`, validated server-side via `std::net::IpAddr`
- Partial updates: PUT endpoints fetch existing record, merge with provided fields
- Frontend: React Query hooks in `src/hooks/`, API layer in `src/api/`, types in `src/types/api.ts`

## Build & Run

```bash
# Full stack via Docker Compose (from project root)
docker compose up -d

# Access:
#   Web UI:  http://localhost
#   API:     http://localhost:3000/api/stats
#   DB:      localhost:5432 (sigma/sigma)
```
