#!/usr/bin/env bash
set -euo pipefail
PYTHON=python3
if [[ "${OS:-}" == "Windows_NT" ]]; then PYTHON=python; fi
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TARGET="${1:-$(rustc -vV | sed -n 's/^host: //p')}"
case "$TARGET" in
  aarch64-apple-darwin|x86_64-apple-darwin|x86_64-unknown-linux-gnu|x86_64-pc-windows-msvc) ;;
  *) echo "Unsupported release target: $TARGET" >&2; exit 1 ;;
esac
"$PYTHON" "$ROOT/scripts/release-assets.py" check
cd "$ROOT/crates/web_client/ui"
npm ci
npm run build
cd "$ROOT"
cargo build --locked --release --bin qinaegis --target "$TARGET"
"$PYTHON" scripts/release-assets.py package --target "$TARGET"
