-- Fix IP history source: use session variable app.change_source when set,
-- so the actual change origin (agent, cloud-sync, manual) is recorded
-- instead of always copying the VPS source column.
--
-- API handlers SET LOCAL app.change_source = '...' before updating ip_addresses.
-- If not set, falls back to vps.source (backward compatible).

CREATE OR REPLACE FUNCTION track_vps_ip_changes() RETURNS trigger AS $$
DECLARE
    old_ips JSONB;
    new_ips JSONB;
    entry   JSONB;
    change_source TEXT;
BEGIN
    -- Prefer explicit change source set by the API handler, fall back to vps.source
    change_source := COALESCE(
        NULLIF(current_setting('app.change_source', true), ''),
        NEW.source,
        ''
    );

    IF TG_OP = 'INSERT' THEN
        new_ips := COALESCE(NEW.ip_addresses, '[]'::jsonb);
        FOR entry IN SELECT * FROM jsonb_array_elements(new_ips)
        LOOP
            INSERT INTO vps_ip_history (vps_id, action, ip, label, source)
            VALUES (
                NEW.id,
                'added',
                COALESCE(entry->>'ip', ''),
                COALESCE(entry->>'label', ''),
                change_source
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
                change_source
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
                change_source
            );
        END LOOP;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
