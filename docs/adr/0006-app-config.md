# AppConfig — Unified Multi-Source Configuration

<!-- product-status-2.0 -->
> **决策沿用与演进说明 · 2026-11-16**
>
> 决策状态：沿用配置职责统一原则，配置与秘密边界继续演进。2.0 包含 AppConfig、独立模型注册与脱敏 Web 更新接口，不能将旧全局/per-project 路径及环境变量解析示例视作完整现行配置契约；真实跨平台目录、迁移和备份能力以源码及验收为准。
>
> 正文的状态、任务勾选、代码和验收结论保留为原始记录，不作为 2.0 开发指令或交付证明。当前以[产品记忆](../../CONTEXT.md)、[2.0 批准方案](../plans/local-web.md)及[实现进度与验收缺口](../progress/local-web-2.0.md)为准。
<!-- /product-status-2.0 -->

CLI 用 `Config`，core 用 `LlmConfig + SandboxConfig`，两套 struct 字段重叠，数据手动复制。

决定统一为 `AppConfig`，支持多来源（per-project `qinAegis.toml` + global `~/.config/qinAegis/config.toml`），LLM API key 通过环境变量引用（`$FOO` / `${FOO}`）延迟解析。
