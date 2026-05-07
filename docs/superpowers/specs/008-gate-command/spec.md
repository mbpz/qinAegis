# Gate Command — 质量门禁规范

**Spec ID:** 008-gate-command
**Status:** In Progress
**Created:** 2026-05-07

---

## Summary

实现 `qinAegis gate` 命令，统一聚合 E2E 测试、性能测试、压力测试结果，根据可配置阈值输出 CI 兼容的 pass/fail exit code。

---

## 需求背景

qinAegis Roadmap 中规划了 `qinAegis gate --project <name>` 作为质量门禁功能：
- **E2E 通过率**：已批准用例的执行通过率
- **性能基准**：Lighthouse CI Web Vitals 指标回归检测
- **压测阈值**：k6/Locust 负载测试 RPS/P95/错误率阈值

当前状态：
- `TestExecutor` 和 `TestResult` 已实现
- `LighthouseResult` 和 `PerformanceComparison` 已实现
- `LocustResult` 已定义但未完全集成
- **gate 命令不存在**

---

## 需求详述

### 1. CLI 接口

```bash
qinAegis gate --project <name> [OPTIONS]
```

| 参数 | 说明 | 默认值 |
|------|------|--------|
| `--project` | 项目名称 | 必填 |
| `--run-id` | 指定运行 ID | 最新运行 |
| `--e2e-threshold` | E2E 通过率阈值 (0-100) | 100 |
| `--perf-threshold` | 性能回归阈值 (%) | 10 |
| `--stress-rps-min` | 压测最低 RPS | 100 |
| `--stress-p95-max` | 压测 P95 最大延迟 (ms) | 2000 |
| `--stress-error-rate-max` | 压测最大错误率 (%) | 5 |
| `--output-json` | JSON 输出 | false |
| `--verbose` | 详细输出 | false |

### 2. 输出格式

**CI Exit Code**:
- `0` = 所有门禁通过
- `1` = 任一指标未达标

**控制台输出**:
```
╔══════════════════════════════════════════════════════════════╗
║                    qinAegis Gate Report                      ║
╠══════════════════════════════════════════════════════════════╣
║  Project: admin-web                                         ║
║  Run ID:  20260507-103000                                    ║
╠══════════════════════════════════════════════════════════════╣
║  [✓] E2E Tests          9/10 passed (90.0%)  [threshold: 100%]║
║  [✓] Performance       No regression detected [threshold: 10%]║
║  [✓] Stress Test       RPS: 450, P95: 850ms  [thresholds met]║
╠══════════════════════════════════════════════════════════════╣
║  RESULT: PASS                                                ║
╚══════════════════════════════════════════════════════════════╝
```

**JSON 输出** (`--output-json`):
```json
{
  "project": "admin-web",
  "run_id": "20260507-103000",
  "timestamp": "2026-05-07T10:30:00Z",
  "results": {
    "e2e": {
      "passed": 9,
      "total": 10,
      "pass_rate": 90.0,
      "threshold": 100.0,
      "passed_gate": false
    },
    "performance": {
      "regressed": false,
      "threshold_percent": 10.0,
      "regressions": []
    },
    "stress": {
      "rps": 450.0,
      "p95_ms": 850.0,
      "error_rate": 2.5,
      "passed_gate": true
    }
  },
  "overall_passed": false,
  "exit_code": 1
}
```

### 3. 数据来源

| 指标 | 数据来源 |
|------|----------|
| E2E 通过率 | `runs/<run-id>/summary.json` 或 `runs/<run-id>/e2e-results.json` |
| 性能回归 | `runs/<run-id>/lighthouse.json` |
| 压测结果 | `runs/<run-id>/locust-summary.json` |

### 4. 阈值配置

阈值可通过以下方式指定（优先级从高到低）：
1. CLI 参数 `--e2e-threshold` 等
2. 项目配置 `~/.qinAegis/projects/<project>/config.yaml`
3. 全局配置 `~/.config/qinAegis/config.toml`
4. 默认值

---

## 架构设计

### 模块结构

```
crates/
├── cli/src/commands/
│   └── gate.rs          # NEW: gate 命令入口
├── core/src/
│   ├── gate.rs          # NEW: Gate 服务核心逻辑
│   ├── executor.rs      # 已有: TestExecutor
│   ├── performance.rs   # 已有: LighthouseResult, PerformanceComparison
│   └── stress.rs        # 已有: LocustResult, LocustStats
```

### 数据流

```
gate 命令
    │
    ├── Load latest run results from LocalStorage
    │       │
    │       ├── e2e-results.json    → E2E summary
    │       ├── lighthouse.json    → PerformanceComparison
    │       └── locust-summary.json → StressTestResult
    │
    ├── Apply thresholds
    │       │
    │       ├── E2E pass rate >= threshold?
    │       ├── Performance regression <= threshold%?
    │       └── Stress metrics within bounds?
    │
    ├── Output result
    │       │
    │       ├── Console table (default)
    │       ├── JSON (--output-json)
    │       └── Exit code (0 = pass, 1 = fail)
    │
    └── (optional) Save gate result to runs/<run-id>/gate-result.json
```

### 错误处理

| 场景 | 行为 |
|------|------|
| 项目不存在 | 返回 error 并 exit code 1 |
| 指定 run-id 不存在 | 返回 error 并 exit code 1 |
| 缺少 E2E 结果文件 | E2E 门禁视为 SKIPPED（非 pass/fail） |
| 缺少性能结果文件 | 性能门禁视为 SKIPPED |
| 缺少压测结果文件 | 压测门禁视为 SKIPPED |
| 全部 SKIPPED | 返回 warning，exit code 1 |

---

## 实现要点

### 1. Gate 结构

```rust
// crates/core/src/gate.rs

pub struct GateResult {
    pub project: String,
    pub run_id: String,
    pub timestamp: String,
    pub e2e_result: E2EGateResult,
    pub perf_result: PerfGateResult,
    pub stress_result: StressGateResult,
    pub overall_passed: bool,
    pub exit_code: i32,
}

pub struct E2EGateResult {
    pub passed: usize,
    pub total: usize,
    pub pass_rate: f64,
    pub threshold: f64,
    pub passed_gate: bool,
    pub skipped: bool,
}

pub struct PerfGateResult {
    pub regressed: bool,
    pub threshold_percent: f64,
    pub regressions: Vec<String>,
    pub passed_gate: bool,
    pub skipped: bool,
}

pub struct StressGateResult {
    pub rps: f64,
    pub p95_ms: f64,
    pub error_rate: f64,
    pub passed_gate: bool,
    pub skipped: bool,
    // ... threshold fields
}
```

### 2. 阈值默认值

```rust
impl Default for GateThresholds {
    fn default() -> Self {
        Self {
            e2e_pass_rate_min: 100.0,       // 要求 100% 通过
            perf_regression_max: 10.0,      // 性能退化不超过 10%
            stress_rps_min: 100.0,          // 最低 100 RPS
            stress_p95_max_ms: 2000.0,      // P95 最大 2s
            stress_error_rate_max: 5.0,     // 错误率最大 5%
        }
    }
}
```

### 3. CI 兼容性

- Exit code 0 = pass, 1 = fail，可被 shell/CI 直接使用
- JSON 输出便于程序解析
- 表格输出便于人类阅读

---

## 验收标准

1. `qinAegis gate --project <name>` 能正确读取最新运行结果
2. 阈值覆盖 E2E 通过率、性能回归、压测指标
3. JSON 输出包含所有门禁结果
4. Exit code 与门禁结果一致
5. 缺失数据时正确处理（SKIPPED）

---

_Last Updated: 2026-05-07_