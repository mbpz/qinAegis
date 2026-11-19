#!/bin/sh
set -eu
VERSION=2.0.0
PREFIX="${HOME:?HOME must be set}/.local/bin"
ARCHIVE=
EXPECTED=
START=1
fail() { echo "qinaegis: $*" >&2; exit 1; }
while [ "$#" -gt 0 ]; do
  case "$1" in
    --version|--prefix|--archive|--checksum)
      [ "$#" -ge 2 ] || fail "Missing argument for $1"
      case "$1" in
        --version) VERSION=${2#v} ;;
        --prefix) PREFIX=$2 ;;
        --archive) ARCHIVE=$2 ;;
        --checksum) EXPECTED=$2 ;;
      esac
      shift 2 ;;
    --no-start) START=0; shift ;;
    --help) echo 'Usage: sh install.sh [--version 2.0.0] [--prefix BIN_DIR] [--no-start] [--archive FILE --checksum SHA256]'; exit 0 ;;
    *) fail "Unknown option: $1" ;;
  esac
done
case "$VERSION" in ''|*[!0-9A-Za-z.-]*|[!0-9]*) fail 'Invalid version' ;; esac
case "$PREFIX" in /*) ;; *) fail '--prefix must be an absolute directory path' ;; esac
case "$(uname -s)/$(uname -m)" in
  Darwin/arm64) TARGET=aarch64-apple-darwin ;;
  Darwin/x86_64) TARGET=x86_64-apple-darwin ;;
  Linux/x86_64) TARGET=x86_64-unknown-linux-gnu ;;
  *) fail 'Unsupported platform; use the matching manual release archive' ;;
esac
WORK=$(mktemp -d)
STAGED=
cleanup() { rm -rf "$WORK"; [ -z "$STAGED" ] || rm -f "$STAGED"; }
trap cleanup EXIT HUP INT TERM
NAME="qinaegis-$TARGET.tar.gz"
if [ -z "$ARCHIVE" ]; then
  BASE="https://github.com/mbpz/qinAegis/releases/download/v$VERSION"
  curl --proto '=https' --tlsv1.2 -fSL --retry 3 "$BASE/$NAME" -o "$WORK/$NAME"
  curl --proto '=https' --tlsv1.2 -fSL --retry 3 "$BASE/SHA256SUMS" -o "$WORK/SHA256SUMS"
  EXPECTED=$(awk -v name="$NAME" '$2 == name {print $1}' "$WORK/SHA256SUMS")
  ARCHIVE="$WORK/$NAME"
else
  [ -n "$EXPECTED" ] || fail '--archive requires --checksum'
fi
[ "${#EXPECTED}" -eq 64 ] || fail 'Invalid or missing SHA-256 checksum'
case "$EXPECTED" in *[!0-9a-fA-F]*) fail 'Invalid SHA-256 checksum' ;; esac
[ -f "$ARCHIVE" ] || fail 'Archive not found'
if command -v sha256sum >/dev/null 2>&1; then
  ACTUAL=$(sha256sum "$ARCHIVE" | awk '{print $1}')
else
  ACTUAL=$(shasum -a 256 "$ARCHIVE" | awk '{print $1}')
fi
[ "$(printf '%s' "$EXPECTED" | tr A-F a-f)" = "$ACTUAL" ] || fail 'Checksum mismatch; nothing installed'
# Extract only the known regular file to stdout; archive paths cannot write to disk.
[ "$(tar -tzf "$ARCHIVE" | awk '$0 == "qinaegis" {n++} END {print n+0}')" -eq 1 ] || fail 'Archive must contain exactly one qinaegis entry'
TYPE=$(tar -tvzf "$ARCHIVE" qinaegis | cut -c1)
[ "$TYPE" = '-' ] || fail 'Executable entry is not a regular file'
mkdir -p "$PREFIX"
STAGED=$(mktemp "$PREFIX/.qinaegis-install.XXXXXX")
tar -xOzf "$ARCHIVE" qinaegis > "$STAGED"
[ -s "$STAGED" ] || fail 'Empty executable'
chmod 755 "$STAGED"
# Keep running installations intact until the user has stopped them.
if [ -x "$PREFIX/qinaegis" ] && "$PREFIX/qinaegis" status >/dev/null 2>&1; then
  fail "Existing service is running; stop it with $PREFIX/qinaegis stop first"
fi
mv -f "$STAGED" "$PREFIX/qinaegis"
STAGED=
printf 'Installed %s\nStart: %s\nStop: %s stop\n' "$PREFIX/qinaegis" "$PREFIX/qinaegis" "$PREFIX/qinaegis"
if [ "$START" -eq 1 ]; then "$PREFIX/qinaegis" start; fi
