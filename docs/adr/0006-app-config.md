# AppConfig — Unified Multi-Source Configuration

CLI 用 `Config`，core 用 `LlmConfig + SandboxConfig`，两套 struct 字段重叠，数据手动复制。

决定统一为 `AppConfig`，支持多来源（per-project `qinAegis.toml` + global `~/.config/qinAegis/config.toml`），LLM API key 通过环境变量引用（`$FOO` / `${FOO}`）延迟解析。
