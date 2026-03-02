-- System-wide key/value settings (runtime-configurable)
CREATE TABLE IF NOT EXISTS system_settings (
    key        TEXT PRIMARY KEY,
    value      TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Seed default DNS sync interval (1 hour)
INSERT INTO system_settings (key, value) VALUES ('dns_sync_interval_secs', '3600')
ON CONFLICT (key) DO NOTHING;
