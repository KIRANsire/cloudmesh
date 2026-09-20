use std::fmt;
use std::str::FromStr;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct SystemMetrics {
    pub timestamp: DateTime<Utc>,

    pub cpu_usage: f32,

    pub memory_used: u64,
    pub memory_total: u64,

    pub disk_used: u64,
    pub disk_total: u64,

    pub network_received: u64,
    pub network_transmitted: u64,
}

/// Supported historical time ranges for telemetry queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricsRange {
    FiveMinutes,
    FifteenMinutes,
    ThirtyMinutes,
    OneHour,
    SixHours,
    TwelveHours,
    TwentyFourHours,
    SevenDays,
}

impl MetricsRange {
    /// Supported range error message string.
    pub const INVALID_RANGE_MESSAGE: &'static str =
        "Supported ranges are 5m, 15m, 30m, 1h, 6h, 12h, 24h, and 7d";

    /// Parse a string slice into a `MetricsRange`.
    pub fn parse(input: &str) -> Result<Self, &'static str> {
        match input {
            "5m" => Ok(Self::FiveMinutes),
            "15m" => Ok(Self::FifteenMinutes),
            "30m" => Ok(Self::ThirtyMinutes),
            "1h" => Ok(Self::OneHour),
            "6h" => Ok(Self::SixHours),
            "12h" => Ok(Self::TwelveHours),
            "24h" => Ok(Self::TwentyFourHours),
            "7d" => Ok(Self::SevenDays),
            _ => Err(Self::INVALID_RANGE_MESSAGE),
        }
    }

    /// Return string representation of the range.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::FiveMinutes => "5m",
            Self::FifteenMinutes => "15m",
            Self::ThirtyMinutes => "30m",
            Self::OneHour => "1h",
            Self::SixHours => "6h",
            Self::TwelveHours => "12h",
            Self::TwentyFourHours => "24h",
            Self::SevenDays => "7d",
        }
    }

    /// Convert range into a chrono Duration.
    pub fn duration(&self) -> Duration {
        match self {
            Self::FiveMinutes => Duration::minutes(5),
            Self::FifteenMinutes => Duration::minutes(15),
            Self::ThirtyMinutes => Duration::minutes(30),
            Self::OneHour => Duration::hours(1),
            Self::SixHours => Duration::hours(6),
            Self::TwelveHours => Duration::hours(12),
            Self::TwentyFourHours => Duration::hours(24),
            Self::SevenDays => Duration::days(7),
        }
    }
}

impl FromStr for MetricsRange {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl fmt::Display for MetricsRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Query parameters for GET /api/metrics/history.
#[derive(Debug, Deserialize, Default)]
pub struct HistoryQueryParams {
    pub range: Option<String>,
}

/// Response payload for GET /api/metrics/history.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct HistoricalMetricsResponse {
    pub range: String,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub count: usize,
    pub metrics: Vec<SystemMetrics>,
}

/// Consistent JSON error response format.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Node {
    pub node_id: String,
    pub hostname: String,
    pub os: String,
    pub architecture: String,
    pub agent_version: String,
    pub status: String, // Online or Offline, computed dynamically
    pub last_seen: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct RegisterNodeRequest {
    pub node_id: String,
    pub hostname: String,
    pub os: String,
    pub architecture: String,
    pub agent_version: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct TelemetryPayload {
    pub node_id: String,
    #[serde(flatten)]
    pub metrics: SystemMetrics,
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_ranges() {
        let cases = [
            ("5m", MetricsRange::FiveMinutes, Duration::minutes(5)),
            ("15m", MetricsRange::FifteenMinutes, Duration::minutes(15)),
            ("30m", MetricsRange::ThirtyMinutes, Duration::minutes(30)),
            ("1h", MetricsRange::OneHour, Duration::hours(1)),
            ("6h", MetricsRange::SixHours, Duration::hours(6)),
            ("12h", MetricsRange::TwelveHours, Duration::hours(12)),
            ("24h", MetricsRange::TwentyFourHours, Duration::hours(24)),
            ("7d", MetricsRange::SevenDays, Duration::days(7)),
        ];

        for (input, expected_variant, expected_duration) in cases {
            let parsed = MetricsRange::parse(input).expect("should parse valid range");
            assert_eq!(parsed, expected_variant);
            assert_eq!(parsed.as_str(), input);
            assert_eq!(parsed.duration(), expected_duration);
            assert_eq!(input.parse::<MetricsRange>().unwrap(), expected_variant);
        }
    }

    #[test]
    fn test_invalid_ranges() {
        let invalid_cases = [
            "",
            "abc",
            "999h",
            "-1h",
            "1hour",
            "today",
            "5M",
            "1H",
            "2d",
            "10m",
            " 1h",
            "1h ",
        ];

        for invalid in invalid_cases {
            let err = MetricsRange::parse(invalid);
            assert!(err.is_err(), "Expected error for invalid input: '{invalid}'");
            assert_eq!(err.unwrap_err(), MetricsRange::INVALID_RANGE_MESSAGE);
        }
    }

    #[test]
    fn test_response_serialization() {
        let now = Utc::now();
        let resp = HistoricalMetricsResponse {
            range: "1h".to_string(),
            start_time: now - Duration::hours(1),
            end_time: now,
            count: 0,
            metrics: vec![],
        };

        let json = serde_json::to_string(&resp).expect("serialization should succeed");
        assert!(json.contains("\"range\":\"1h\""));
        assert!(json.contains("\"count\":0"));
        assert!(json.contains("\"metrics\":[]"));
    }
}