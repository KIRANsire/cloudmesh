CREATE TABLE telemetry (
    id BIGSERIAL PRIMARY KEY,
    timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    cpu_usage REAL NOT NULL,
    memory_used BIGINT NOT NULL,
    memory_total BIGINT NOT NULL,
    disk_used BIGINT NOT NULL,
    disk_total BIGINT NOT NULL,
    network_rx BIGINT NOT NULL,
    network_tx BIGINT NOT NULL
);

CREATE INDEX idx_telemetry_timestamp
ON telemetry (timestamp);