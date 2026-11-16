# QinAegis 2.0

A local Web AI quality workbench based on **Playwright + Midscene.js visual testing**. Install the service, run `qinaegis`, and use your default browser for configuration and testing. The UI is embedded; there is no desktop client bundle.

The product's central asset is the reusable test contract: business intent, inputs, preconditions, assertions and evidence remain separate from project bindings, models and engines. Versioned correction, extensible model routing and composable scenarios are the 2.0 direction. See [project memory](CONTEXT.md) and the [implementation plan](docs/plans/local-web.md) for scope and acceptance criteria.

This branch is 2.0 development, not a claim that every planned feature is shipped. Local Web operation, contract lifecycle and execution provenance, model registration/routing, and official Allure reporting are implemented. Real business visual execution, automatic engine installation, durable task recovery and clean-machine acceptance remain open. Desktop accessibility assistance is planned and requires explicit OS authorization; it is not enabled by installation.

## Product structure and current scope

The same published contract can be bound to two projects with different parameters. A correction creates a reviewed revision; existing executions retain their original inputs and evidence. Models may propose changes but cannot approve a weaker business assertion merely to obtain a pass.

| Responsibility | Current implementation and limits |
|---|---|
| Contract governance | Draft/edit/review/publish, version comparison, correction evidence, deprecation, derivation, reviewed binding migration and controlled import/export |
| Model routing | Private registry, capability/data authorization, task routes and frozen snapshots; Chat Completions adapter today, bounded generation fallback, one compatible visual primary |
| Execution | Playwright + Midscene.js through an explicit runtime boundary; approved scripts and contract inputs fixed before execution |
| Evidence and quality | Immutable run records, failed lifecycle blocks the gate, official Allure generation/view/download; single-run reports today |
| Scenario composition | Dependency validation and stable ordering exist; a runnable orchestration engine, typed data flow and flow editing remain planned |
| Optional computer control | OS accessibility adapter planned, disabled until explicit permission and platform acceptance |

The domain owns contracts and validation rules. Model adapters generate suggestions, execution adapters perform actions, and reporting preserves facts. Their responsibilities stay separate so changing a model or engine does not rewrite business expectations. The Web UI uses official free beUI components.

## Build and run

The 2.0 development source is on `codex/qinaegis-2.0` until it is merged into `main`. The previous public release uses a different architecture; do not use a generic “latest” download to obtain 2.0.

```bash
git clone --branch codex/qinaegis-2.0 https://github.com/mbpz/qinAegis.git
cd qinAegis
cd crates/web_client/ui
npm ci
npm run build
cd ../../..
cargo build --locked --release --bin qinaegis
./target/release/qinaegis
```

The service listens only on loopback, default port `19876`, and opens a browser with a short-lived session bootstrap token. Keep this private URL out of shared logs. No AI key is needed to browse existing assets. Configure models in Settings to use AI operations.

```bash
qinaegis status
qinaegis stop
qinaegis serve --no-open
qinaegis start --port 19877
```

Use the executable's full path if it is not on PATH. Closing the browser does not stop the service. A reopened private session should use `qinaegis open`.

## Installation and engines

See [INSTALL.md](INSTALL.md). The base executable embeds React assets; Rust and frontend build tools are needed only for source builds. **Actual visual tests still require a prepared Playwright + Midscene.js engine**, including compatible Node and automation browser dependencies. Automatic engine installation and complete Web engine management are subsequent phases.

- Installed runtime: `QINAEGIS_RUNTIME_DIR` points to an absolute directory containing a validated `runtime.json`, compiled executor and dependencies.
- Development runtime: `QINAEGIS_DEV_SANDBOX` explicitly points to an absolute prepared `sandbox` source directory.
- Runtime discovery does not rely on the compiler's source directory or current working directory.
- Performance and stress adapters require their own verified Lighthouse/Locust environments.

By default, projects remain under `~/.qinAegis/projects`. Global configuration follows the OS-specific configuration directory; consult service diagnostics rather than assuming a fixed macOS path. For an isolated workspace, set `QINAEGIS_DATA_DIR` to an absolute directory before starting; it covers private data and configuration without migrating existing assets. See the installation guide for restart and path rules. Models may receive test-page content during AI operations; credentials and private project data should not be included in shared contract packages.

## Release artifacts

`bash scripts/build-release.sh TARGET` builds UI plus a service archive. Targets are macOS ARM64/Intel, Linux x86_64 and Windows x86_64; platform availability remains subject to CI and clean-machine verification. No `.app`, DMG or cask is produced. Branch/PR builds produce candidate artifacts, and matching `v2.*` tags produce a **draft** release after checks. Artifacts include provenance metadata and checksums. A candidate build is not a signed production installer. The Homebrew Formula is rendered only from verified, published stable release assets; the repository template is not directly installable before release.

## Allure and the public website

In Reports, select a saved run to generate, view or download an official **Allure 3.33.0** single-file report. Report generation uses the prepared Node engine, does not rerun tests, and preserves execution failures independently of individual case passes. HTML downloads work offline; attachments can contain project inputs, scripts and screenshots, so review them before sharing.

The public GitHub Pages site describes the product; the private workbench runs on your computer. Pages builds independently with `npm run build:pages` in `crates/web_client/ui`, producing `build/pages`. Pull requests build a preview artifact; deployment follows reviewed changes to `main`. See [CI/release maintenance](docs/CI_INTEGRATION.md) and [verified progress and remaining acceptance](docs/progress/local-web-2.0.md).

## Documentation

Start with the [documentation index](docs/README.md) and [user guide](docs/USER_GUIDE.md). The [product plan](docs/plans/local-web.md) defines the target; [progress](docs/progress/local-web-2.0.md) records implemented behavior and acceptance gaps; the [roadmap](qinAegis-platform-roadmap.md) sets the next order. Historical designs and reviews are labeled as archives and do not override 2.0 scope.

[中文说明](README_zh.md) · [安装说明](INSTALL_zh.md) · [License](LICENSE)
