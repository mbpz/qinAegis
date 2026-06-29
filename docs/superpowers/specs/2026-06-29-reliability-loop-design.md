# Reliability Loop - Closed Pipeline

**Date:** 2026-06-29
**Status:** Draft

## Overview

把现有 Critic / Healer / Gate 三个孤岛用事件总线串成闭环.
`Generate → Critic → Execute → Heal → FlakyTrack → Gate` 一条流水线,
生成失败重生成, 执行失败自动愈合, 抖动自动隔离, 闸门统一收口.

## Architecture

```
crates/core/src/
├── bus.rs          # ReliabilityBus: Tokio broadcast + 主题路由
├── events.rs       # ReliabilityEvent 枚举
├── pipeline.rs     # 启动入口, 装配 worker, 关闭
├── flaky.rs        # FlakyTracker worker + N 跑窗口
└── gate_loop.rs    # GateLoop worker, 跑完触发 Gate
```

总线: `tokio::sync::broadcast::Sender<ReliabilityEvent>`, 容量 1024.
慢消费者 → `RecvError::Lagged(n)` → warn, 跳到最新. 不重投, 不持久化事件流.

Worker: `pub async fn run(bus, ctx) -> anyhow::Result<()>`. `tokio::select!` 监听
bus + `CancellationToken`. 生命周期由 `pipeline::run()` 的 `JoinSet` 管理.

## Events

```rust
pub enum ReliabilityEvent {
    CaseGenerated    { case_id: String, yaml: String, critic_score: Option<u8> },
    CriticRejected   { case_id: String, reason: String, attempt: u8 },

    CaseExecuted     { case_id: String, run_id: String, passed: bool,
                       error: Option<String>, failed_step: Option<String> },

    HealRequested    { case_id: String, original_yaml: String,
                       error: String, failed_step: String, attempt: u8 },
    HealProduced     { case_id: String, healed_yaml: String, attempt: u8 },
    HealExhausted    { case_id: String, attempts: u8, last_error: String },

    FlakyDetected    { case_id: String, flip_rate: f64, window: usize },
    CaseQuarantined  { case_id: String, reason: String },

    RunFinished      { run_id: String },
    GateDecided      { run_id: String, passed: bool, reasons: Vec<String> },
}
```

关键字段:
- `failed_step: Option<String>` — 修复 Healer 硬编码 `"aiAssert"` 的 bug.
  需在 sandbox 协议加同名字段; 无回退时填 `"unknown"`.
- `attempt: u8` — 阶段本地计数, 跨阶段不共享.

## Workers

| Worker | 订阅 | 产出 | 边界 |
|---|---|---|---|
| Generate | 启动 / `CriticRejected` | `CaseGenerated` | max 3 次重生 |
| Critic | `CaseGenerated` | 透传 或 `CriticRejected` | 评分 ≥ 70 透传, 否则回 Generate |
| Execute | `CaseGenerated` / `HealProduced` | `CaseExecuted` | 复用 `TestExecutor::run_case` |
| Heal | `CaseExecuted{passed:false}` | `HealProduced` / `HealExhausted` | 拿真 `failed_step`, max 2 次 |
| FlakyTracker | 所有 `CaseExecuted` | `FlakyDetected` / `CaseQuarantined` | N=10 窗口, flip_rate>0.3 报, 连续 3 run flip 隔 |
| GateLoop | `RunFinished` | `GateDecided` | 复用 `GateService::check` |

`HealExhausted` 不直接 quarantined — 留给 FlakyTracker 用历史决定, 单次失败不算抖.

## Data Flow

```
启动
  → Generate 产 CaseGenerated{score:None}
  → Critic 评分, ≥70 透传
  → Execute 跑用例
       ├─ pass → FlakyTracker 记
       └─ fail → Heal(attempt=1)
            ├─ 修好 → Execute 重跑 (传 HealProduced)
            └─ 没修好 → Heal(attempt=2)
                 ├─ 修好 → Execute
                 └─ 仍 fail → HealExhausted
  → 所有 case 完 → RunFinished
  → GateLoop → GateDecided
```

## Persistence

复用 `~/.qinAegis/...` 布局. 三个文件:

- `runs/{run_id}/summary.json` — 加 `pipeline_meta: { heal_attempts, flaky_score, critic_score }` 字段, 向后兼容.
- `flaky_history.json` — 新. `{"{case_id}": [pass, fail, pass, ...]}`, 滑动窗口 N=10. 写用 `tempfile + rename` 防半写.
- `quarantined.json` — 新. `["case_id", ...]`. 启动时读, 命中 case 跳过执行.

**Heal 产物:** 存 `runs/{run_id}/healed/{case_id}.yaml`, 不覆盖原 yaml.
原 case 永远不动 — 与 healer 注释一致 (`never overwriting approved assets`).

## Bounded Loops + Error Handling

**有界:**

| 阶段 | 封顶 | 超限 |
|---|---|---|
| Generate 重生成 | 3 | 升级人工, 留 `CriticRejected` final |
| Heal 重试 | 2 | `HealExhausted`, 进 run 结果 |
| 总 run 时长 | 30 min | watchdog tokio task, 强 shutdown |

**三类错误:**
1. 可恢复 (LLM 429, 网络瞬断) — worker 内 `retry_with_backoff(3, 100ms..2s)`.
2. 数据错 (yaml 解析炸) — 跳过该 case, 记 `error: "skipped: bad yaml"`, 不发事件, 必写盘.
3. 致命 (storage 写不了, bus 关闭) — worker 返回 `Err`, pipeline 聚合上报.

**shutdown:** `CancellationToken` → 所有 worker `tokio::select!` 退出. pipeline
等 `JoinSet` 5s, 超时强杀.

## Testing

随模块单元:
- `flaky::update_window()` — pass/fail 序列 → flip_rate + 阈值触发. 核心算法, 必测.
- `bus` — 多订阅者 + slow consumer lagged 跳最新.
- healer 回归 — 给 `failed_step="aiAct: click X"`, prompt 出现真 step, 不出现裸 `"aiAssert"`.

集成 (`tests/pipeline_loop.rs`): mock LlmClient + mock MidsceneProcess, 端到端
fake run, 断言收到 `GateDecided{passed:true}` + flaky_history 有第二次 pass.

契约测试 (不进 CI, 跑慢): 对真 midscene 沙箱跑 1 case, 验证协议 `failed_step` 字段能拿到.

**主动 skip:** 性能 / 负载 / UI 端到端 / 模糊测试 — 单机桌面 app, 无业务价值.

## Out of Scope

- Web UI 改动 — pipeline 只暴露 CLI 入口 `qinAegis pipeline run <project>`.
- 多 run 并行 — 单 worker pool, 跑完一个再下一个, 防资源争抢.
- 持久化事件流 — bus 进程内一次, 审计走 summary.json + reporter.html.
- AI 自动改 spec — 留给后续 spec-drift 阶段.

## Rollout

1. events.rs + bus.rs 落地, 编译过, 无 worker.
2. Generate + Critic 接现有 generator/critic, 走通到 `CaseGenerated`.
3. Execute 接入, 修 `failed_step` 协议字段.
4. Heal 接入, 替换 executor 内联 heal 调用.
5. FlakyTracker + quarantined 读写.
6. GateLoop 接入.
7. 集成测试 + 契约测试.
