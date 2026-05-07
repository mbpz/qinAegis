// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocustStats {
    pub total_requests: u64,
    pub total_failures: u64,
    pub median_response_time: f64,
    pub avg_response_time: f64,
    pub p95_response_time: f64,
    pub p99_response_time: f64,
    pub rps: f64,
    pub duration: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocustResult {
    pub target_url: String,
    pub stats: LocustStats,
    pub timestamp: String,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StressTestConfig {
    pub target_url: String,
    pub users: u32,
    pub spawn_rate: u32,
    pub duration_seconds: u32,
}

impl StressTestConfig {
    pub fn new(target_url: &str, users: u32, spawn_rate: u32, duration_seconds: u32) -> Self {
        Self {
            target_url: target_url.to_string(),
            users,
            spawn_rate,
            duration_seconds,
        }
    }
}