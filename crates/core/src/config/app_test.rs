// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

#[cfg(test)]
mod tests {
    use crate::config::{
        resolve_env_var, AppConfig, ConfigError, ExplorationConfigSection, LlmConfigSection,
        SandboxConfigSection,
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
        assert_eq!(config.llm.model, "MiniMax-M2.7");
        assert_eq!(config.sandbox.cdp_port, 9222);
        assert_eq!(config.exploration.max_depth, 3);
        assert_eq!(config.exploration.max_pages_per_seed, 20);
    }

    #[test]
    fn test_llm_config_section_default() {
        let config = LlmConfigSection::default();
        assert_eq!(config.provider, "minimax");
        assert_eq!(config.model, "MiniMax-M2.7");
        assert!(config.api_key.is_empty());
    }

    #[test]
    fn test_sandbox_config_section_default() {
        let config = SandboxConfigSection::default();
        assert_eq!(config.cdp_port, 9222);
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
        assert!(toml_str.contains("MiniMax-M2.7"));
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
            cdp_port = 9333

            [exploration]
            max_depth = 5
            max_pages_per_seed = 50
        "#;
        let config: AppConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.llm.provider, "openai");
        assert_eq!(config.llm.model, "gpt-4");
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
                br#"
                [llm]
                provider = "test-provider"
                base_url = "https://test.com"
                api_key = "test-key"
                model = "test-model"

                [sandbox]
                cdp_port = 5678
                "#,
            )
            .unwrap();

        let config = AppConfig::load_from(temp_file.path()).unwrap();
        assert_eq!(config.llm.provider, "test-provider");
        assert_eq!(config.llm.model, "test-model");
        assert_eq!(config.sandbox.cdp_port, 5678);
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
                model: String::new(),
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

    #[test]
    fn atomic_save_backup_restore_and_stale_revision() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut config = AppConfig::default();
        config.llm.api_key = "private-key".into();
        assert_eq!(config.save_to_checked(&path, 0).unwrap(), 1);
        assert!(!AppConfig::inspect_at(&path).unwrap().backup_available);
        config.llm.model = "replacement".into();
        assert_eq!(config.save_to_checked(&path, 1).unwrap(), 2);
        let before = std::fs::read(&path).unwrap();
        assert!(config.save_to_checked(&path, 1).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), before);
        let metadata = serde_json::to_string(&AppConfig::inspect_at(&path).unwrap()).unwrap();
        assert!(!metadata.contains("private-key"));
        assert_eq!(AppConfig::restore_backup_at(&path, 2).unwrap(), 3);
        assert_eq!(
            AppConfig::load_from(&path).unwrap().llm.model,
            "MiniMax-M2.7"
        );
        assert_eq!(
            AppConfig::load_from(&path).unwrap().llm.api_key,
            "private-key"
        );
        assert_eq!(
            AppConfig::load_from(&AppConfig::backup_path(&path))
                .unwrap()
                .llm
                .model,
            "replacement"
        );
    }

    #[test]
    fn legacy_configuration_is_preserved_before_revision_upgrade() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let legacy = toml::to_string(&AppConfig::default()).unwrap();
        std::fs::write(&path, &legacy).unwrap();
        assert_eq!(AppConfig::inspect_at(&path).unwrap().revision, 0);
        AppConfig::default().save_to_checked(&path, 0).unwrap();
        assert_eq!(
            std::fs::read_to_string(AppConfig::backup_path(&path)).unwrap(),
            legacy
        );
    }

    #[test]
    fn corrupt_primary_and_backup_are_never_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let invalid = b"api_key = [ secret-private-value";
        std::fs::write(&path, invalid).unwrap();
        let error = AppConfig::default()
            .save_to_checked(&path, 0)
            .unwrap_err()
            .to_string();
        assert!(!error.contains("secret-private-value"));
        assert_eq!(std::fs::read(&path).unwrap(), invalid);
        std::fs::remove_file(&path).unwrap();
        AppConfig::default().save_to_checked(&path, 0).unwrap();
        let before = std::fs::read(&path).unwrap();
        std::fs::write(AppConfig::backup_path(&path), invalid).unwrap();
        assert!(AppConfig::default().save_to_checked(&path, 1).is_err());
        assert!(AppConfig::restore_backup_at(&path, 1).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert_eq!(
            std::fs::read(AppConfig::backup_path(&path)).unwrap(),
            invalid
        );
    }

    #[test]
    fn oversized_configuration_is_bounded() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, vec![b'a'; 1024 * 1024 + 1]).unwrap();
        assert!(AppConfig::load_from(&path).is_err());
        assert!(AppConfig::default().save_to_checked(&path, 0).is_err());
    }

    #[test]
    fn busy_writer_does_not_modify_configuration() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        AppConfig::default().save_to_checked(&path, 0).unwrap();
        let before = std::fs::read(&path).unwrap();
        let file = crate::asset_file::AssetFile::new(path.clone(), 1024 * 1024);
        let _guard = file.lock().unwrap();
        assert!(AppConfig::default().save_to_checked(&path, 1).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }

    #[cfg(unix)]
    #[test]
    fn private_permissions_and_symlink_refusal() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        AppConfig::default().save_to_checked(&path, 0).unwrap();
        AppConfig::default().save_to_checked(&path, 1).unwrap();
        for file in [&path, &AppConfig::backup_path(&path)] {
            assert_eq!(
                std::fs::metadata(file).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        let link = dir.path().join("linked.toml");
        symlink(&path, &link).unwrap();
        assert!(AppConfig::load_from(&link).is_err());
        assert!(AppConfig::default().save_to_checked(&link, 0).is_err());
        std::fs::remove_file(AppConfig::backup_path(&path)).unwrap();
        symlink(&path, AppConfig::backup_path(&path)).unwrap();
        assert!(AppConfig::default().save_to_checked(&path, 2).is_err());
    }
}
