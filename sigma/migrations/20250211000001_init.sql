-- providers: cloud/hosting vendors
CREATE TABLE providers (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name        TEXT NOT NULL UNIQUE,
    country     TEXT NOT NULL DEFAULT '',
    website     TEXT NOT NULL DEFAULT '',
    panel_url   TEXT NOT NULL DEFAULT '',
    api_supported BOOLEAN NOT NULL DEFAULT FALSE,
    rating      SMALLINT CHECK (rating BETWEEN 1 AND 5),
    notes       TEXT NOT NULL DEFAULT '',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- vps instances
CREATE TABLE vps (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    hostname        TEXT NOT NULL,
    alias           TEXT NOT NULL DEFAULT '',
    provider_id     UUID NOT NULL REFERENCES providers(id) ON DELETE RESTRICT,

    -- networking
    ip_addresses    INET[] NOT NULL DEFAULT '{}',
    ssh_port        INT NOT NULL DEFAULT 22,

    -- location
    country         TEXT NOT NULL DEFAULT '',
    city            TEXT NOT NULL DEFAULT '',
    dc_name         TEXT NOT NULL DEFAULT '',

    -- specs
    cpu_cores       SMALLINT,
    ram_mb          INT,
    disk_gb         INT,
    bandwidth_tb    NUMERIC(10,2),

    -- cost
    cost_monthly    NUMERIC(10,2),
    currency        TEXT NOT NULL DEFAULT 'USD',

    -- lifecycle
    status          TEXT NOT NULL DEFAULT 'provisioning'
                    CHECK (status IN ('provisioning', 'active', 'retiring', 'retired', 'suspended')),
    purchase_date   DATE,
    expire_date     DATE,

    -- purpose
    purpose         TEXT NOT NULL DEFAULT ''
                    CHECK (purpose IN ('', 'vpn-exit', 'vpn-relay', 'vpn-entry', 'monitor', 'management', 'other')),
    vpn_protocol    TEXT NOT NULL DEFAULT '',
    tags            TEXT[] NOT NULL DEFAULT '{}',

    -- monitoring
    monitoring_enabled  BOOLEAN NOT NULL DEFAULT TRUE,
    node_exporter_port  INT NOT NULL DEFAULT 9100,

    -- metadata
    extra           JSONB NOT NULL DEFAULT '{}',
    notes           TEXT NOT NULL DEFAULT '',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_vps_provider ON vps(provider_id);
CREATE INDEX idx_vps_status ON vps(status);
CREATE INDEX idx_vps_country ON vps(country);
CREATE INDEX idx_vps_expire ON vps(expire_date);
CREATE INDEX idx_vps_tags ON vps USING GIN(tags);

-- ip quality tracking (optional, for CN reachability)
CREATE TABLE ip_checks (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    vps_id      UUID NOT NULL REFERENCES vps(id) ON DELETE CASCADE,
    ip          INET NOT NULL,
    check_type  TEXT NOT NULL DEFAULT 'icmp',  -- icmp, tcp, http
    source      TEXT NOT NULL DEFAULT '',       -- cn-beijing, cn-shanghai, etc.
    success     BOOLEAN NOT NULL,
    latency_ms  INT,
    checked_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_ip_checks_vps ON ip_checks(vps_id);
CREATE INDEX idx_ip_checks_time ON ip_checks(checked_at DESC);

-- auto-update updated_at
CREATE OR REPLACE FUNCTION update_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_providers_updated
    BEFORE UPDATE ON providers
    FOR EACH ROW EXECUTE FUNCTION update_updated_at();

CREATE TRIGGER trg_vps_updated
    BEFORE UPDATE ON vps
    FOR EACH ROW EXECUTE FUNCTION update_updated_at();
