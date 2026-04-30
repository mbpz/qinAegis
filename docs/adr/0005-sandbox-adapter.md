# SandboxAdapter — Health Check + Hot Reload

Rust hardcode tsx 路径通过 `CARGO_MANIFEST_DIR` 计算，TS 进程出错时 Rust 只收到"process died"，无法诊断。

决定引入 `SandboxAdapter` trait，支持热重载（browser 崩溃后自动重连）和 CDP URL retry（等待 browser 就绪）。`SteelBrowserAdapter` 使用已有 CDP URL；`ShellBrowserAdapter` 从 `/json/version` 动态解析。
