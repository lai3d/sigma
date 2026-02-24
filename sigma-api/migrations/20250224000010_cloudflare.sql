-- Cloudflare DNS domain management (read-only sync)

CREATE TABLE cloudflare_accounts (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name        TEXT NOT NULL,
    api_token   TEXT NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE cloudflare_zones (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    account_id          UUID NOT NULL REFERENCES cloudflare_accounts(id) ON DELETE CASCADE,
    zone_id             TEXT NOT NULL,
    zone_name           TEXT NOT NULL,
    status              TEXT NOT NULL DEFAULT '',
    domain_expires_at   TIMESTAMPTZ,
    cert_expires_at     TIMESTAMPTZ,
    synced_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(account_id, zone_id)
);

CREATE TABLE cloudflare_dns_records (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    zone_uuid   UUID NOT NULL REFERENCES cloudflare_zones(id) ON DELETE CASCADE,
    record_id   TEXT NOT NULL,
    record_type TEXT NOT NULL,
    name        TEXT NOT NULL,
    content     TEXT NOT NULL,
    ttl         INTEGER NOT NULL DEFAULT 1,
    proxied     BOOLEAN NOT NULL DEFAULT false,
    vps_id      UUID REFERENCES vps(id) ON DELETE SET NULL,
    synced_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(zone_uuid, record_id)
);

CREATE INDEX idx_cf_zones_account ON cloudflare_zones(account_id);
CREATE INDEX idx_cf_dns_zone ON cloudflare_dns_records(zone_uuid);
CREATE INDEX idx_cf_dns_vps ON cloudflare_dns_records(vps_id);
CREATE INDEX idx_cf_dns_type ON cloudflare_dns_records(record_type);

CREATE TRIGGER trg_cf_accounts_updated
    BEFORE UPDATE ON cloudflare_accounts
    FOR EACH ROW EXECUTE FUNCTION update_updated_at();

CREATE TRIGGER trg_cf_zones_updated
    BEFORE UPDATE ON cloudflare_zones
    FOR EACH ROW EXECUTE FUNCTION update_updated_at();

CREATE TRIGGER trg_cf_dns_records_updated
    BEFORE UPDATE ON cloudflare_dns_records
    FOR EACH ROW EXECUTE FUNCTION update_updated_at();
