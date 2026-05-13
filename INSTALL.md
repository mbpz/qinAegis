# Installation

## Via Homebrew (Recommended)

```bash
brew install --cask mbpz/qinAegis/qinAegis
```

This will:
1. Download the latest DMG for your architecture (arm64 or x86_64)
2. Mount the DMG and copy QinAegis.app to `/Applications`
3. Remove the quarantine attribute so Gatekeeper doesn't block it

Find **QinAegis.app** in your Applications folder after installation.

## From DMG (Manual)

1. Download the DMG from GitHub Releases
2. Double-click to mount
3. Drag QinAegis.app to Applications folder
4. (First run only) Click "Open" in the Gatekeeper dialog

## From Source

```bash
git clone https://github.com/mbpz/qinAegis.git
cd qinAegis

# Build React UI
cd crates/web_client/ui && npm install && npm run build && cd ../..

# Build Rust binary
cargo build --release --bin qinAegis-web
```

The binary will be at `target/release/qinAegis-web`.

## Requirements

- macOS 10.15 or later
- Apple Silicon (M1/M2/M3) or Intel Mac

## First Launch

1. Double-click QinAegis.app in Applications
2. If you see a Gatekeeper warning, click "Open Anyway" in System Preferences > Security
3. The GUI will open with a setup wizard for AI model configuration

## Uninstall

```bash
rm -rf /Applications/QinAegis.app
# Optionally remove config (your test data will remain)
rm -rf ~/.config/qinAegis
```