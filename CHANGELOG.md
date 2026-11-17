# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [2.0.0 — Unreleased]

Development branch, not a published stable release. Current scope and remaining acceptance are tracked in [product memory](CONTEXT.md) and [verified progress](docs/progress/local-web-2.0.md).

### Added

- Reusable contract library with immutable published versions, project bindings, reviewed corrections, comparison, deprecation, derivation, controlled import/export and reviewed binding migration.
- Extensible model registry, capability/data authorization, task routing and frozen configuration snapshots; built-in Chat Completions and native Anthropic Messages text/JSON protocols.
- Atomic case generation provenance and immutable execution evidence; infrastructure/lifecycle failure blocks the quality gate even when returned cases passed.
- Official Allure 3.33.0 standard results and on-demand isolated HTML generation, viewing and explicit download.
- Four-platform CI candidates with version/commit manifests, archive validation and SHA-256 checksums; matching v2 tags create draft releases.
- Independent contract-focused GitHub Pages build, PR preview artifacts and main-only deployment.

- Project-scoped model overrides, synthetic text/JSON capability observations and actual generator/critic attempt records.
- Standalone compiled runtime packaging with bundled Node, validated offline import, fixed Web diagnostics and explicit Chromium installation.
- Atomic configuration CAS, private backup and explicit recovery; bounded durable task history, incremental logs and interrupted recovery without replay.
- Full case editing with original-content comparison, approval reset, exact-content review/approval and recoverable case transitions.

- Reviewed migration from fixed configuration sources with explicit dual-credential policy, exact-content conflict checks, idempotence and backup.
- Explicit cleanup of verified owned Allure staging residue with expiring previews; execution facts and completed reports remain intact.
- Windows managed Job Objects bound before child execution, owned-tree cancellation and cleanup error propagation; verified in actual Windows CI; per-engine timing remains pending.

- One execution slot with up to eight FIFO waiting tasks, frozen submission inputs, visible queue positions and queued cancellation without replay after restart.
- Separate owned Chromium profiles and private engine workspaces, actual browser isolation and relocated bundled-runtime checks.
- Pinned workspace directory identity checks, bounded ownership markers and explicit cleanup failure propagation; abandoned work retains files without leaking the directory handle. New Windows directory handling awaits its own CI; cross-restart recovery remains pending.
- Managed project specifications with bounded reads, explicit approved CAS saves and frozen generator/critic inputs; core/HTTP/UI and actual browser checks passed, including conflict retention and delayed-save project isolation.
- Lighthouse/Locust failure and metric validation; ZAP explicitly unavailable until managed container cancellation is implemented.

- Versioned JSON flow drafts with CAS, server-owned capability/type/dependency validation, explicitly approved immutable publication and frozen project plan history. Planning only: all runners unavailable and plans non-executable; core/HTTP, real Chromium and independent review passed for this planning scope.
- Native Messages authentication, system/content blocks and structured output requests, bounded responses/deadlines, explicit rejection and approved cross-protocol fallback; Web protocol selection and saved-profile probe identity. Loopback protocol and actual browser acceptance do not certify paid models or visual business execution.

### Changed

- Replaced the tao/wry desktop shell with an embedded Web UI and a loopback-only HTTP service: background start, browser opening, status and safe stop, private sessions and CSRF checks.
- Adopted official free beUI interactions throughout the workbench and public site.
- Made engine discovery explicit through installed runtime manifests or an opted-in development sandbox.
- Added an absolute `QINAEGIS_DATA_DIR` for isolated private workspaces across platforms while preserving default paths, and detached Windows service output handles from their launcher.
- Replaced roadmap file/name counts with executable repository checks and explicit acceptance gaps.

### Removed

- Desktop client, DMG and Homebrew Cask packaging.
- Execution shortcuts that reused historical passes without running the original actions/assertions.
- Tracking scripts and unsupported offline/installation claims from the public website.

### Pending acceptance

- Real dual-project visual execution with authorized compatible models; complete Web runtime preparation/upgrade; multi-resource concurrency, exceptional browser workspace cleanup and per-engine cancellation timing acceptance and runnable scenario orchestration.
- Clean-machine installation, Windows one-command installer, signing, upgrade/rollback and formal release.
- Optional OS accessibility assistance remains planned and requires separate permission/platform acceptance. Additional model protocols and step fallback policies remain future work; synthetic text/JSON probing does not certify visual compatibility.

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
