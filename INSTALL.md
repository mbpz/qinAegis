# Install the local Web service

2.0 is in development. Release installation below applies **after the corresponding verified release is published**. Until then, build from source using [README.md](README.md). Do not install the Formula template directly.

Automatic CI builds on `codex/qinaegis-2.0` and pull requests provide candidate service archives in the Actions run artifacts. Review the workflow commit, target and manifest, and verify the archive checksum. These are development candidates; a green build does not certify signing, clean-machine installation or a complete 2.0 GA. The public `v1.0` desktop release is not a 2.0 installer.

## macOS / Linux

Download `install.sh` from a trusted 2.0 release, review it, then run:

```bash
sh install.sh --version 2.0.0
```

The installer detects macOS ARM64/Intel or Linux x86_64, downloads the matching service archive and `SHA256SUMS`, checks the archive, installs to `~/.local/bin/qinaegis`, then starts it and opens the browser. It does not use sudo, modify shell profiles, or remove OS security protections. Add that directory to PATH yourself or run the absolute path. SHA-256 validates integrity against the release channel; it is not an independent publisher signature.

Options: `--prefix /absolute/bin/directory`, `--no-start`, and offline `--archive /path/package.tar.gz --checksum HEX_SHA256`. Stop an existing service before upgrading. A failed startup does not imply the installed executable was removed; use `status` and service logs for diagnosis.

```bash
~/.local/bin/qinaegis status
~/.local/bin/qinaegis open
~/.local/bin/qinaegis stop
~/.local/bin/qinaegis serve --no-open
```

## Windows

Download and review `install.ps1` from the same trusted 2.0 release or verified candidate. In PowerShell, run:

```powershell
& .\install.ps1 -Version 2.0.0
```

The script installs to `%LOCALAPPDATA%\QinAegis`, then starts the service and opens its local Web workbench. It uses Windows PowerShell 5.1 and system .NET; no administrator rights, Node or Python are required. It does not change execution policy or PATH. Follow your machine's script execution policy; do not disable system security protections.

Offline candidates use `-Archive 'C:\path with spaces\service.zip' -Checksum TRUSTED_SHA256 -NoStart`. `-Prefix` selects an absolute user-owned installation directory. SHA-256, exact ZIP members, platform/version metadata and existing installation receipts are checked before replacement. A running service or unknown installation contents are refused. Stop it with `& "$env:LOCALAPPDATA\QinAegis\qinaegis.exe" stop` before maintenance.

Upgrades preserve `qinaegis.previous.exe`. Use `-Rollback -NoStart` to exchange the two verified versions; an existing older backup requires explicit `-DiscardPrevious` for another upgrade. `-Uninstall` removes verified executables and their receipt while preserving project/configuration/runtime/report data. Use the same `-Prefix` for maintenance. After a forced interruption, retained stage/swap files require inspection before further maintenance. See the [Windows acceptance plan](docs/plans/windows-user-installation.md). Signing, clean desktop installation, default browser opening and stable online distribution remain separate acceptance gates.

## Playwright + Midscene.js runtime

The base package does not include the engine or download it automatically. Browsing assets and configuring the workbench require no Node runtime; executing engine-backed operations does.

An installed runtime directory must contain:

```json
{"schema_version":1,"engine":"midscene","version":"your-verified-engine-version","entry":"dist/executor.js","node":"bin/node"}
```

`entry` and optional `node` reference files inside that directory. Supply compiled executor resources, matching production dependencies and a compatible automation browser; a manifest alone is insufficient. For an explicit external override, set `QINAEGIS_RUNTIME_DIR` to its absolute path **before starting the service**. Managed imports use the default data directory without this variable. Environment changes require restarting the service.

For development only, prepare the repository's root `sandbox` dependencies/browser, then set `QINAEGIS_DEV_SANDBOX` to that absolute source directory. The development adapter uses `src/executor.ts` and `node_modules/tsx/dist/cli.mjs`. No implicit source-path fallback is used. Environment can manage the default installation; changing that installation does not change an external or developer override.

With compatible Node 24 installed, prepare the development engine from the 2.0 checkout:

```bash
cd sandbox
npx --yes pnpm@11.24.0 install --frozen-lockfile
npx --yes pnpm@11.24.0 run build
npx --yes pnpm@11.24.0 exec playwright install chromium
export QINAEGIS_DEV_SANDBOX="$PWD"
cd ..
./target/release/qinaegis
```

Linux may also need Playwright's documented OS browser dependencies. Configure a compatible visual model in the local Web workbench before executing AI actions. Official Allure HTML generation uses this same engine and the pinned `allure@3.33.0`; it requires no model call or Java server, but still needs the prepared Node dependencies. Downloading a report does not install or prepare the visual-test engine.


## Standalone runtime candidates

### Import and maintain through Environment

1. Choose **Runtime archive (.tar.gz)**, supply its trusted 64-character SHA-256, and click **Upload and verify archive**. The archive is limited to 1 GiB and transferred in 512 KiB chunks. The Rust service validates host, provenance, members and content without Python or global Node.
2. Review the verified version, source and digest, then select **Approve this exact runtime change** and **Confirm installation**. A preview expires after 10 minutes; changed state requires a new preview. Refresh restores a pending preview but clears approval. **Discard pending preview** releases it.
3. A successful upgrade retains the former current runtime as `previous`. If an older backup already exists, explicitly allow its discard after success. Use **Prepare rollback**, review and separately approve **Confirm rollback**. Content is checked again and failed activation restores the prior installation.
4. Wait for running, queued and cancelling work to finish before maintenance. Switching blocks new task admission through cleanup. A retained upload can be explicitly discarded; restart never replays installation automatically.

**Check available stable runtimes** explicitly queries compatible stable assets from the fixed `mbpz/qinAegis` GitHub publisher. Select a version and allow its download before verification; arbitrary URLs are not accepted. An empty list means no compatible archive/checksum pair is published, not that a candidate artifact has become a stable release. Network access occurs only on explicit query/download. Chromium remains separate.

### Build or import offline with Python

Build on the target host with Node 24 and pnpm 11.24.0: `python3 scripts/build-runtime.py`. It produces a separate archive and SHA-256 file under `build/runtime`; service archives do not include this engine. Verify the trusted checksum, stop the service, then import locally:

```bash
python3 scripts/install-runtime.py /absolute/runtime.tar.gz --sha256 TRUSTED_SHA256 --data-dir /absolute/private-data
```

Start subsequent service commands with `QINAEGIS_DATA_DIR=/absolute/private-data`. The installer verifies paths, host, content and checksum before switching `runtimes/midscene/current`, retaining the old directory as `previous`. It refuses an active service or an existing backup requiring a retention decision. A matching archive marker is insufficient: installed files are checked again. This fallback requires Python on the importing machine and a stopped service. The Web import above runs inside the Rust service and has no Python prerequisite. Complete platform distribution acceptance remains open.

In **Environment**, check runtime/browser facts, explicitly authorize **Install Chromium**, then inspect the actual launch result. Downloads use the selected runtime cache and show safe per-component progress, followed by an actual Chromium launch check. Stop waits for owned process cleanup; retry requires a new download approval. Refresh restores the latest environment task without replay. Raw installer URLs and output are not exposed, and system libraries are not installed with elevated privileges. Browser/model compatibility and clean-machine acceptance remain separate.

## Data and uninstall

By default, project data stays in `~/.qinAegis/projects`; configuration uses the OS configuration directory. Set `QINAEGIS_DATA_DIR` to an absolute directory before starting to isolate projects, contracts, model registry, service state, default runtimes and `config.toml` under one root. Empty/relative values are rejected. Stop the old service before changing the variable and use the same setting for subsequent commands; existing assets are not automatically migrated. Explicit runtime/state overrides still take precedence. Stop the service first, then remove only the installed `qinaegis` executable to uninstall the base package. Keep project/config directories for future use, or back them up before deliberately deleting data. Do not assume replacing a binary reverses any future data migration.

See the [user guide](docs/USER_GUIDE.md), [documentation index](docs/README.md) and [CI distribution details](docs/CI_INTEGRATION.md) for current operations and acceptance limits.

Configuration migration is explicit in Settings: preview one known source, review the resulting endpoints and choose both credential fields before applying. It backs up the previous target and leaves source files intact; it does not migrate projects or model profiles. For interrupted Allure generation, Reports offers inspected temporary-file cleanup; unknown residue and completed reports are preserved. See the [user guide](docs/USER_GUIDE.md).
