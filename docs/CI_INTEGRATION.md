# QinAegis CI/CD 集成指南

> 本文档介绍如何在 GitHub Actions 中集成 QinAegis 质量门禁流水线

## 概述

QinAegis 提供三个 GitHub Actions workflow：

| Workflow | 用途 | 触发 |
|----------|------|------|
| `ci-qinAegis.yml` | E2E + Performance + Stress + Gate | push / PR |
| `qinAegis-orchestration.yml` | 完整流水线（explore→generate→review→run→gate） | push / PR / dispatch |
| `publish-homebrew-tap.yml` | 发布 Homebrew Cask | release published |

---

## 1. 质量门禁（E2E + Performance + Stress）

文件：`.github/workflows/ci-qinAegis.yml`

### 流水线结构

```
e2e-tests → performance → stress-test → quality-gate
```

### 配置

在 GitHub仓库设置中添加以下 **Secrets**：

| Secret | 说明 | 示例 |
|--------|------|------|
| `MIDSCENE_MODEL_BASE_URL` | LLM API 端点 | `https://api.minimax.chat/v1` |
| `MIDSCENE_MODEL_API_KEY` | MiniMax API Key | `eyJ...` |
| `MIDSCENE_MODEL_NAME` | 模型名称 | `MiniMax-VL-01` |

添加仓库 **Variable**：

| Variable | 说明 | 默认值 |
|----------|------|--------|
| `QINAEGIS_PROJECT` | 项目名称 | `my-webapp` |

### 阈值配置

```yaml
env:
  E2E_THRESHOLD: 95          # E2E 通过率最低要求（%）
  PERF_THRESHOLD: 80          # Performance Score 最低要求
  STRESS_RPS_MIN: 100        # 压测每秒请求数最低要求
  STRESS_P95_MAX: 2000       # P95 响应时间最高要求（ms）
  STRESS_ERROR_MAX: 5        # 错误率最高要求（%）
```

### 使用方式

复制到你的项目 `.github/workflows/` 目录，配置 Secrets 和 Variables 后自动生效。

---

## 2. 完整流水线（Orchestration）

文件：`.github/workflows/qinAegis-orchestration.yml`

### 流水线结构

```
Stage 1: Explore
    ↓
Stage 2: Generate
    ↓
Stage 3: Review
    ↓
Stage 4a: E2E Smoke     Stage 4b: Performance     Stage 4c: Stress
    ↓                      ↓                        ↓
Stage 5: Quality Gate（汇总评估）
    ↓
Stage 6: Summary + PR Comment
```

### 使用方式

```yaml
on:
  push:
    branches: [main, develop]
  workflow_dispatch:
    inputs:
      target_url:
        description: Target URL to test
        required: true
        default: "https://your-app.com"
      project_name:
        description: Project name in QinAegis
        required: true
        default: "my-webapp"
```

手动触发（workflow_dispatch）可自定义目标 URL 和项目名。

---

## 3. Homebrew Cask 发布

文件：`.github/workflows/publish-homebrew-tap.yml`

### 前提条件

1. GitHub Release 已发布（tag `v*`）
2. Release产物包含两个 DMG：
   - `QinAegis-{version}-mac-arm64.dmg`
   - `QinAegis-{version}-mac-x64.dmg`
3. `homebrew-tap` 仓库（`mbpz/homebrew-tap`）已创建

### 工作流程

1. 下载两个架构的 DMG
2. 计算 SHA256
3. 生成 `qinaegis.rb` cask 文件
4. Push 到 `homebrew-tap` 仓库

### 安装命令

```bash
brew install --cask mbpz/qinAegis/qinAegis
```

发布后用户执行上述命令即可安装，自动处理 ad-hoc 签名 + quarantine xattr 清除。

---

## 4. 流水线状态检测

### GitHub PR Checks

流水线完成后自动在 PR 中显示各 Stage 状态：

| Stage | Check Name |
|-------|-----------|
| E2E Smoke | `E2E Smoke Tests` |
| Performance | `Performance Tests` |
| Stress | `Stress Tests` |
| Quality Gate | `Quality Gate` |

### Gate 判定

Quality Gate 检查以下条件：

- **E2E Pass Rate** ≥ `E2E_THRESHOLD`
- **Performance Score** ≥ `PERF_THRESHOLD`
- **Stress RPS** ≥ `STRESS_RPS_MIN`
- **Stress P95** ≤ `STRESS_P95_MAX`
- **Stress Error Rate** ≤ `STRESS_ERROR_MAX`

全部通过 → Gate PASSED
任一未达 → Gate FAILED（阻止合并）

### 查看详细报告

每个 job 都会上传 artifact，包含：

- `e2e-results-*` — E2E 测试结果
- `performance-results-*` — Lighthouse 报告
- `stress-results-*` — Locust 压测结果
- `gate-report-*` — 质量门禁汇总

---

## 5. 本地运行 CLI（非 GitHub Actions）

> 注意：PC 客户端通过 GUI 操作，不提供本地 CLI。以下命令适用于 qinAegis CLI 二进制（Phase 4 规划中）。

```bash
# 安装 CLI（Phase 4 提供）
cargo install --path crates/cli

# 运行冒烟测试
./qinAegis run --project "my-webapp" --test-type smoke

# 运行性能测试
./qinAegis performance --url https://example.com

# 运行压测
./qinAegis stress --target https://example.com --users 100 --duration 60

# 触发质量门禁
./qinAegis gate --project "my-webapp" --e2e-threshold 95
```

---

## 6. 常见问题

### Q: `MIDSCENE_MODEL_API_KEY` 无效

API Key 必须与 `MIDSCENE_MODEL_BASE_URL` 匹配。确认 MiniMax Key 用于 MiniMax 端点，OpenAI Key 用于 OpenAI 兼容端点。

### Q: E2E Tests 超时

增加 `concurrency` 参数或减少并发数：

```yaml
- name: Run E2E Tests
  run: |
    ./qinAegis run --project "$QINAEGIS_PROJECT" --test-type smoke --concurrency 2
```

### Q: Performance 测试 Lighthouse 超时

Lighthouse 默认 120s 超时，复杂页面可调整 sandbox 超时：

```bash
export MIDSCENE_TIMEOUT_MS=180000
```

### Q: GitHub Actions 网络问题

部分 Steps 依赖 GitHub Actions runner 网络下载 qinAegis 二进制。如遇网络错误，可：

1. 使用 `actions/cache` 缓存二进制
2. 切换到 macOS runner（网络更稳定）
3. 配置代理：`HTTPS_PROXY` 环境变量