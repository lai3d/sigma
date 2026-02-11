# Σ Sigma — Project Context

## What is this?

Sigma is a lightweight VPS fleet management API for a VPN infrastructure project. The operator manages VPS instances across **dozens of small cloud providers** in many countries. VPS turnover is high — instances typically last ~1 month before being replaced.

## Key Design Decisions

- **Rust + Axum + SQLx + PostgreSQL** — single binary deployment, low resource usage, can run on any VPS
- **PostgreSQL over SQLite** — leverages INET[], TEXT[], JSONB, GIN indexes for IP and tag queries
- **No ORM magic** — raw SQL via sqlx::query_as for full control and transparency
- **Runtime migrations** — using `sqlx::migrate::Migrator::new()` instead of compile-time `migrate!()` macro to avoid needing DATABASE_URL at build time
- **Simple API key auth** — via `X-Api-Key` header, optional (disabled if API_KEY env not set)
- **Prometheus file_sd integration** — the core integration point with the existing Thanos/Prometheus/Grafana observability stack

## Architecture

```
Sigma API (this project)
  ↓ GET /api/prometheus/targets
sync-targets.sh (cron every 60s)
  ↓ writes file_sd JSON
Prometheus (per-region instances)
  ↓ remote_write
Thanos → Grafana
```

## Data Model

- **Provider** — cloud platform vendor (DMIT, BandwagonHost, RackNerd, etc.)
- **VPS** — individual server instance with lifecycle: provisioning → active → retiring → retired
- **ip_checks** — optional table for tracking IP reachability from China (not yet implemented in API)

### VPS fields of note:
- `ip_addresses` — PostgreSQL INET[] array, accepts multiple IPs per VPS
- `purpose` — enum: vpn-exit, vpn-relay, vpn-entry, monitor, management
- `tags` — TEXT[] array with GIN index, for flexible categorization (e.g. cn-optimized, iplc, cmhi)
- `extra` — JSONB for arbitrary metadata
- `monitoring_enabled` + `node_exporter_port` — controls whether this VPS appears in Prometheus targets

## Current State (MVP)

### Implemented:
- Provider CRUD: `/api/providers`
- VPS CRUD with filtering: `/api/vps` (filter by status, country, provider, purpose, tag, expiring_within_days)
- Quick retire endpoint: `POST /api/vps/{id}/retire`
- Prometheus file_sd output: `GET /api/prometheus/targets`
- Dashboard stats: `GET /api/stats`
- Docker Compose for PG + Dockerfile for the API

### Not yet implemented (roadmap):
- [ ] Telegram/webhook notifications for expiring VPS
- [ ] CLI client (could be a separate binary in the same workspace, or a shell script wrapping curl)
- [ ] Bulk import/export (CSV/JSON)
- [ ] IP reachability check API (ip_checks table exists but no routes yet)
- [ ] Ansible dynamic inventory output (`GET /api/ansible/inventory`)
- [ ] Cost tracking and reporting per provider/country/month
- [ ] Auto-deploy node_exporter on new VPS via SSH
- [ ] Web UI (separate frontend, or could be a simple SPA served by the API)
- [ ] OpenAPI/Swagger spec generation
- [ ] Pagination on list endpoints
- [ ] Rate limiting

## Code Style & Conventions

- Each route module (providers, vps, prometheus, stats) exposes a `pub fn router() -> Router<AppState>`
- Error handling via `AppError` enum → auto-converts to JSON error responses
- All handlers return `Result<Json<T>, AppError>`
- IP addresses accepted as strings in API input (e.g. "103.1.2.3"), auto-parsed to IpNetwork with /32 default
- Partial updates: PUT endpoints fetch existing record, merge with provided fields (Option<T> = None means keep existing)
- Database: no compile-time query checking (would need SQLX_OFFLINE mode), just runtime

## Build & Run

```bash
docker compose up -d db          # start PostgreSQL
cp .env.example .env             # configure
cargo run                        # migrations auto-apply on startup
```

## Testing

No tests yet. When adding tests:
- Use sqlx's test fixtures or a dedicated test database
- Integration tests against a real PG instance (not mocks)
- Test the Prometheus target output format carefully — it must be valid file_sd JSON
