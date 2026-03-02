-- DNS record change history (tracked automatically via trigger on dns_records)

CREATE TABLE dns_record_history (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    dns_record_id  UUID NOT NULL,          -- no FK: history preserved after record deletion
    zone_uuid      UUID NOT NULL,
    record_id      TEXT NOT NULL,           -- provider-side record ID
    record_type    TEXT NOT NULL,
    name           TEXT NOT NULL,
    action         TEXT NOT NULL CHECK (action IN ('created', 'updated', 'deleted')),
    old_content    TEXT,                    -- NULL for 'created'
    new_content    TEXT,                    -- NULL for 'deleted'
    old_extra      JSONB,
    new_extra      JSONB,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_dns_record_history_record ON dns_record_history(dns_record_id, created_at DESC);
CREATE INDEX idx_dns_record_history_zone   ON dns_record_history(zone_uuid, created_at DESC);

-- Trigger function: track INSERT / UPDATE / DELETE on dns_records
CREATE OR REPLACE FUNCTION track_dns_record_changes() RETURNS trigger AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        INSERT INTO dns_record_history
            (dns_record_id, zone_uuid, record_id, record_type, name, action, new_content, new_extra)
        VALUES
            (NEW.id, NEW.zone_uuid, NEW.record_id, NEW.record_type, NEW.name,
             'created', NEW.content, NEW.extra);
        RETURN NEW;

    ELSIF TG_OP = 'UPDATE' THEN
        -- Only log when content or extra actually changed
        IF OLD.content IS DISTINCT FROM NEW.content
           OR OLD.extra IS DISTINCT FROM NEW.extra THEN
            INSERT INTO dns_record_history
                (dns_record_id, zone_uuid, record_id, record_type, name, action,
                 old_content, new_content, old_extra, new_extra)
            VALUES
                (NEW.id, NEW.zone_uuid, NEW.record_id, NEW.record_type, NEW.name,
                 'updated', OLD.content, NEW.content, OLD.extra, NEW.extra);
        END IF;
        RETURN NEW;

    ELSIF TG_OP = 'DELETE' THEN
        INSERT INTO dns_record_history
            (dns_record_id, zone_uuid, record_id, record_type, name, action, old_content, old_extra)
        VALUES
            (OLD.id, OLD.zone_uuid, OLD.record_id, OLD.record_type, OLD.name,
             'deleted', OLD.content, OLD.extra);
        RETURN OLD;
    END IF;

    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_dns_record_changes
    AFTER INSERT OR UPDATE OR DELETE ON dns_records
    FOR EACH ROW
    EXECUTE FUNCTION track_dns_record_changes();
