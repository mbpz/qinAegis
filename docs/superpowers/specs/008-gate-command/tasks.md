# Gate Command 实现任务清单

**Spec:** 008-gate-command
**Status:** In Progress

---

## 任务列表

### P0 - 核心功能 (必须完成)

| ID | Task | Status | Notes |
|----|------|--------|-------|
| T1 | 实现 GateService 和 GateResult 结构 | SELECTED | crates/core/src/gate.rs |
| T2 | 实现 CLI gate 命令入口 | SELECTED | crates/cli/src/commands/gate.rs |
| T3 | 更新 mod.rs 导出 gate 模块 | SELECTED | crates/cli/src/commands/mod.rs |
| T4 | 更新 lib.rs 导出 gate 模块 | SELECTED | crates/core/src/lib.rs |
| T5 | 集成到 main.rs/clap | SELECTED | crates/cli/src/main.rs |

### P1 - 增强功能 (可选)

| ID | Task | Status | Notes |
|----|------|--------|-------|
| T6 | 支持从项目配置加载阈值 | DEFERRED | 优先级低于 CLI 参数 |
| T7 | 支持 JSON 输出格式 | DEFERRED | --output-json |
| T8 | 详细输出模式 | DEFERRED | --verbose |

---

## 依赖关系

```
T1 (GateService) → T2 (CLI) → T3 (mod.rs) → T4 (lib.rs) → T5 (main.rs)
```

---

## 实现细节

### T1: GateService (core/gate.rs)

**文件**: `crates/core/src/gate.rs`

**实现内容**:
```rust
// 数据结构
pub struct GateThresholds { ... }
pub struct GateResult { ... }
pub struct E2EGateResult { ... }
pub struct PerfGateResult { ... }
pub struct StressGateResult { ... }

// 服务
pub struct GateService { ... }
impl GateService {
    pub async fn check(&self) -> anyhow::Result<GateResult>
    fn load_results(&self) -> anyhow::Result<GateResults>
    fn evaluate(&self, results: &GateResults) -> GateResult
}

// 输出函数
pub fn print_gate_result(result: &GateResult, verbose: bool)
pub fn print_gate_result_json(result: &GateResult)
```

### T2: CLI gate 命令 (cli/commands/gate.rs)

**文件**: `crates/cli/src/commands/gate.rs`

**clap 参数**:
```rust
#[derive(Parser)]
struct GateArgs {
    #[arg(long)]
    project: String,
    #[arg(long)]
    run_id: Option<String>,
    #[arg(long, default_value = "100")]
    e2e_threshold: f64,
    #[arg(long, default_value = "10")]
    perf_threshold: f64,
    #[arg(long, default_value = "100")]
    stress_rps_min: f64,
    #[arg(long, default_value = "2000")]
    stress_p95_max: f64,
    #[arg(long, default_value = "5")]
    stress_error_max: f64,
    #[arg(long)]
    output_json: bool,
    #[arg(long)]
    verbose: bool,
}
```

### T3-T5: 模块导出

更新 `mod.rs` 和 `lib.rs` 添加 gate 模块导出。

---

_Last Updated: 2026-05-07_