# CloudMesh

CloudMesh is a lightweight system telemetry collection and monitoring service written in Rust (2024 Edition). It periodically collects system metrics (CPU, Memory, Disk, and Network) every 5 seconds, maintains the latest snapshot in memory for fast retrieval, and persists historical telemetry records into PostgreSQL.

---

## API Endpoints

### 1. Health Check
* **Method:** `GET`
* **Path:** `/api/health`
* **Description:** Returns service name, health status, and version.
* **Example Response (`200 OK`):**
  ```json
  {
    "service": "cloudmesh",
    "status": "healthy",
    "version": "0.1.0"
  }
  ```

---

### 2. Current Metrics
* **Method:** `GET`
* **Path:** `/api/metrics`
* **Description:** Returns the most recent system telemetry snapshot from memory.
* **Example Response (`200 OK`):**
  ```json
  {
    "timestamp": "2026-09-16T02:00:05.123456Z",
    "cpu_usage": 12.4,
    "memory_used": 7340032000,
    "memory_total": 16831893504,
    "disk_used": 182000000000,
    "disk_total": 510747734016,
    "network_received": 1520,
    "network_transmitted": 830
  }
  ```

---

### 3. Historical Metrics
* **Method:** `GET`
* **Path:** `/api/metrics/history?range={range}`
* **Description:** Retrieves historical telemetry collected during the specified time range directly from PostgreSQL.

#### Required Query Parameter
* `range`: Specifies the duration back from the current UTC time.

#### Supported Ranges
Only the following exact values are accepted:
* `5m` — 5 minutes
* `15m` — 15 minutes
* `30m` — 30 minutes
* `1h` — 1 hour
* `6h` — 6 hours
* `12h` — 12 hours
* `24h` — 24 hours
* `7d` — 7 days

Any other value or missing parameter is rejected with a controlled HTTP 400 response.

#### Example Request
```http
GET /api/metrics/history?range=1h
```

#### Example Response (`200 OK`):
```json
{
  "range": "1h",
  "start_time": "2026-09-16T01:00:00Z",
  "end_time": "2026-09-16T02:00:00Z",
  "count": 721,
  "metrics": [
    {
      "timestamp": "2026-09-16T01:00:05Z",
      "cpu_usage": 12.4,
      "memory_used": 7340032000,
      "memory_total": 16831893504,
      "disk_used": 182000000000,
      "disk_total": 510747734016,
      "network_received": 1520,
      "network_transmitted": 830
    }
  ]
}
```

#### Behavior & Characteristics
* **UTC Timestamps:** All queries, start/end bounds, and metric timestamps strictly use UTC (`chrono::DateTime<Utc>`).
* **PostgreSQL Source:** Queries use parameterized SQL filtered on `timestamp >= $1` using the `idx_telemetry_timestamp` index, ordered chronologically (`timestamp ASC`).
* **Bounded Results:** Results are capped by `MAX_HISTORICAL_METRICS_LIMIT` (10,000 records) to prevent memory exhaustion and maintain responsiveness.
* **Empty Ranges:** If no records exist for the specified timeframe, the endpoint returns HTTP 200 with `count: 0` and `metrics: []`.
* **Invalid or Missing Range:** Returns HTTP 400 Bad Request:
  ```json
  {
    "error": "invalid_range",
    "message": "Supported ranges are 5m, 15m, 30m, 1h, 6h, 12h, 24h, and 7d"
  }
  ```
* **Database Errors:** If PostgreSQL is unreachable or fails, returns HTTP 500 Internal Server Error without exposing internal database credentials or SQL queries:
  ```json
  {
    "error": "database_error",
    "message": "Unable to retrieve historical metrics"
  }
  ```
