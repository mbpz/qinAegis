// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

#[cfg(test)]
mod tests {
    use crate::stress::{LocustResult, LocustStats, StressTestConfig};

    // ========================================================================
    // StressTestConfig
    // ========================================================================

    #[test]
    fn test_stress_test_config_new() {
        let config = StressTestConfig::new("https://example.com", 100, 10, 60);
        assert_eq!(config.target_url, "https://example.com");
        assert_eq!(config.users, 100);
        assert_eq!(config.spawn_rate, 10);
        assert_eq!(config.duration_seconds, 60);
    }

    #[test]
    fn test_stress_test_config_serialization() {
        let config = StressTestConfig::new("https://test.com", 50, 5, 30);
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("https://test.com"));
        assert!(json.contains("50"));
    }

    // ========================================================================
    // LocustStats
    // ========================================================================

    #[test]
    fn test_locust_stats_serialization() {
        let stats = LocustStats {
            total_requests: 10000,
            total_failures: 50,
            median_response_time: 150.0,
            avg_response_time: 200.0,
            p95_response_time: 500.0,
            p99_response_time: 800.0,
            rps: 166.7,
            duration: 60.0,
        };
        let json = serde_json::to_string(&stats).unwrap();
        assert!(json.contains("10000"));
        assert!(json.contains("166.7"));
    }

    #[test]
    fn test_locust_result_deserialization() {
        let json = r#"{
            "target_url": "https://example.com",
            "stats": {
                "total_requests": 5000,
                "total_failures": 10,
                "median_response_time": 100.0,
                "avg_response_time": 150.0,
                "p95_response_time": 400.0,
                "p99_response_time": 600.0,
                "rps": 83.3,
                "duration": 60.0
            },
            "timestamp": "2026-01-01T00:00:00Z",
            "errors": []
        }"#;
        let result: LocustResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.target_url, "https://example.com");
        assert_eq!(result.stats.total_requests, 5000);
        assert_eq!(result.stats.total_failures, 10);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_locust_result_with_errors() {
        let result = LocustResult {
            target_url: "https://fail.com".to_string(),
            stats: LocustStats {
                total_requests: 1000,
                total_failures: 100,
                median_response_time: 200.0,
                avg_response_time: 300.0,
                p95_response_time: 600.0,
                p99_response_time: 900.0,
                rps: 16.7,
                duration: 60.0,
            },
            timestamp: "2026-01-01T00:00:00Z".to_string(),
            errors: vec!["Connection timeout".to_string(), "500 Internal Server Error".to_string()],
        };
        assert_eq!(result.stats.total_failures, 100);
        assert_eq!(result.errors.len(), 2);
    }
}