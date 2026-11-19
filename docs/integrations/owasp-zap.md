# 历史 ZAP 集成提案

> **归档标记：2026-11-16。以下原文保留用于历史追溯，不是当前产品方案或可直接执行的操作手册。** 下文为外部安全扫描能力的集成设想，不代表 2.0 已集成、已授权扫描或已验收。命令、镜像与第三方 API 未在本次重新验证，不应直接用于真实目标。

当前 2.0 入口：[产品记忆](../../CONTEXT.md) · [完整产品与实施方案](../plans/local-web.md) · [已验证进度与缺口](../progress/local-web-2.0.md)。

2.0 的核心是可跨项目复用、维护、修订和人工纠正的契约。产品运行形态是本地 HTTP 服务与浏览器 Web 工作台，无桌面客户端打包；模型通过可扩展适配器与路由切换，视觉执行保留 Playwright + Midscene.js，报告采用官方 Allure。引擎按需安装、完整场景编排、任务恢复和经明确授权的无障碍电脑控制仍有待交付或验收，不能因以下提案而标记完成。

旧 TUI、桌面外壳、DMG/Cask、Notion 存储或其它引擎的描述均不能覆盖上述 2.0 方向。需要实际安装和配置时，请使用[安装说明](../../INSTALL_zh.md)；模型和测试页面数据授权以产品记忆为准。

---

## 历史原文

# OWASP ZAP Security Scanning Integration

## Overview

[OWASP ZAP (Zed Attack Proxy)](https://www.zaproxy.org/) is a free, open-source security scanner. This document describes how to integrate ZAP scanning into qinAegis workflows.

## Quick Start

### 1. Install ZAP

```bash
# macOS via Homebrew
brew install owasp-zap

# Or download from https://www.zaproxy.org/download/
```

### 2. Run Baseline Scan

```bash
# Quick baseline scan (passive scanning only)
zap-baseline.py -t https://your-app-url.com

# Active scan with attack
zap-full-scan.py -t https://your-app-url.com

# Docker
docker run -t owasp/zap2docker-stable zap-baseline.py -t https://your-app-url.com
```

### 3. Generate Report

```bash
# Generate HTML report
zap-baseline.py -t https://your-app-url.com -j report.html

# Generate JSON report for CI
zap-baseline.py -t https://your-app-url.com -J zap-report.json
```

## Integration with qinAegis

### Option 1: Shell Command Wrapper

Create a wrapper script `qinAegis-security`:

```bash
#!/bin/bash
# qinAegis Security Wrapper

set -e

PROJECT="$1"
URL="$2"
REPORT_DIR="${3:-.qinAegis/reports/security}"

mkdir -p "$REPORT_DIR"

echo "Running OWASP ZAP security scan for $PROJECT..."

# Run baseline scan
zap-baseline.py \
  -t "$URL" \
  -J "$REPORT_DIR/zap-report.json" \
  -j "$REPORT_DIR/zap-report.html" \
  -r "$REPORT_DIR/zap-report.html" \
  -l WARN \
  -m 3

# Parse results
FAIL_COUNT=$(jq '.site[].alerts[] | select(.riskdesc | contains("High") or contains("Medium")) | .name' "$REPORT_DIR/zap-report.json" | wc -l)

if [ "$FAIL_COUNT" -gt 0 ]; then
  echo "SECURITY ISSUES FOUND: $FAIL_COUNT high/medium risk alerts"
  exit 1
fi

echo "Security scan passed: no high/medium risk issues found"
exit 0
```

### Option 2: GitHub Actions Integration

Add to your CI workflow:

```yaml
security-scan:
  name: Security Scan
  runs-on: ubuntu-latest
  steps:
    - name: Checkout
      uses: actions/checkout@v4

    - name: Install ZAP
      run: |
        docker pull owasp/zap2docker-stable

    - name: Run Security Scan
      run: |
        docker run -v $(pwd):/zap/wrk:rw \
          owasp/zap2docker-stable \
          zap-baseline.py \
          -t ${{ env.APP_URL }} \
          -J zap-report.json \
          -r zap-report.html \
          -l WARN

    - name: Upload Report
      uses: actions/upload-artifact@v4
      with:
        name: security-report
        path: |
          zap-report.json
          zap-report.html

    - name: Fail on High Risk Issues
      if: failure()
      run: |
        FAIL_COUNT=$(jq '[.site[].alerts[] | select(.riskdesc | test("High|Medium"))] | length' zap-report.json)
        if [ "$FAIL_COUNT" -gt 0 ]; then
          echo "Found $FAIL_COUNT high/medium risk security issues"
          exit 1
        fi
```

## ZAP Scan Types

| Scan Type | Command | Description | Safety |
|-----------|---------|-------------|--------|
| Baseline | `zap-baseline.py` | Passive scan, no attacks | Safe |
| Full | `zap-full-scan.py` | Active scan with attacks | ⚠️ Can modify data |
| API | `zap-api-scan.py` | Scan OpenAPI/SPDYH contracts | Safe |
| GraphQL | `zap-graphql-scan.py` | Scan GraphQL endpoints | ⚠️ May trigger mutations |

## Interpreting Results

### Risk Levels

- **High**: Active exploitation likely, fix immediately
- **Medium**: Potential vulnerability, prioritize
- **Low**: Minor issue, address when possible
- **Informational**: Best practice violations, not vulnerabilities

### Common Alerts

| Alert | Risk | Description | Fix |
|-------|------|------------|-----|
| Absence of Anti-CSRF Tokens | Medium | Forms should include CSRF tokens | Add anti-CSRF tokens |
| Cross-Domain JavaScript Source File | Low | External JS inclusion | Review external scripts |
| Missing Content Security Policy Header | Low | CSP header missing | Add CSP header |
| Server Leaks Version Information | Low | Server version exposed | Hide server version |

## Automation Examples

### Pre-commit Hook

```bash
#!/bin/bash
# .git/hooks/pre-commit

echo "Running security scan..."

# Only scan changed files
CHANGED_FILES=$(git diff --cached --name-only)
if echo "$CHANGED_FILES" | grep -q "\.js$"; then
  zap-baseline.py -t https://staging.example.com -l PASS
fi
```

### Nightly Security Scan

```yaml
# .github/workflows/nightly-security.yml
name: Nightly Security Scan

on:
  schedule:
    - cron: '0 2 * * *'  # 2 AM daily

jobs:
  security:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: zaproxy/action-baseline@v0.9.0
        with:
          target: 'https://your-app-url.com'
          token: ${{ secrets.GITHUB_TOKEN }}
```

## Resources

- [OWASP ZAP Documentation](https://www.zaproxy.org/docs/)
- [ZAP API](https://www.zaproxy.org/docs/api/)
- [ZAP Docker Images](https://www.zaproxy.org/docs/docker/)
- [ZAP GitHub Action](https://github.com/marketplace/actions/owasp-zap)
