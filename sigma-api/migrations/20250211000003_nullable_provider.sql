-- Allow VPS instances to exist without a provider (e.g. agent-registered VPS)
ALTER TABLE vps ALTER COLUMN provider_id DROP NOT NULL;
