-- Add 'agent' role: restricted to /agent/* and /envoy-* endpoints only
ALTER TABLE users DROP CONSTRAINT IF EXISTS users_role_check;
ALTER TABLE users ADD CONSTRAINT users_role_check
    CHECK (role IN ('admin','operator','readonly','agent'));

ALTER TABLE api_keys DROP CONSTRAINT IF EXISTS api_keys_role_check;
ALTER TABLE api_keys ADD CONSTRAINT api_keys_role_check
    CHECK (role IN ('admin','operator','readonly','agent'));
