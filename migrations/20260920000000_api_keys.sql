CREATE TABLE node_api_keys (
    id BIGSERIAL PRIMARY KEY,

    node_id VARCHAR(255) NOT NULL,

    key_hash VARCHAR(255) UNIQUE NOT NULL,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    last_used_at TIMESTAMPTZ,

    revoked_at TIMESTAMPTZ,

    CONSTRAINT fk_node_api_key
        FOREIGN KEY (node_id)
        REFERENCES nodes (node_id)
        ON DELETE CASCADE
);

CREATE INDEX idx_node_api_keys_node_id
    ON node_api_keys (node_id);

CREATE INDEX idx_node_api_keys_key_hash
    ON node_api_keys (key_hash);