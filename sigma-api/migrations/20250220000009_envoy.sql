-- envoy_nodes: Envoy instances on each VPS (identified by xDS node.id)
CREATE TABLE envoy_nodes (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    vps_id          UUID NOT NULL REFERENCES vps(id) ON DELETE CASCADE,
    node_id         TEXT NOT NULL,                           -- xDS node.id, e.g. "layer4-01"
    admin_port      INTEGER,                                 -- Envoy admin port, e.g. 9911
    description     TEXT NOT NULL DEFAULT '',
    config_version  BIGINT NOT NULL DEFAULT 0,               -- incremented on route changes, used as xDS version_info
    status          TEXT NOT NULL DEFAULT 'active',           -- active / disabled
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(vps_id, node_id)
);

-- envoy_routes: forwarding rules (each row = one listener + one cluster)
CREATE TABLE envoy_routes (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    envoy_node_id           UUID NOT NULL REFERENCES envoy_nodes(id) ON DELETE CASCADE,
    name                    TEXT NOT NULL,                    -- "tcp-proxy-hk01"
    listen_port             INTEGER NOT NULL,
    backend_host            TEXT,                             -- "hk013.example.com" (NULL = placeholder)
    backend_port            INTEGER,                          -- 30008 (NULL = placeholder)
    cluster_type            TEXT NOT NULL DEFAULT 'logical_dns', -- logical_dns / static / strict_dns
    connect_timeout_secs    INTEGER NOT NULL DEFAULT 5,
    proxy_protocol          INTEGER NOT NULL DEFAULT 1,       -- 0=none, 1=v1, 2=v2
    status                  TEXT NOT NULL DEFAULT 'active',   -- active / placeholder / disabled
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(envoy_node_id, listen_port)
);

-- Index for looking up routes by node
CREATE INDEX idx_envoy_routes_node ON envoy_routes(envoy_node_id);
