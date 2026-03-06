-- Allow 'deleted' status for soft delete
ALTER TABLE vps DROP CONSTRAINT IF EXISTS vps_status_check;
ALTER TABLE vps ADD CONSTRAINT vps_status_check
    CHECK (status IN ('provisioning', 'active', 'retiring', 'retired', 'suspended', 'deleted'));
