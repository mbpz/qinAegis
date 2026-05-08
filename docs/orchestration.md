# qinAegis CI/CD Orchestration Guide

## Overview

This document describes how to orchestrate the complete qinAegis testing pipeline from URL input through quality gate using GitHub Actions.

## Pipeline Flow

```
URL Input
    │
    ▼
┌─────────────────────────────────────────────────────────────────┐
│  Stage 1: EXPLORE - AI-powered project discovery                 │
│  qinAegis explore --project X --url Y --max-depth 3              │
│  Output: spec/product.md, routes.json, ui-map.json               │
└─────────────────────────────────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────────────────────────────────┐
│  Stage 2: GENERATE - AI-powered test case generation            │
│  qinAegis generate --project X --spec-path S --requirement R    │
│  Output: cases/draft/*.json                                     │
└─────────────────────────────────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────────────────────────────────┐
│  Stage 3: REVIEW - Human/AI review                              │
│  qinAegis review --project X                                    │
│  Output: cases/reviewed/*, cases/approved/*                     │
└─────────────────────────────────────────────────────────────────┘
    │
    ├──────────────────────────────────────────────────────────────┤
    │                                                              │
    ▼                                                              ▼
┌───────────────────────┐  ┌───────────────────────┐  ┌───────────────────────┐
│  Stage 4a: E2E Tests  │  │  Stage 4b: Performance │  │  Stage 4c: Stress     │
│  qinAegis run --type  │  │  qinAegis performance  │  │  qinAegis stress       │
│    smoke              │  │    --url U --threshold│  │    --target U         │
│                       │  │    80                 │  │    --users 100        │
└───────────────────────┘  └───────────────────────┘  └───────────────────────┘
    │                          │                          │
    └──────────────────────────┼──────────────────────────┘
                               ▼
┌─────────────────────────────────────────────────────────────────┐
│  Stage 5: QUALITY GATE                                          │
│  qinAegis gate --project X --e2e-threshold 95                   │
│           --perf-threshold 80 --stress-rps-min 100 ...           │
│  Exit code: 0 = pass, non-zero = fail                           │
└─────────────────────────────────────────────────────────────────┘
                               │
                               ▼
                    ┌───────────────────────┐
                    │  Stage 6: Summary     │
                    │  Artifact upload       │
                    │  PR comment           │
                    └───────────────────────┘
```

## GitHub Actions Workflows

### 1. Full Pipeline Workflow

**File**: `.github/workflows/qinAegis-orchestration.yml`

Runs all stages: explore → generate → review → e2e → performance → stress → gate

### 2. Quick Smoke Workflow

For fast feedback on PRs:

```yaml
quick-smoke:
  name: Quick Smoke
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    - name: Install qinAegis
      run: |
        curl -fsSL https://github.com/qinaegis/qinAegis/releases/latest/download/qinAegis-x86_64-unknown-linux-gnu.tar.gz | tar xz
    - name: Run Smoke
      run: ./qinAegis run --project ${{ env.QINAEGIS_PROJECT }} --test-type smoke
```

### 3. Nightly Full Pipeline

```yaml
nightly-full:
  name: Nightly Full Pipeline
  on:
    schedule:
      - cron: '0 2 * * *'  # 2 AM daily
    workflow_dispatch:
```

## Reusable Workflow Pattern

### Reusable Workflow (called workflow)

```yaml
# .github/workflows/reusable/qinAegis-gate.yml
name: qinAegis Quality Gate

on:
  workflow_call:
    inputs:
      project:
        required: true
        type: string
      e2e_threshold:
        required: false
        type: number
        default: 95
      perf_threshold:
        required: false
        type: number
        default: 80

jobs:
  gate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install qinAegis
        run: curl -fsSL https://github.com/qinaegis/qinAegis/releases/latest/download/qinAegis-x86_64-unknown-linux-gnu.tar.gz | tar xz
      - name: Run Gate
        run: |
          ./qinAegis gate \
            --project ${{ inputs.project }} \
            --e2e-threshold ${{ inputs.e2e_threshold }} \
            --perf-threshold ${{ inputs.perf_threshold }} \
            --output-json
```

### Calling Workflow

```yaml
# .github/workflows/myapp-ci.yml
jobs:
  e2e:
    needs: build
    uses: ./.github/workflows/reusable/qinAegis-gate.yml
    with:
      project: my-webapp
      e2e_threshold: 90
    secrets: inherit
```

## Matrix Strategy for Multi-Project Testing

```yaml
jobs:
  test-all-projects:
    strategy:
      matrix:
        project: [admin, user-portal, checkout, dashboard]
        include:
          - project: admin
            url: https://admin.example.com
          - project: user-portal
            url: https://user.example.com
          - project: checkout
            url: https://checkout.example.com
          - project: dashboard
            url: https://dashboard.example.com
    steps:
      - name: Explore ${{ matrix.project }}
        run: |
          ./qinAegis project add --name ${{ matrix.project }} --url ${{ matrix.url }}
          ./qinAegis explore --project ${{ matrix.project }} --max-depth 2

      - name: Generate for ${{ matrix.project }}
        run: |
          ./qinAegis generate --project ${{ matrix.project }} --requirement "Full coverage"

      - name: Run for ${{ matrix.project }}
        run: |
          ./qinAegis run --project ${{ matrix.project }} --test-type smoke

      - name: Gate for ${{ matrix.project }}
        run: |
          ./qinAegis gate --project ${{ matrix.project }}
```

## Environment Variables and Secrets

### Required Secrets

| Secret | Description | Example |
|--------|-------------|---------|
| `MIDSCENE_MODEL_BASE_URL` | LLM API base URL | `https://api.minimax.chat/v1` |
| `MIDSCENE_MODEL_API_KEY` | LLM API key | `sk-...` |

### Optional Secrets

| Secret | Default | Description |
|--------|---------|-------------|
| `MIDSCENE_MODEL_NAME` | `MiniMax-VL-01` | Model name |
| `MIDSCENE_MODEL_FAMILY` | `openai` | Model family |

### Repository Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `QINAEGIS_PROJECT` | `my-webapp` | Project name |
| `TARGET_URL` | `https://your-app.com` | Target URL |

## Gate Thresholds

| Metric | Default | CLI Flag |
|--------|---------|----------|
| E2E Pass Rate | 95% | `--e2e-threshold` |
| Performance Score | 80 | `--perf-threshold` |
| Stress RPS Min | 100 | `--stress-rps-min` |
| Stress P95 Max | 2000ms | `--stress-p95-max` |
| Stress Error Rate Max | 5% | `--stress-error-max` |

## Artifact Retention

| Artifact | Retention | Purpose |
|----------|-----------|---------|
| `exploration-spec-*` | 30 days | Spec files, routes |
| `generated-cases-*` | 30 days | Draft test cases |
| `review-report-*` | 30 days | Review status |
| `e2e-smoke-results-*` | 30 days | E2E test outputs |
| `performance-results-*` | 30 days | Lighthouse results |
| `stress-results-*` | 30 days | k6 results |
| `gate-report-*` | 90 days | Gate decision artifacts |

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Quality gate passed |
| 1 | Quality gate failed (threshold not met) |

## Integration Examples

### With GitHub PR Checks

```yaml
on:
  pull_request:
    branches: [main]

jobs:
  qinAegis:
    name: qinAegis Quality Gate
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install
        run: curl -fsSL https://github.com/qinaegis/qinAegis/releases/latest/download/qinAegis-x86_64-unknown-linux-gnu.tar.gz | tar xz
      - name: Configure
        env:
          MIDSCENE_MODEL_BASE_URL: ${{ secrets.MIDSCENE_MODEL_BASE_URL }}
          MIDSCENE_MODEL_API_KEY: ${{ secrets.MIDSCENE_MODEL_API_KEY }}
        run: |
          echo "MIDSCENE_MODEL_BASE_URL=$MIDSCENE_MODEL_BASE_URL" >> $GITHUB_ENV
          echo "MIDSCENE_MODEL_API_KEY=$MIDSCENE_MODEL_API_KEY" >> $GITHUB_ENV
      - name: Run Pipeline
        run: |
          ./qinAegis run --project ${{ vars.QINAEGIS_PROJECT }} --test-type smoke
          ./qinAegis gate --project ${{ vars.QINAEGIS_PROJECT }}
```

### With Deployment Pipeline

```yaml
jobs:
  # ... test stages ...

  deploy:
    name: Deploy
    needs: [quality-gate]
    if: needs.quality-gate.result == 'success'
    steps:
      - name: Deploy to Staging
        run: echo "Deploying..."
```

## Local Testing

Test the pipeline locally:

```bash
# Explore
qinAegis explore --project my-app --url http://localhost:3000 --max-depth 3

# Generate
qinAegis generate --project my-app --spec-path ~/.qinAegis/projects/my-app/spec/product.md --requirement "Login flow"

# Run tests
qinAegis run --project my-app --test-type smoke

# Run performance
qinAegis performance --url http://localhost:3000 --threshold 80

# Run stress
qinAegis stress --target http://localhost:3000 --users 100 --duration 60

# Quality gate
qinAegis gate --project my-app --e2e-threshold 95 --perf-threshold 80 --stress-rps-min 100
```

## Resources

- [qinAegis GitHub](https://github.com/qinaegis/qinAegis)
- [CI Workflow Example](./.github/workflows/ci-qinAegis.yml)
- [Full Orchestration](./.github/workflows/qinAegis-orchestration.yml)