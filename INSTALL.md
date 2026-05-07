# Installation

## Via Homebrew (Recommended)

```bash
brew install yourorg/qinAegis/qinAegis
```

Or add the tap first:

```bash
brew tap yourorg/qinAegis
brew install qinAegis
```

## From Source

```bash
git clone https://github.com/yourorg/qinAegis.git
cd qinAegis
cargo install --path crates/cli
```

## Requirements

- macOS 12.0 or later
- Node.js 18+ (for sandbox layer)
- Playwright (automatically managed, no manual install needed)

```bash
# Playwright browsers are installed automatically on first run
# If you need to install manually:
cd sandbox && pnpm exec playwright install chromium
```

## Post-Installation

After installation, run:

```bash
qinAegis init
```

This will:
1. Guide you through AI model configuration
2. Set up the configuration directory
3. Install Playwright browsers (first run only)

## No Docker Required

qinAegis uses Playwright to manage browser instances directly on your machine. No Docker or container runtime required.