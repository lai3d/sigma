ALTER TABLE envoy_routes ADD COLUMN source TEXT NOT NULL DEFAULT 'dynamic';
-- 'dynamic' = managed via API/UI, 'static' = synced from envoy.yaml

-- Partial unique index for static route upsert (match by node + port for static routes only)
CREATE UNIQUE INDEX envoy_routes_static_upsert
    ON envoy_routes (envoy_node_id, listen_port)
    WHERE source = 'static';
