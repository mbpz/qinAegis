# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Planned

- Multi-LLM dynamic switching (route between MiniMax-VL / Qwen3-VL / UI-TARS based on page complexity)
- Visual regression dashboard (like lost-pixel / Percy)
- Accessibility testing (WCAG compliance checks)
- CI/CD native GitHub Actions integration with PR comments
- Mobile testing support (iOS / Android)

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
