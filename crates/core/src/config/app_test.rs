// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

#[cfg(test)]
mod tests {
    use crate::config::{
        AppConfig, ConfigError, ExplorationConfigSection, LlmConfigSection, SandboxConfigSection,
        resolve_env_var,
    };
    use std::io::Write;
    use tempfile::NamedTempFile;

    // ========================================================================
    // AppConfig defaults
    // ========================================================================

    #[test]
    fn test_app_config_default() {
        let config = AppConfig::default();
        assert_eq!(config.llm.provider, "minimax");
        assert_eq!(config.llm.base_url, "https://api.minimax.chat/v1");
        assert_eq!(config.llm.model, "MiniMax-VL-01");
        assert_eq!(config.sandbox.steel_port, 3333);
        assert_eq!(config.sandbox.cdp_port, 9222);
        assert_eq!(config.exploration.max_depth, 3);
        assert_eq!(config.exploration.max_pages_per_seed, 20);
    }

    #[test]
    fn test_llm_config_section_default() {
        let config = LlmConfigSection::default();
        assert_eq!(config.provider, "minimax");
        assert_eq!(config.model, "MiniMax-VL-01");
        assert!(config.api_key.is_empty());
    }

    #[test]
    fn test_sandbox_config_section_default() {
        let config = SandboxConfigSection::default();
        assert_eq!(config.steel_port, 3333);
        assert_eq!(config.cdp_port, 9222);
        assert!(config.compose_file.is_empty());
    }

    #[test]
    fn test_exploration_config_section_default() {
        let config = ExplorationConfigSection::default();
        assert_eq!(config.max_depth, 3);
        assert_eq!(config.max_pages_per_seed, 20);
    }

    // ========================================================================
    // AppConfig serialization
    // ========================================================================

    #[test]
    fn test_app_config_serialize() {
        let config = AppConfig::default();
        let toml_str = toml::to_string(&config).unwrap();
        assert!(toml_str.contains("minimax"));
        assert!(toml_str.contains("MiniMax-VL-01"));
    }

    #[test]
    fn test_app_config_deserialize() {
        let toml_str = r#"
            [llm]
            provider = "openai"
            base_url = "https://api.openai.com/v1"
            api_key = "sk-test"
            model = "gpt-4"

            [sandbox]
            compose_file = "/path/to/compose.yml"
            steel_port = 4444
            cdp_port = 9333

            [exploration]
            max_depth = 5
            max_pages_per_seed = 50
        "#;
        let config: AppConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.llm.provider, "openai");
        assert_eq!(config.llm.model, "gpt-4");
        assert_eq!(config.sandbox.steel_port, 4444);
        assert_eq!(config.sandbox.cdp_port, 9333);
        assert_eq!(config.exploration.max_depth, 5);
    }

    // ========================================================================
    // AppConfig load / save (using tempfile)
    // ========================================================================

    #[test]
    fn test_app_config_load_from_file() {
        let mut temp_file = NamedTempFile::with_suffix(".toml").unwrap();
        temp_file
            .write_all(
                r#"
                [llm]
                provider = "test-provider"
                base_url = "https://test.com"
                api_key = "test-key"
                model = "test-model"

                [sandbox]
                steel_port = 1234
                cdp_port = 5678
                "#,
            )
            .unwrap();

        let config = AppConfig::load_from(temp_file.path()).unwrap();
        assert_eq!(config.llm.provider, "test-provider");
        assert_eq!(config.llm.model, "test-model");
        assert_eq!(config.sandbox.steel_port, 1234);
    }

    #[test]
    fn test_app_config_load_not_found() {
        let result = AppConfig::load_from(std::path::Path::new("/nonexistent/path.toml"));
        assert!(matches!(result, Err(ConfigError::Internal(_))));
    }

    // ========================================================================
    // AppConfig merge
    // ========================================================================

    #[test]
    fn test_app_config_merge() {
        let mut base = AppConfig::default();
        base.llm.provider = "base-provider".to_string();

        let override_config = AppConfig {
            llm: LlmConfigSection {
                provider: "override-provider".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };

        base.merge(override_config);
        assert_eq!(base.llm.provider, "override-provider");
    }

    #[test]
    fn test_app_config_merge_partial() {
        let mut base = AppConfig::default();
        base.llm.provider = "original".to_string();
        base.llm.model = "original-model".to_string();

        let partial = AppConfig {
            llm: LlmConfigSection {
                provider: "new".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };

        base.merge(partial);
        assert_eq!(base.llm.provider, "new");
        assert_eq!(base.llm.model, "original-model");
    }

    // ========================================================================
    // Environment variable resolution
    // ========================================================================

    #[test]
    fn test_resolve_env_var_dollar_syntax() {
        std::env::set_var("TEST_API_KEY", "secret123");
        let result = crate::config::app::resolve_env_var("$TEST_API_KEY");
        assert_eq!(result, "secret123");
        std::env::remove_var("TEST_API_KEY");
    }

    #[test]
    fn test_resolve_env_var_braces_syntax() {
        std::env::set_var("TEST_VAR", "value456");
        let result = crate::config::app::resolve_env_var("${TEST_VAR}");
        assert_eq!(result, "value456");
        std::env::remove_var("TEST_VAR");
    }

    #[test]
    fn test_resolve_env_var_mixed_string() {
        std::env::set_var("HOST", "example.com");
        let result = crate::config::app::resolve_env_var("https://$HOST/api");
        assert_eq!(result, "https://example.com/api");
        std::env::remove_var("HOST");
    }

    #[test]
    fn test_resolve_env_var_unset_var() {
        std::env::remove_var("NONEXISTENT_VAR");
        let result = crate::config::app::resolve_env_var("$NONEXISTENT_VAR");
        assert_eq!(result, "");
    }

    #[test]
    fn test_resolve_env_var_no_var() {
        let result = crate::config::app::resolve_env_var("plain-text");
        assert_eq!(result, "plain-text");
    }
}