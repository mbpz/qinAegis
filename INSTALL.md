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
- Docker (for sandbox execution)

```bash
brew install --cask docker
```

## Post-Installation

After installation, run:

```bash
qinAegis init
```

This will:
1. Authenticate with Notion via OAuth2
2. Set up the configuration directory
3. Guide you through initial setup
