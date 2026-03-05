-- GIN index for efficient IP-based VPS deduplication lookups
CREATE INDEX IF NOT EXISTS idx_vps_ip_addresses_gin
  ON vps USING GIN (ip_addresses jsonb_path_ops);
