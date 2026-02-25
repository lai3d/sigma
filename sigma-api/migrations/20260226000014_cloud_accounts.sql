-- Cloud provider accounts for auto-syncing VPS instances
CREATE TABLE cloud_accounts (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name            TEXT NOT NULL,
    provider_type   TEXT NOT NULL,          -- 'aws' | 'alibaba'
    config          JSONB NOT NULL DEFAULT '{}',
    last_synced_at  TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TRIGGER trg_cloud_accounts_updated
    BEFORE UPDATE ON cloud_accounts
    FOR EACH ROW EXECUTE FUNCTION update_updated_at();

-- Track VPS origin: manual (default), agent, cloud-sync
ALTER TABLE vps ADD COLUMN source TEXT NOT NULL DEFAULT 'manual';
ALTER TABLE vps ADD COLUMN cloud_account_id UUID REFERENCES cloud_accounts(id) ON DELETE SET NULL;
CREATE INDEX idx_vps_source ON vps(source);
CREATE INDEX idx_vps_cloud_account ON vps(cloud_account_id);
