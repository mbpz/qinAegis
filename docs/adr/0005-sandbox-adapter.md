# SandboxAdapter — Health Check + Hot Reload

<!-- product-status-2.0 -->
> **决策沿用与演进说明 · 2026-11-16**
>
> 决策状态：沿用生命周期/健康检查的适配边界，交付说明已演进。2.0 基础服务不捆绑 Node/Chromium，Playwright + Midscene.js 引擎按需准备；“零外部依赖”“跨平台均可运行”是旧方案表述，不能作为当前干净机器验收结论。旧启动、重连和配置示例仅记录该决策阶段。
>
> 正文的状态、任务勾选、代码和验收结论保留为原始记录，不作为 2.0 开发指令或交付证明。当前以[产品记忆](../../CONTEXT.md)、[2.0 批准方案](../plans/local-web.md)及[实现进度与验收缺口](../progress/local-web-2.0.md)为准。
<!-- /product-status-2.0 -->

## Status

**Accepted** — 2026-05-07 更新

## 背景

Rust 硬编码 tsx 路径通过 `CARGO_MANIFEST_DIR` 计算，TS 进程出错时 Rust 只收到"process died"，无法诊断。

## 决策

引入 `SandboxAdapter` trait，支持热重载（browser 崩溃后自动重连）和 CDP URL retry（等待 browser 就绪）。

### 2026-05-07 技术路线更新

原方案使用 SteelBrowserAdapter（Docker + steel-browser）和 ShellBrowserAdapter。

**新方案**：使用 `PlaywrightBrowserAdapter`，直接通过 Playwright 的 `chromium.launch()` 启动独立浏览器进程，无需 Docker 或外部浏览器安装。

**优势**：
- 零外部依赖：Playwright 自动下载和管理 Chromium
- 完全隔离：每个测试使用独立的浏览器上下文
- 跨平台支持：Windows/Linux/macOS 均可运行
- 简化部署：不再需要 Docker Desktop

## 实现

### PlaywrightBrowserAdapter

```rust
pub struct PlaywrightBrowserAdapter {
    cdp_port: u16,
    child: Arc<Mutex<Option<Child>>>,
}

impl PlaywrightBrowserAdapter {
    /// 启动 Playwright 管理的 Chromium
    pub fn launch(&self) -> Result<()> {
        // 优先使用系统 Chrome，否则使用 Playwright 下载的 Chromium
    }

    /// 检查浏览器是否运行
    fn is_browser_running(&self) -> bool {
        // 通过 CDP 端口健康检查
    }

    /// 等待浏览器就绪
    fn wait_for_browser_internal(&self, timeout_secs: u64) -> Result<String> {
        // 轮询 /json/version 直到浏览器响应
    }
}
```

### SandboxAdapter Trait

```rust
#[async_trait]
pub trait SandboxAdapter: Send + Sync {
    fn cdp_url(&self) -> Option<String>;
    async fn health(&self) -> Result<SandboxHealth, SandboxError>;
    async fn wait_for_browser(&self, timeout_secs: u64) -> Result<String, SandboxError>;
    async fn restart(&self) -> Result<(), SandboxError>;
}
```

## 配置

```toml
[sandbox]
cdp_port = 9222
```

不再需要 `compose_file` 和 `steel_port`。

## 影响

- 移除 `docker.rs`、`steel.rs` 模块
- 移除 `docker/` 目录及其配置文件
- 更新所有文档中的沙箱说明