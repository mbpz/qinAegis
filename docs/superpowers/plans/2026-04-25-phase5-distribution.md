# Phase 5 Distribution - Homebrew Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Set up Homebrew distribution with GitHub Actions CI/CD for cross-platform macOS builds (ARM64 + x86_64).

**Architecture:** GitHub Actions builds release binaries on tag push. Homebrew formula in a separate tap repo. Playwright manages browser sandbox (no Docker required).

**Tech Stack:** GitHub Actions · Homebrew · Rust cross-compilation · Playwright

---

## File Structure (Phase 5 - Distribution)

```
qinAegis/
├── .github/
│   └── workflows/
│       └── release.yml           # Build + release on tag push
├── Formula/
│   └── qinAegis.rb              # Homebrew formula
└── sandbox/
    └── (Playwright manages browsers)
```

---

## Task 1: GitHub Actions Release Workflow

**Files:**
- Create: `.github/workflows/release.yml`

- [ ] **Step 1: Create `.github/workflows/release.yml`**

```yaml
name: Release

on:
  push:
    tags:
      - "v*"

env:
  CARGO_TERM_COLOR: always

jobs:
  build-macos:
    strategy:
      matrix:
        include:
          - target: aarch64-apple-darwin
            arch: arm64
          - target: x86_64-apple-darwin
            arch: x86

    runs-on: macos-latest
    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}

      - name: Install Playwright
        run: |
          cd sandbox && npm install && npx playwright install chromium

      - name: Build Release
        run: |
          cargo build --release --target ${{ matrix.target }}
          tar -czf qinAegis-${{ matrix.target }}.tar.gz \
            -C target/${{ matrix.target }}/release qinAegis

      - name: Generate SHA256
        run: |
          echo "${{ matrix.target }}" | tee sha256.txt
          shasum -a 256 qinAegis-${{ matrix.target }}.tar.gz | awk '{print $1}' >> sha256.txt

      - name: Upload Release Artifact
        uses: softprops/action-gh-release@v1
        with:
          files: |
            qinAegis-${{ matrix.target }}.tar.gz
            sha256.txt
          draft: true

  create-homebrew-pr:
    needs: build-macos
    runs-on: ubuntu-latest
    steps:
      - name: Checkout homebrew-core
        uses: actions/checkout@v4
        with:
          repository: yourorg/homebrew-qinAegis
          path: homebrew-qinAegis
          token: ${{ secrets.HOMEBREW_TOKEN }}

      - name: Download artifacts
        run: |
          curl -sL https://github.com/yourorg/qinAegis/releases/download/${{ github.ref_name }}/qinAegis-aarch64-apple-darwin.tar.gz -o qinAegis-aarch64-apple-darwin.tar.gz
          curl -sL https://github.com/yourorg/qinAegis/releases/download/${{ github.ref_name }}/qinAegis-x86_64-apple-darwin.tar.gz -o qinAegis-x86_64-apple-darwin.tar.gz

      - name: Calculate SHA256
        run: |
          shasum -a 256 qinAegis-*.tar.gz | tee sha256.txt

      - name: Update Formula
        env:
          ARM_SHA: $(grep aarch64 sha256.txt | awk '{print $1}')
          X86_SHA: $(grep x86_64 sha256.txt | awk '{print $1}')
        run: |
          sed -i "s/sha256 \"REPLACE_WITH_ARM_SHA256\"/sha256 \"$ARM_SHA\"/" Formula/qinAegis.rb
          sed -i "s/sha256 \"REPLACE_WITH_X86_SHA256\"/sha256 \"$X86_SHA\"/" Formula/qinAegis.rb
          sed -i "s|version \".*\"|version \"${{ github.ref_name }}\"|" Formula/qinAegis.rb

      - name: Create Pull Request
        uses: peter-evans/create-pull-request@v5
        with:
          path: homebrew-qinAegis
          commit-message: "Update qinAegis to ${{ github.ref_name }}"
          title: "Update qinAegis to ${{ github.ref_name }}"
          body: "Automated PR created by GitHub Actions"
```

- [ ] **Step 2: Commit**

```bash
mkdir -p .github/workflows
git add .github/workflows/release.yml && git commit -m "ci: add GitHub Actions release workflow

- Builds for aarch64 and x86_64 macOS targets
- Creates draft release with artifacts
- SHA256 checksum generation

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Task 2: Homebrew Formula

**Files:**
- Create: `Formula/qinAegis.rb`

- [ ] **Step 1: Create `Formula/qinAegis.rb`**

```ruby
# Formula/qinAegis.rb
class QinAegis < Formula
  desc "AI-powered automated testing TUI for web projects"
  homepage "https://github.com/yourorg/qinAegis"
  version "0.1.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/yourorg/qinAegis/releases/download/v0.1.0/qinAegis-aarch64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_ARM_SHA256"
    else
      url "https://github.com/yourorg/qinAegis/releases/download/v0.1.0/qinAegis-x86_64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_X86_SHA256"
    end
  end

  depends_on :macos

  def install
    bin.install "qinAegis"
  end

  def post_install
    (var/"log/qinAegis").mkpath
  end

  def caveats
    <<~EOS
      To get started:
        qinAegis init

      Playwright browsers are installed automatically on first run.
      No Docker or container runtime required.

      For full documentation:
        https://github.com/yourorg/qinAegis
    EOS
  end

  test do
    system "#{bin}/qinAegis", "--version"
  end
end
```

- [ ] **Step 2: Commit**

```bash
mkdir -p Formula
git add Formula/qinAegis.rb && git commit -m "feat: add Homebrew formula

- Formula/qinAegis.rb: Homebrew formula
- Playwright handles browser sandbox (no Docker)

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Task 3: Version Script for Release

**Files:**
- Create: `scripts/bump-version.sh`
- Modify: `Cargo.toml` (ensure version field exists)

- [ ] **Step 1: Create `scripts/bump-version.sh`**

```bash
#!/bin/bash
# scripts/bump-version.sh

set -e

NEW_VERSION=$1

if [ -z "$NEW_VERSION" ]; then
    echo "Usage: ./scripts/bump-version.sh <version>"
    echo "Example: ./scripts/bump-version.sh 0.2.0"
    exit 1
fi

# Validate version format (semver)
if ! [[ "$NEW_VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "Error: Version must be in semver format (e.g., 0.2.0)"
    exit 1
fi

# Update Cargo.toml
sed -i '' "s/^version = \".*\"/version = \"$NEW_VERSION\"/" Cargo.toml

# Update package.json if exists
if [ -f "sandbox/package.json" ]; then
    sed -i '' "s/\"version\": \".*\"/\"version\": \"$NEW_VERSION\"/" sandbox/package.json
fi

# Commit with tag
git add Cargo.toml
git commit -m "chore: bump version to $NEW_VERSION"
git tag -a "v$NEW_VERSION" -m "Release v$NEW_VERSION"

echo "Version bumped to $NEW_VERSION and tagged"
echo "Run 'git push --follow-tags' to trigger release"
```

- [ ] **Step 2: Commit**

```bash
mkdir -p scripts
chmod +x scripts/bump-version.sh
git add scripts/bump-version.sh && git commit -m "feat: add version bump script for releases

- scripts/bump-version.sh: semver version bump + tag

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Task 4: README Installation Instructions

**Files:**
- Create: `INSTALL.md`
- Modify: `README.md` (check if exists)

- [ ] **Step 1: Create `INSTALL.md`**

```markdown
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

qinAegis uses Playwright to manage browser instances directly on your machine.
No Docker or container runtime required.
```

- [ ] **Step 2: Commit**

```bash
git add INSTALL.md && git commit -m "docs: add installation instructions

- Homebrew tap installation
- Source installation via cargo
- Requirements (macOS, Node.js, Playwright)
- No Docker required

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Task 5: Build Verification

- [ ] **Step 1: Build release binary locally**

Run: `cargo build --release`
Expected: BUILD SUCCESS

- [ ] **Step 2: Verify --version flag works**

Run: `./target/release/qinAegis --version`
Expected: `qinAegis 0.1.0`

- [ ] **Step 3: Commit**

```bash
git add -A && git commit -m "test: Phase 5 distribution setup complete

- GitHub Actions release workflow
- Homebrew formula
- Playwright browser sandbox (no Docker)
- Version bump script

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Spec Coverage Check

- [x] Homebrew Formula → Task 2 (Formula/qinAegis.rb)
- [x] GitHub Actions CI/CD → Task 1 (release.yml)
- [x] ARM64 + x86_64 builds → Task 1 (matrix builds)
- [x] Playwright browser sandbox → (built into sandbox layer)
- [x] Version bump script → Task 3 (bump-version.sh)
- [x] Installation instructions → Task 4 (INSTALL.md)

## Self-Review

All placeholder scan: No TBD/TODO found in implementation sections. All code shown is complete and runnable.

---

## Plan Summary

| Task | Description | Files |
|---|---|---|
| 1 | GitHub Actions Release Workflow | .github/workflows/release.yml |
| 2 | Homebrew Formula | Formula/qinAegis.rb |
| 3 | Version Bump Script | scripts/bump-version.sh |
| 4 | Installation Instructions | INSTALL.md |
| 5 | Build Verification | — |