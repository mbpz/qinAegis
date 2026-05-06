// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

#[cfg(test)]
mod tests {
    use crate::protocol::{JsonRpcResponse, SandboxConfig};

    // ========================================================================
    // JsonRpcResponse
    // ========================================================================

    #[test]
    fn test_json_rpc_response_ok() {
        let resp = JsonRpcResponse::ok("test-id", vec!["a", "b"]);
        assert!(resp.ok);
        assert_eq!(resp.id, "test-id");
        assert!(resp.data.is_some());
        assert!(resp.error.is_none());
    }

    #[test]
    fn test_json_rpc_response_err() {
        let resp = JsonRpcResponse::err("error-id", "something went wrong");
        assert!(!resp.ok);
        assert_eq!(resp.id, "error-id");
        assert!(resp.data.is_none());
        assert!(resp.error.is_some());
        assert_eq!(resp.error.as_deref(), Some("something went wrong"));
    }

    #[test]
    fn test_json_rpc_response_serialize() {
        let resp = JsonRpcResponse::ok("id-123", &{"key": "value"});
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"ok\":true"));
        assert!(json.contains("\"id\":\"id-123\""));
    }

    // ========================================================================
    // SandboxConfig
    // ========================================================================

    #[test]
    fn test_sandbox_config_default() {
        let config = SandboxConfig::default();
        assert_eq!(config.cdp_port, 9333);
    }

    #[test]
    fn test_sandbox_config_custom() {
        let config = SandboxConfig { cdp_port: 9222 };
        assert_eq!(config.cdp_port, 9222);
    }
}