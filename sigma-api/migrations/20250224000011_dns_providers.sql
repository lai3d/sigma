-- Multi-provider DNS: rename CF-specific tables to generic DNS tables

-- Rename tables
ALTER TABLE cloudflare_accounts RENAME TO dns_accounts;
ALTER TABLE cloudflare_zones RENAME TO dns_zones;
ALTER TABLE cloudflare_dns_records RENAME TO dns_records;

-- Add provider_type to accounts
ALTER TABLE dns_accounts ADD COLUMN provider_type TEXT NOT NULL DEFAULT 'cloudflare';
-- Replace single api_token with flexible JSONB config
ALTER TABLE dns_accounts ADD COLUMN config JSONB NOT NULL DEFAULT '{}';
-- Migrate existing tokens into config
UPDATE dns_accounts SET config = jsonb_build_object('api_token', api_token);
ALTER TABLE dns_accounts DROP COLUMN api_token;

-- Add extra JSONB to records for provider-specific fields (e.g. CF proxied)
ALTER TABLE dns_records ADD COLUMN extra JSONB NOT NULL DEFAULT '{}';
-- Migrate proxied into extra
UPDATE dns_records SET extra = jsonb_build_object('proxied', proxied);
ALTER TABLE dns_records DROP COLUMN proxied;

-- Rename indexes
ALTER INDEX idx_cf_zones_account RENAME TO idx_dns_zones_account;
ALTER INDEX idx_cf_dns_zone RENAME TO idx_dns_records_zone;
ALTER INDEX idx_cf_dns_vps RENAME TO idx_dns_records_vps;
ALTER INDEX idx_cf_dns_type RENAME TO idx_dns_records_type;

-- Rename triggers
DROP TRIGGER trg_cf_accounts_updated ON dns_accounts;
DROP TRIGGER trg_cf_zones_updated ON dns_zones;
DROP TRIGGER trg_cf_dns_records_updated ON dns_records;
CREATE TRIGGER trg_dns_accounts_updated BEFORE UPDATE ON dns_accounts FOR EACH ROW EXECUTE FUNCTION update_updated_at();
CREATE TRIGGER trg_dns_zones_updated BEFORE UPDATE ON dns_zones FOR EACH ROW EXECUTE FUNCTION update_updated_at();
CREATE TRIGGER trg_dns_records_updated BEFORE UPDATE ON dns_records FOR EACH ROW EXECUTE FUNCTION update_updated_at();

-- Rename unique constraints
ALTER TABLE dns_zones RENAME CONSTRAINT cloudflare_zones_account_id_zone_id_key TO dns_zones_account_id_zone_id_key;
ALTER TABLE dns_records RENAME CONSTRAINT cloudflare_dns_records_zone_uuid_record_id_key TO dns_records_zone_uuid_record_id_key;
