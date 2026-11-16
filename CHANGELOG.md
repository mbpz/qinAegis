# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [2.0.0 — Unreleased]

Development branch, not a published stable release. Current scope and remaining acceptance are tracked in [product memory](CONTEXT.md) and [verified progress](docs/progress/local-web-2.0.md).

### Added

- Reusable contract library with immutable published versions, project bindings, reviewed corrections, comparison, deprecation, derivation, controlled import/export and reviewed binding migration.
- Extensible model registry, capability/data authorization, task routing and frozen configuration snapshots; current built-in protocol is Chat Completions.
- Atomic case generation provenance and immutable execution evidence; infrastructure/lifecycle failure blocks the quality gate even when returned cases passed.
- Official Allure 3.33.0 standard results and on-demand isolated HTML generation, viewing and explicit download.
- Four-platform CI candidates with version/commit manifests, archive validation and SHA-256 checksums; matching v2 tags create draft releases.
- Independent contract-focused GitHub Pages build, PR preview artifacts and main-only deployment.

### Changed

- Replaced the tao/wry desktop shell with an embedded Web UI and a loopback-only HTTP service: background start, browser opening, status and safe stop, private sessions and CSRF checks.
- Adopted official free beUI interactions throughout the workbench and public site.
- Made engine discovery explicit through installed runtime manifests or an opted-in development sandbox.
- Replaced roadmap file/name counts with executable repository checks and explicit acceptance gaps.

### Removed

- Desktop client, DMG and Homebrew Cask packaging.
- Execution shortcuts that reused historical passes without running the original actions/assertions.
- Tracking scripts and unsupported offline/installation claims from the public website.

### Pending acceptance

- Real dual-project visual execution with authorized compatible models; installed runtime preparation/diagnostics; durable tasks, cancellation/recovery and runnable scenario orchestration.
- Clean-machine installation, Windows one-command installer, signing, upgrade/rollback and formal release.
- Optional OS accessibility assistance remains planned and requires separate permission/platform acceptance. Model protocol expansion and project/step routing remain future work; current registration does not imply online compatibility testing.

## Historical entries

The entries below preserve prior release notes. They are not the 2.0 installation guide or verified current capability inventory. Old desktop/Cask paths, action-cache claims, Keychain/offline claims and other proposed integrations must not be applied to 2.0; use the current [installation guide](INSTALL.md) and [user guide](docs/USER_GUIDE.md).

## [0.1.0] - 2026-05

### Added

- **Desktop GUI Application** — macOS native app via tao + wry + React WebView
  - Double-click to launch from `/Applications`, no terminal required
  - 7-page SPA: Dashboard, Explore, Generate, Run, Reports, Review, Settings
  - Init wizard for first-time LLM configuration
  - RPC bridge between React frontend and Rust backend
  - Debug console toggle (Cmd+Shift+D)
- **AI-Powered Project Exploration** — Midscene.js visual-driven page discovery
  - BFS route crawling with configurable depth
  - Accessibility snapshot + DOM structure analysis
  - Auto-generated spec.md saved to local project directory
- **Test Case Generation** — Natural language to executable Midscene YAML
  - LLM-driven test generation from requirement descriptions
  - Draft / reviewed / approved / flaky / archived lifecycle management
  - AI Critic automatic review mode
- **Test Execution** — 4 test types via sandboxed Playwright browser
  - Smoke testing (P0 approved cases, parallel execution)
  - Functional testing (full case coverage with boundary/edge cases)
  - Performance testing (Lighthouse CI Web Vitals: LCP, FCP, CLS, TBT)
  - Stress testing (k6/locust load testing with thresholds)
  - Security scanning (OWASP ZAP baseline scan)
  - Natural language test plan preview before execution
- **Self-Healing** — AI-powered broken locator repair
  - LLM generates corrected Midscene YAML on failure
  - Original approved YAML preserved (no pollution)
- **Action Caching** — Stagehand-style LRU cache for AI decisions
  - 30-minute TTL, 500-entry limit, reduces LLM API costs
- **Quality Gates** — Unified gate calculation
  - E2E pass rate threshold
  - Lighthouse performance budget
  - Stress test RPS thresholds
- **Local Storage** — All data under `~/.qinAegis/projects/`
  - Project config, specs, requirements, cases, reports
  - No cloud dependency, complete offline capability
  - macOS Keychain integration for LLM API keys (keyring crate)
- **Distribution** — Homebrew cask + GitHub Actions CI/CD
  - `brew install --cask mbpz/homebrew-qinAegis/qinaegis` one-click install
  - Ad-hoc code signing, DMG packaging
  - Auto-publish to Homebrew tap on release
  - Dual-architecture support (aarch64 + x86_64)

### Technical Stack

| Layer | Technology |
|-------|------------|
| Desktop App | Rust + tao (window) + wry (WebView) |
| Frontend | React 18 + TypeScript + Vite |
| Core Services | Rust + tokio async runtime |
| Browser Automation | Playwright (Chromium) |
| AI Execution Engine | Midscene.js (visual act/assert/extract) |
| Performance Testing | Lighthouse CI |
| Stress Testing | k6 / locust |
| Storage | Local filesystem (`~/.qinAegis/`) |

### Changed

- Migrated from CLI/TUI to WebView GUI desktop application
- Storage backend replaced: Notion cloud → local filesystem
- AI model requirement updated: pure text → vision-capable (MiniMax-VL, Qwen3-VL, UI-TARS)

[0.1.0]: https://github.com/mbpz/qinAegis/releases/tag/v0.1.0
