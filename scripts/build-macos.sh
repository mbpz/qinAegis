#!/bin/bash
set -e

VERSION=${1:-"0.7.2"}
ARCH=${2:-"arm64"}

echo "Building QinAegis v${VERSION} for ${ARCH}..."

# Go to project root
cd /Users/jinguo.zeng/dmall/project/qinAegis

# Setup
BUILD_DIR="build"
rm -rf "$BUILD_DIR"
mkdir -p "$BUILD_DIR"

# Build React UI
echo "Building React UI..."
cd crates/web_client/ui
npm install
npm run build
cd ../..
# vite's outDir is ../src/assets, which matches the embedded assets path - no copy needed

# Build Rust binary
echo "Building Rust binary..."
HOST_ARCH=$(uname -m)
TARGET_DIR="target/release"

if [ "$HOST_ARCH" = "arm64" ] && [ "$ARCH" = "arm64" ]; then
    cargo build --release --bin qinAegis-web
elif [ "$HOST_ARCH" = "x86_64" ] && [ "$ARCH" = "x64" ]; then
    cargo build --release --bin qinAegis-web
else
    if [ "$ARCH" = "x64" ]; then
        TARGET_DIR="target/x86_64-apple-darwin/release"
    else
        TARGET_DIR="target/aarch64-apple-darwin/release"
    fi
    cargo build --release --bin qinAegis-web --target "${TARGET_DIR%%/release}"
fi

# Create app bundle
echo "Creating app bundle..."
BUNDLE="$BUILD_DIR/QinAegis.app"
rm -rf "$BUNDLE"
mkdir -p "$BUNDLE/Contents/MacOS"
cp "$TARGET_DIR/qinAegis-web" "$BUNDLE/Contents/MacOS/QinAegis"

PLIST="<?xml version=\"1.0\" encoding=\"UTF-8\"?><!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\"><plist version=\"1.0\"><dict><key>CFBundleExecutable</key><string>QinAegis</string><key>CFBundleIdentifier</key><string>com.qinaegis.app</string><key>CFBundleName</key><string>QinAegis</string><key>CFBundlePackageType</key><string>APPL</string><key>CFBundleShortVersionString</key><string>${VERSION}</string><key>CFBundleVersion</key><string>1</string><key>LSMinimumSystemVersion</key><string>10.15</string><key>LSApplicationCategoryType</key><string>public.app-category.developer-tools</string></dict></plist>"
echo "$PLIST" > "$BUNDLE/Contents/Info.plist"

# Sign app (ad-hoc)
echo "Signing app..."
codesign --force --deep --sign - "$BUNDLE"

# Create tarball
echo "Creating tarball..."
TAR_NAME="qinAegis-$ARCH-apple-darwin.tar.gz"
tar -czf "$BUILD_DIR/$TAR_NAME" -C "$TARGET_DIR" qinAegis-web

# Create DMG
DMG_NAME="QinAegis-${VERSION}-mac-$ARCH"
echo "Creating DMG..."
hdiutil create -volname "$DMG_NAME" -fs APFS -format UDZO \
    "$BUILD_DIR/${DMG_NAME}.dmg" -srcfolder "$BUNDLE"

# Quarantine fix
xattr -r -d com.apple.quarantine "$BUILD_DIR/${DMG_NAME}.dmg" 2>/dev/null || true

# SHA256
echo "Computing SHA256..."
ARCH_NAME="aarch64-apple-darwin"
if [ "$ARCH" = "x64" ]; then
    ARCH_NAME="x86_64-apple-darwin"
fi
echo "$ARCH_NAME" > "$BUILD_DIR/sha256_${ARCH}.txt"
shasum -a 256 "$BUILD_DIR/${DMG_NAME}.dmg" | awk '{print $1}' >> "$BUILD_DIR/sha256_${ARCH}.txt"

echo ""
echo "=== Build Complete ==="
echo "DMG: $BUILD_DIR/${DMG_NAME}.dmg"
echo "Tarball: $BUILD_DIR/$TAR_NAME"
echo "SHA256: $BUILD_DIR/sha256_${ARCH}.txt"
echo ""
cat "$BUILD_DIR/sha256_${ARCH}.txt"