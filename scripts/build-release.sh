#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TARGET="${1:-$(rustc -vV | sed -n 's/^host: //p')}"
case "$TARGET" in
  aarch64-apple-darwin|x86_64-apple-darwin|x86_64-unknown-linux-gnu|x86_64-pc-windows-msvc) ;;
  *) echo "Unsupported release target: $TARGET" >&2; exit 1 ;;
esac
cd "$ROOT/crates/web_client/ui"
npm ci
npm run build
cd "$ROOT"
cargo build --locked --release --bin qinaegis --target "$TARGET"
mkdir -p "$ROOT/build"
if [[ "$TARGET" == *windows* ]]; then
  powershell.exe -NoProfile -Command "Compress-Archive -Force -Path 'target/$TARGET/release/qinaegis.exe','LICENSE' -DestinationPath 'build/qinaegis-$TARGET.zip'"
else
  tar -czf "build/qinaegis-$TARGET.tar.gz" -C "target/$TARGET/release" qinaegis -C "$ROOT" LICENSE
fi
