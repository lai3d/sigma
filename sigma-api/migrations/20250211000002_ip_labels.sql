-- Convert ip_addresses from INET[] to JSONB with labels
-- Each entry: {"ip": "1.2.3.4", "label": ""}

-- Add new JSONB column
ALTER TABLE vps ADD COLUMN ip_entries JSONB NOT NULL DEFAULT '[]';

-- Migrate existing INET[] data to JSONB (existing IPs get empty label)
UPDATE vps SET ip_entries = (
    SELECT COALESCE(
        jsonb_agg(jsonb_build_object('ip', host(elem), 'label', '')),
        '[]'::jsonb
    )
    FROM unnest(ip_addresses) AS elem
);

-- Drop old column and rename
ALTER TABLE vps DROP COLUMN ip_addresses;
ALTER TABLE vps RENAME COLUMN ip_entries TO ip_addresses;

-- Add GIN index for querying IPs within JSONB
CREATE INDEX idx_vps_ip_addresses ON vps USING GIN(ip_addresses);
