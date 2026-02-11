# Σ Sigma

Lightweight VPS fleet management API with Prometheus/Thanos/Grafana integration. Built for high-turnover VPN infrastructure across multiple small cloud providers.

## Features

- **Provider management** — Track dozens of small cloud platforms with ratings and notes
- **VPS lifecycle** — Provisioning → Active → Retiring → Retired status flow
- **Prometheus integration** — Auto-generate `file_sd` targets with rich labels
- **Filtering** — Query VPS by status, country, provider, purpose, tags, expiring soon
- **Dashboard stats** — Aggregate counts by country, provider, status + expiring VPS list

## Quick Start

```bash
# 1. Start PostgreSQL
docker compose up -d db

# 2. Configure
cp .env.example .env
# Edit .env as needed

# 3. Build and run
cargo run

# Migrations run automatically on startup
```

## API Endpoints

### Providers

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/providers` | List all providers |
| POST | `/api/providers` | Create provider |
| GET | `/api/providers/{id}` | Get provider |
| PUT | `/api/providers/{id}` | Update provider |
| DELETE | `/api/providers/{id}` | Delete provider |

### VPS

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/vps` | List VPS (with filters) |
| POST | `/api/vps` | Create VPS |
| GET | `/api/vps/{id}` | Get VPS |
| PUT | `/api/vps/{id}` | Update VPS |
| DELETE | `/api/vps/{id}` | Delete VPS |
| POST | `/api/vps/{id}/retire` | Quick retire |

**List filters** (query params):
- `status` — provisioning, active, retiring, retired, suspended
- `country` — Country code (HK, US, JP, etc.)
- `provider_id` — Filter by provider UUID
- `purpose` — vpn-exit, vpn-relay, vpn-entry, monitor, management
- `tag` — Match VPS with this tag
- `expiring_within_days` — Show VPS expiring within N days

### Prometheus

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/prometheus/targets` | file_sd JSON output |

### Stats

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/stats` | Dashboard summary |

## Authentication

Set `API_KEY` env var to enable. Pass via `X-Api-Key` header.

```bash
curl -H "X-Api-Key: your-key" http://localhost:3000/api/vps
```

## Usage Examples

```bash
# Create a provider
curl -X POST http://localhost:3000/api/providers \
  -H "Content-Type: application/json" \
  -d '{
    "name": "DMIT",
    "country": "US",
    "website": "https://dmit.io",
    "rating": 4,
    "notes": "Good HK & LA nodes, CN optimized"
  }'

# Create a VPS
curl -X POST http://localhost:3000/api/vps \
  -H "Content-Type: application/json" \
  -d '{
    "hostname": "hk-exit-01",
    "alias": "Hong Kong Exit Node 1",
    "provider_id": "<uuid-from-above>",
    "ip_addresses": ["103.x.x.x"],
    "country": "HK",
    "city": "Hong Kong",
    "cpu_cores": 2,
    "ram_mb": 2048,
    "disk_gb": 40,
    "bandwidth_tb": 2.0,
    "cost_monthly": 15.99,
    "status": "active",
    "purchase_date": "2025-02-01",
    "expire_date": "2025-03-01",
    "purpose": "vpn-exit",
    "vpn_protocol": "wireguard",
    "tags": ["cn-optimized", "cmhi"]
  }'

# List active VPS expiring within 7 days
curl "http://localhost:3000/api/vps?status=active&expiring_within_days=7"

# Retire a VPS
curl -X POST http://localhost:3000/api/vps/<uuid>/retire

# Get Prometheus targets
curl http://localhost:3000/api/prometheus/targets
```

## Prometheus Integration

1. Set up the target sync script:

```bash
cp scripts/sync-targets.sh /usr/local/bin/
chmod +x /usr/local/bin/sync-targets.sh

# Cron every minute
echo "* * * * * root CMDB_URL=http://sigma:3000 OUTPUT_FILE=/etc/prometheus/targets/sigma.json /usr/local/bin/sync-targets.sh" >> /etc/crontab
```

2. Add to your `prometheus.yml` (see `scripts/prometheus-snippet.yml`).

3. Labels available in Grafana: `instance_name`, `provider`, `country`, `city`, `dc`, `purpose`, `vpn_protocol`, `tags`, `expire_date`, `status`.

## Building for Production

```bash
# Native build
cargo build --release

# Docker build
docker build -t sigma .

# The binary is statically-ish linked, ~10MB, runs anywhere
```

## Roadmap

- [ ] Telegram/webhook notifications for expiring VPS
- [ ] CLI client (`sigma-cli`)
- [ ] Bulk import/export (CSV/JSON)
- [ ] IP reachability check integration (China connectivity)
- [ ] Ansible inventory output (`/api/ansible/inventory`)
- [ ] Cost tracking dashboard
- [ ] Auto-deploy node_exporter on new VPS via SSH
