# Σ Sigma — Project Context

## What is this?

Sigma is a lightweight VPS fleet management platform for a VPN infrastructure project. The operator manages VPS instances across **dozens of small cloud providers** in many countries. VPS turnover is high — instances typically last ~1 month before being replaced.

## Repo Structure

```
sigma/
├── sigma-api/          # Rust backend (Axum + SQLx + PostgreSQL)
├── sigma-web/          # React frontend (Vite + TypeScript + Tailwind CSS)
├── sigma-cli/          # Rust CLI client (clap + reqwest)
├── sigma-probe/        # IP reachability probe (deployed on China nodes)
├── sigma-agent/        # VPS system agent (auto-register + heartbeat + port scan + metrics)
├── k8s/                # Kubernetes deployment configs
├── docker-compose.yml  # Dev/staging orchestration (db + api + web + probe + agent)
├── Makefile            # Common commands
└── DEPLOYMENT.md       # Deployment guide
```

## Key Design Decisions

- **Rust + Axum + SQLx + PostgreSQL** — single binary deployment, low resource usage
- **React + Vite + TypeScript + Tailwind CSS** — professional web UI for non-tech operators
- **PostgreSQL** — leverages JSONB, TEXT[], GIN indexes for IP and tag queries
- **No ORM magic** — raw SQL via sqlx::query_as for full control and transparency
- **Runtime migrations** — using `sqlx::migrate::Migrator::new()` instead of compile-time `migrate!()` macro
- **JWT + API key dual auth** — email/password login with JWT (Bearer token), plus legacy API key (`X-Api-Key`) for CLI/agent. Three roles: admin, operator, readonly
- **Prometheus file_sd integration** — core integration point with Thanos/Prometheus/Grafana stack
- **Docker Compose (dev)** → **Kubernetes (production)**

## Data Model

- **Provider** — cloud platform vendor (DMIT, BandwagonHost, RackNerd, etc.)
- **VPS** — individual server instance with lifecycle: provisioning → active → retiring → retired
- **ip_checks** — table for tracking IP reachability from China (API done, probe: `sigma-probe/`)

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
- [x] VPS list: filterable table (status/purpose/provider/country/tag), agent online/offline indicator, retire/delete actions
- [x] VPS form: create/edit with dynamic IP list + label selector (color-coded), read-only agent info panel (heartbeat, CPU, RAM, disk, uptime, load avg)
- [x] Provider list + create/edit dialog
- [x] Settings page (API key config in localStorage)
- [x] Layout with sidebar navigation
- [x] Audit log page: filterable table (resource/action), expandable JSON details, admin-only
- [x] Dockerfile (Node 20 → nginx:alpine, API reverse proxy)

### Deployment — Done:
- [x] Root-level docker-compose.yml (db + api + web)
- [x] Kubernetes configs (k8s/ directory)
- [x] GitHub Actions CI/CD workflows (per-package: api, web, agent, probe)
- [x] Makefile with common commands
- [x] DEPLOYMENT.md guide

### Roadmap:
- [x] CLI client (`sigma-cli/` — Rust binary using clap + reqwest)
- [x] Bulk import/export (CSV/JSON)
- [x] IP reachability check API (`/api/ip-checks` — CRUD, summary, purge)
- [x] IP reachability probe (`sigma-probe/` — ICMP/TCP/HTTP checks from China nodes)
- [x] VPS agent auto-registration + heartbeat (`sigma-agent/` + `/api/agent/*` endpoints)
- [x] Pagination on list endpoints
- [x] Telegram/webhook notifications for expiring VPS
- [x] Ansible dynamic inventory output (`GET /api/ansible/inventory`)
- [x] Cost tracking and reporting per provider/country/month
- [x] User authentication & RBAC (email+password, JWT, admin/operator/readonly roles)
- [ ] Auto-deploy node_exporter on new VPS via SSH
- [x] OpenAPI/Swagger spec generation (`/swagger-ui`, `/api-docs/openapi.json`)
- [x] Rate limiting (Redis-based sliding window, per-IP)
- [x] Tests
- [x] TOTP MFA (two-factor authentication with Google Authenticator / Authy)
- [x] Audit log (who changed what — tracks all mutations with user, action, resource, details)
- [x] Ticket system (issue tracking with status workflow, comments, priority, VPS/provider links)
- [x] Agent port scanning + Prometheus metrics (`/metrics` endpoint with port usage by process)

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
- Auth: JWT in localStorage (`sigma_token`), AuthContext provides `useAuth()` hook, ProtectedRoute component for route guarding
- RBAC: mutating handlers require `admin` or `operator` role; user management requires `admin`; read endpoints open to all authenticated users

## Build & Run

```bash
# Full stack via Docker Compose (from project root)
docker compose up -d

# Access:
#   Web UI:  http://localhost
#   API:     http://localhost:3000/api/stats
#   DB:      localhost:5432 (sigma/sigma)
```
