# Gate Command 实现计划

**Spec:** 008-gate-command
**Status:** In Progress
**Created:** 2026-05-07

---

## 架构设计

### 概览

```
gate 命令
    │
    ├── CLI 参数解析 (clap)
    │
    ├── GateService::check()
    │       │
    │       ├── load_results()     → 从 LocalStorage 加载各测试结果
    │       ├── evaluate()         → 对比阈值，返回 GateResult
    │       └── format_output()   → 控制台/JSON 输出
    │
    └── Exit code (0=pass, 1=fail)
```

---

## 模块设计

### 1. crates/core/src/gate.rs (新建)

```rust
// Gate 服务核心
pub struct GateService {
    project: String,
    run_id: Option<String>,
    thresholds: GateThresholds,
    storage: Arc<dyn Storage>,
}

pub struct GateThresholds {
    pub e2e_pass_rate_min: f64,       // 默认 100.0
    pub perf_regression_max: f64,      // 默认 10.0 (%)
    pub stress_rps_min: f64,           // 默认 100.0
    pub stress_p95_max_ms: f64,        // 默认 2000.0
    pub stress_error_rate_max: f64,    // 默认 5.0 (%)
}

pub struct GateResult {
    pub project: String,
    pub run_id: String,
    pub timestamp: String,
    pub e2e: E2EGateResult,
    pub performance: PerfGateResult,
    pub stress: StressGateResult,
    pub overall_passed: bool,
    pub exit_code: i32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum GateStatus {
    Passed,
    Failed,
    Skipped,
}

pub struct E2EGateResult {
    pub status: GateStatus,
    pub passed: usize,
    pub total: usize,
    pub pass_rate: f64,
    pub threshold: f64,
}

pub struct PerfGateResult {
    pub status: GateStatus,
    pub regressed: bool,
    pub regressions: Vec<String>,
    pub threshold: f64,
}

pub struct StressGateResult {
    pub status: GateStatus,
    pub rps: f64,
    pub p95_ms: f64,
    pub error_rate: f64,
    pub thresholds: StressThresholds,
}

impl GateService {
    pub async fn check(&self) -> anyhow::Result<GateResult>;

    fn load_results(&self) -> anyhow::Result<GateResults>;
    fn evaluate(&self, results: &GateResults) -> GateResult;
}

pub fn print_gate_result(result: &GateResult, verbose: bool);
pub fn print_gate_result_json(result: &GateResult);
```

### 2. crates/cli/src/commands/gate.rs (新建)

```rust
// CLI 命令入口
use qin_aegis_core::gate::{GateService, GateThresholds, print_gate_result, print_gate_result_json};

pub struct GateCommand {
    pub project: String,
    pub run_id: Option<String>,
    pub thresholds: GateThresholds,
    pub output_json: bool,
    pub verbose: bool,
}

impl GateCommand {
    pub async fn execute(&self) -> anyhow::Result<i32> {
        let storage = LocalStorage::new();
        let service = GateService::new(
            self.project.clone(),
            self.run_id.clone(),
            self.thresholds.clone(),
            Arc::new(storage),
        );

        let result = service.check().await?;

        if self.output_json {
            print_gate_result_json(&result);
        } else {
            print_gate_result(&result, self.verbose);
        }

        Ok(result.exit_code)
    }
}
```

### 3. 更新 crates/cli/src/commands/mod.rs

添加：
```rust
pub mod gate;
```

### 4. 更新 crates/core/src/lib.rs

添加：
```rust
pub mod gate;
```

---

## 数据流

### 1. 加载测试结果

```rust
struct GateResults {
    e2e: Option<E2ETestSummary>,       // from runs/<run-id>/summary.json
    performance: Option<LighthouseResult>, // from runs/<run-id>/lighthouse.json
    stress: Option<LocustResult>,       // from runs/<run-id>/locust-summary.json
}

impl GateService {
    fn load_results(&self) -> anyhow::Result<GateResults> {
        // 1. 确定 run_id（最新或指定）
        // 2. 读取对应的结果文件
        // 3. 返回 GateResults（缺失的文件返回 None）
    }
}
```

### 2. 评估门禁

```rust
impl GateService {
    fn evaluate(&self, results: &GateResults) -> GateResult {
        let e2e = self.evaluate_e2e(&results.e2e);
        let perf = self.evaluate_perf(&results.performance);
        let stress = self.evaluate_stress(&results.stress);

        let overall_passed = e2e.passed_gate && perf.passed_gate && stress.passed_gate;
        let exit_code = if overall_passed { 0 } else { 1 };

        GateResult { e2e, performance: perf, stress, overall_passed, exit_code, ... }
    }
}
```

### 3. 输出格式

**表格输出**:
```
╔══════════════════════════════════════════════════════════════╗
║                    qinAegis Gate Report                      ║
╠══════════════════════════════════════════════════════════════╣
║  Project: admin-web                                         ║
║  Run ID:  20260507-103000                                    ║
╠══════════════════════════════════════════════════════════════╣
║  [✓] E2E Tests          9/10 passed (90.0%)  [threshold: 100%]║
║  [✗] Performance       3 metrics regressed [threshold: 10%] ║
║  [✓] Stress Test       RPS: 450, P95: 850ms                  ║
╠══════════════════════════════════════════════════════════════╣
║  RESULT: FAIL                                                ║
╚══════════════════════════════════════════════════════════════╝
```

**JSON 输出** (`--output-json`):
```json
{
  "project": "admin-web",
  "run_id": "20260507-103000",
  "timestamp": "2026-05-07T10:30:00Z",
  "results": {
    "e2e": { "status": "failed", "passed": 9, "total": 10, "pass_rate": 90.0, "threshold": 100.0 },
    "performance": { "status": "failed", "regressions": ["LCP", "CLS", "TBT"], "threshold": 10.0 },
    "stress": { "status": "passed", "rps": 450.0, "p95_ms": 850.0, "error_rate": 2.5 }
  },
  "overall_passed": false,
  "exit_code": 1
}
```

---

## 实现顺序

1. **gate.rs** (core) - GateService、GateResult、GateThresholds
2. **gate.rs** (cli) - CLI 命令入口、clap 参数解析
3. **mod.rs** 更新 - 导出 gate 模块
4. **lib.rs** 更新 - 导出 gate 模块
5. **main.rs** 更新 - 注册 gate 命令（如果需要）

---

## 关键决策

### 1. run_id 确定逻辑

- 如果指定了 `--run-id`，使用指定值
- 如果未指定，使用 `runs/` 目录下最新创建的目录
- 如果没有运行记录，返回 error

### 2. 缺失数据处理

- 任一测试类型缺失结果 → 该类型标记为 `Skipped`
- 全部 `Skipped` → exit code 1（无法确定通过）
- 任一 `Failed` → overall `Failed`，exit code 1

### 3. 阈值优先级

1. CLI 参数（最高优先级）
2. 项目配置 `config.yaml`
3. 全局配置 `config.toml`
4. 默认值

---

_Last Updated: 2026-05-07_