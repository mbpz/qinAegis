#!/usr/bin/env bash
set -euo pipefail
case "${1:-$(uname -m)}" in
  arm64|aarch64) TARGET=aarch64-apple-darwin ;;
  x64|x86_64) TARGET=x86_64-apple-darwin ;;
  *) echo 'Usage: bash scripts/build-macos.sh [arm64|x64]' >&2; exit 1 ;;
esac
exec bash "$(dirname "$0")/build-release.sh" "$TARGET"
