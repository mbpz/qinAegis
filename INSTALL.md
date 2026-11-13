# Install the local Web service

2.0 is in development. Release installation below applies **after the corresponding verified release is published**. Until then, build from source using README.md. Do not install the Formula template directly.

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

## Data and uninstall

Project data stays in `~/.qinAegis/projects`; configuration uses the OS configuration directory. Stop the service first, then remove only the installed `qinaegis` executable to uninstall the base package. Keep project/config directories for future use, or back them up before deliberately deleting data. Do not assume replacing a binary reverses any future data migration.
