# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-04

### Added

- Initial release with core functionality:
  - TUI (Terminal UI) client with interactive dashboard
  - CLI commands: init, explore, generate, run, performance, stress, report, config
  - AI-powered project exploration using Midscene.js
  - Test case generation with LLM integration
  - Visual-driven browser automation (aiAct, aiQuery, aiAssert)
  - Smoke testing and functional testing execution
  - Performance testing with Lighthouse CI integration
  - Stress testing with k6 integration
  - Local filesystem storage for projects, specs, cases, and reports
  - Playwright-based browser sandbox (no Docker required)
  - Homebrew installation support
  - GitHub Actions CI/CD for automated releases

### Features

- **Explorer**: AI-driven web project exploration and specification generation
- **Generator**: Natural language to test case conversion (Midscene YAML format)
- **Executor**: Test execution with visual verification
- **Reporter**: Test result analysis and HTML report generation
- **Performance**: Lighthouse-based Web Vitals metrics (LCP, FCP, CLS, TBT)
- **Stress**: k6-based load testing with configurable users and duration

### Technical Stack

- Rust (workspace-based multi-crate)
- ratatui for TUI
- tokio for async runtime
- Midscene.js for browser automation
- Playwright for sandbox isolation (no Docker)

---

## [Unreleased]

### Planned

- Windows/Linux support
- Parallel test execution improvements
- Enhanced TUI interactive features
- Integration with more LLM providers
- Test case templates library
- CI/CD pipeline integrations

[0.1.0]: https://github.com/yourorg/qinAegis/releases/tag/v0.1.0
