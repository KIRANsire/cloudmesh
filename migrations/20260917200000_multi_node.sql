CREATE TABLE nodes (
    id BIGSERIAL PRIMARY KEY,
    node_id VARCHAR(255) UNIQUE NOT NULL,
    hostname VARCHAR(255) NOT NULL,
    os VARCHAR(255) NOT NULL,
    architecture VARCHAR(255) NOT NULL,
    agent_version VARCHAR(255) NOT NULL,
    status VARCHAR(50) NOT NULL,
    last_seen TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Insert a legacy node for existing telemetry records
INSERT INTO nodes (node_id, hostname, os, architecture, agent_version, status, last_seen) 
VALUES ('local-legacy', 'legacy', 'unknown', 'unknown', '0.1.0', 'offline', NOW());

-- Add node_id to telemetry table
ALTER TABLE telemetry ADD COLUMN node_id VARCHAR(255);

-- Update existing records to the legacy node
UPDATE telemetry SET node_id = 'local-legacy' WHERE node_id IS NULL;

-- Make node_id NOT NULL and add foreign key
ALTER TABLE telemetry ALTER COLUMN node_id SET NOT NULL;
ALTER TABLE telemetry ADD CONSTRAINT fk_node FOREIGN KEY (node_id) REFERENCES nodes (node_id) ON DELETE CASCADE;

-- Update indexes: drop old index, create new one optimized for (node_id, timestamp)
DROP INDEX IF EXISTS idx_telemetry_timestamp;
CREATE INDEX idx_telemetry_node_timestamp ON telemetry (node_id, timestamp ASC);
