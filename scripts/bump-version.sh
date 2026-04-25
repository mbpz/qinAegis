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