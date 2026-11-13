# QinAegis 2.0

A local Web AI quality workbench based on **Playwright + Midscene.js visual testing**. Install the service, run `qinaegis`, and use your default browser for configuration and testing. The UI is embedded; there is no desktop client bundle.

The product's central asset is the reusable test contract: business intent, inputs, preconditions, assertions and evidence remain separate from project bindings, models and engines. Versioned correction, extensible model routing and composable scenarios are the 2.0 direction. See [project memory](CONTEXT.md) and the [implementation plan](docs/plans/local-web.md) for scope and acceptance criteria.

This branch is 2.0 development, not a claim that every planned feature is shipped. Contract/scenario domain foundations and the local HTTP migration are being delivered in phases. Desktop accessibility assistance is planned and requires explicit OS authorization; it is not enabled by installation.

## Build and run

```bash
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

Projects remain under `~/.qinAegis/projects`. Global configuration follows the OS-specific configuration directory; consult service diagnostics rather than assuming a fixed macOS path. Models may receive test-page content during AI operations; credentials and private project data should not be included in shared contract packages.

## Release artifacts

`bash scripts/build-release.sh TARGET` builds UI plus a service archive. Targets are macOS ARM64/Intel, Linux x86_64 and Windows x86_64; platform availability remains subject to CI and clean-machine verification. No `.app`, DMG or cask is produced. Workflows produce a **draft** release for review before publication. The Homebrew Formula is rendered from verified release checksums; the repository template is not directly installable before release.

[中文说明](README_zh.md) · [安装说明](INSTALL_zh.md) · [License](LICENSE)
