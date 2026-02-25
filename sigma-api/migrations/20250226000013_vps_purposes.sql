-- VPS Purpose types (dynamic, DB-driven)
CREATE TABLE vps_purposes (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name       TEXT NOT NULL UNIQUE,
    label      TEXT NOT NULL,
    color      TEXT NOT NULL DEFAULT 'gray',
    sort_order INT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TRIGGER trg_vps_purposes_updated
    BEFORE UPDATE ON vps_purposes
    FOR EACH ROW EXECUTE FUNCTION update_updated_at();

INSERT INTO vps_purposes (name, label, color, sort_order) VALUES
    ('vpn-exit',      'VPN Exit',      'green',  1),
    ('vpn-relay',     'VPN Relay',     'blue',   2),
    ('vpn-entry',     'VPN Entry',     'orange', 3),
    ('monitor',       'Monitor',       'purple', 4),
    ('management',    'Management',    'gray',   5),
    ('core-services', 'Core Services', 'cyan',   6)
ON CONFLICT (name) DO NOTHING;

-- Drop the hardcoded CHECK constraint on vps.purpose
ALTER TABLE vps DROP CONSTRAINT IF EXISTS vps_purpose_check;
