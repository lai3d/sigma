-- VPS IP address change history (tracked automatically via trigger)

CREATE TABLE vps_ip_history (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    vps_id     UUID NOT NULL REFERENCES vps(id) ON DELETE CASCADE,
    action     TEXT NOT NULL CHECK (action IN ('added', 'removed')),
    ip         TEXT NOT NULL,
    label      TEXT NOT NULL DEFAULT '',
    source     TEXT NOT NULL DEFAULT '',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_vps_ip_history_vps ON vps_ip_history(vps_id);
CREATE INDEX idx_vps_ip_history_time ON vps_ip_history(created_at DESC);
CREATE INDEX idx_vps_ip_history_ip ON vps_ip_history(ip);

-- Trigger function: automatically diff old vs new ip_addresses and record changes
CREATE OR REPLACE FUNCTION track_vps_ip_changes() RETURNS trigger AS $$
DECLARE
    old_ips JSONB;
    new_ips JSONB;
    entry   JSONB;
    vps_source TEXT;
BEGIN
    -- Get the VPS source for the history record
    vps_source := COALESCE(NEW.source, '');

    IF TG_OP = 'INSERT' THEN
        -- On INSERT, all IPs are "added"
        new_ips := COALESCE(NEW.ip_addresses, '[]'::jsonb);
        FOR entry IN SELECT * FROM jsonb_array_elements(new_ips)
        LOOP
            INSERT INTO vps_ip_history (vps_id, action, ip, label, source)
            VALUES (
                NEW.id,
                'added',
                COALESCE(entry->>'ip', ''),
                COALESCE(entry->>'label', ''),
                vps_source
            );
        END LOOP;

    ELSIF TG_OP = 'UPDATE' THEN
        old_ips := COALESCE(OLD.ip_addresses, '[]'::jsonb);
        new_ips := COALESCE(NEW.ip_addresses, '[]'::jsonb);

        -- Find removed IPs (in old but not in new, matched by ip field)
        FOR entry IN
            SELECT * FROM jsonb_array_elements(old_ips) AS o
            WHERE NOT EXISTS (
                SELECT 1 FROM jsonb_array_elements(new_ips) AS n
                WHERE n->>'ip' = o->>'ip'
            )
        LOOP
            INSERT INTO vps_ip_history (vps_id, action, ip, label, source)
            VALUES (
                NEW.id,
                'removed',
                COALESCE(entry->>'ip', ''),
                COALESCE(entry->>'label', ''),
                vps_source
            );
        END LOOP;

        -- Find added IPs (in new but not in old, matched by ip field)
        FOR entry IN
            SELECT * FROM jsonb_array_elements(new_ips) AS n
            WHERE NOT EXISTS (
                SELECT 1 FROM jsonb_array_elements(old_ips) AS o
                WHERE o->>'ip' = n->>'ip'
            )
        LOOP
            INSERT INTO vps_ip_history (vps_id, action, ip, label, source)
            VALUES (
                NEW.id,
                'added',
                COALESCE(entry->>'ip', ''),
                COALESCE(entry->>'label', ''),
                vps_source
            );
        END LOOP;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_vps_ip_changes
    AFTER INSERT OR UPDATE OF ip_addresses ON vps
    FOR EACH ROW
    EXECUTE FUNCTION track_vps_ip_changes();
