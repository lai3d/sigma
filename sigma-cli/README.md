# Sigma CLI

Terminal client for [Sigma](../README.md) VPS fleet management. Calls the REST API to manage providers, VPS instances, and view stats — useful for automation scripts, SSH-only environments, and shell piping.

## Install

```bash
# From the project root
make cli-install

# Or manually
cd sigma-cli && cargo build --release
cp target/release/sigma ~/.cargo/bin/
```

## Configuration

Config is loaded in layers (later overrides earlier):

1. **Config file** — `~/.config/sigma/config.toml`
2. **Environment variables** — `SIGMA_API_URL`, `SIGMA_API_KEY`
3. **CLI flags** — `--api-url`, `--api-key`

### Quick setup

```bash
sigma config set-url http://localhost:3000/api
sigma config set-key your-secret-key
```

This writes to `~/.config/sigma/config.toml`:

```toml
api_url = "http://localhost:3000/api"
api_key = "your-secret-key"
```

## Usage

### Providers

```bash
sigma providers list
sigma providers list --page 2 --per-page 10
sigma providers get <UUID>
sigma providers create --name "DMIT" --country US --website https://dmit.io
sigma providers update <UUID> --rating 5 --notes "Great provider"
sigma providers delete <UUID>

# Export / Import
sigma providers export --format csv -o providers.csv
sigma providers import providers.json --format json
```

### VPS

```bash
sigma vps list
sigma vps list --status active --country HK --purpose vpn-exit
sigma vps list --tag cn-optimized --expiring 7
sigma vps get <UUID>

sigma vps create \
  --hostname hk-exit-01 \
  --provider-id <UUID> \
  --ip 103.1.2.3:china-telecom \
  --ip 10.0.0.1:internal \
  --country HK \
  --status active \
  --purpose vpn-exit \
  --tag cn-optimized --tag cmhi \
  --cost-monthly 15.99 \
  --expire-date 2025-03-01

sigma vps update <UUID> --status retiring --notes "Scheduled for replacement"
sigma vps retire <UUID>    # Quick retire: sets status=retired, disables monitoring
sigma vps delete <UUID>

# Export / Import
sigma vps export --format csv -o vps.csv
sigma vps import vps.json --format json
```

### IP Checks

```bash
sigma ip-checks list
sigma ip-checks list --vps-id <UUID> --success true
sigma ip-checks list --ip 103.1.2.3 --check-type icmp --source cn-beijing
sigma ip-checks get <UUID>

sigma ip-checks create \
  --vps-id <UUID> \
  --ip 103.1.2.3 \
  --success true \
  --check-type icmp \
  --source cn-beijing \
  --latency-ms 45

sigma ip-checks delete <UUID>

# Aggregated summary per VPS/IP (success rate, avg latency, last status)
sigma ip-checks summary
sigma ip-checks summary --vps-id <UUID>

# Purge old checks
sigma ip-checks purge --older-than-days 30
```

### Stats

```bash
sigma stats
```

### JSON output

Add `--json` to any command for raw JSON (pipe-friendly):

```bash
sigma vps list --status active --json | jq '.[].hostname'
sigma providers list --json | jq '.[] | select(.country == "US")'
sigma stats --json
```

### IP address format

IPs accept an optional label after a colon:

```
--ip 103.1.2.3                    # no label
--ip 103.1.2.3:china-telecom      # with label
--ip 10.0.0.1:internal            # with label
```

Valid labels: `china-telecom`, `china-unicom`, `china-mobile`, `china-cernet`, `overseas`, `internal`, `anycast`

## Shell examples

```bash
# List hostnames of all active VPS in Hong Kong
sigma vps list --status active --country HK --json | jq -r '.[].hostname'

# Count VPS by status
sigma stats --json | jq '.by_status[] | "\(.label): \(.count)"'

# Backup all providers to CSV
sigma providers export --format csv -o "providers_$(date +%Y%m%d).csv"

# Retire all VPS expiring within 3 days
sigma vps list --expiring 3 --json | jq -r '.[].id' | xargs -I{} sigma vps retire {}
```
