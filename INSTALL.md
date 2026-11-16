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

Download the matching `qinaegis-x86_64-pc-windows-msvc.zip`, compare its hash with `SHA256SUMS` using `Get-FileHash`, extract to a user-owned directory, and run `qinaegis.exe`. A one-command PowerShell installer is not provided in this phase. Do not disable system security mechanisms to work around unsigned artifacts; platform signing and clean-machine acceptance remain release gates.

## Playwright + Midscene.js runtime

The base package does not include the engine or automatically install it yet. Browsing assets and configuring the workbench require no Node runtime; executing engine-backed operations does.

An installed runtime directory must contain:

```json
{"schema_version":1,"engine":"midscene","version":"your-verified-engine-version","entry":"dist/executor.js","node":"bin/node"}
```

`entry` and optional `node` reference files inside that directory. Supply compiled executor resources, matching production dependencies and a compatible automation browser; a manifest alone is insufficient. Set `QINAEGIS_RUNTIME_DIR` to its absolute path **before starting the service**. Environment changes require restarting the service.

For development only, prepare the repository's root `sandbox` dependencies/browser, then set `QINAEGIS_DEV_SANDBOX` to that absolute source directory. The development adapter uses `src/executor.ts` and `node_modules/tsx/dist/cli.mjs`. No implicit source-path fallback is used. Automated runtime downloads/repair and Web engine management are planned, not part of this installer.

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

## Data and uninstall

Project data stays in `~/.qinAegis/projects`; configuration uses the OS configuration directory. Stop the service first, then remove only the installed `qinaegis` executable to uninstall the base package. Keep project/config directories for future use, or back them up before deliberately deleting data. Do not assume replacing a binary reverses any future data migration.

See the [user guide](docs/USER_GUIDE.md), [documentation index](docs/README.md) and [CI distribution details](docs/CI_INTEGRATION.md) for current operations and acceptance limits.
