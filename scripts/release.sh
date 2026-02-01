#!/bin/bash
set -euo pipefail

VERSION="${1:?Usage: ./scripts/release.sh 0.1.4}"

echo "Releasing v$VERSION..."

# Detect OS for sed compatibility
if [[ "$OSTYPE" == "darwin"* ]]; then
    SED="sed -i ''"
else
    SED="sed -i"
fi

# Cargo.toml
$SED "s/^version = \".*\"/version = \"$VERSION\"/" Cargo.toml

# README.md
$SED "s/v[0-9][0-9]*\.[0-9][0-9]*\.[0-9][0-9]*/v$VERSION/g" README.md

# DEPLOY.md
$SED "s/v[0-9][0-9]*\.[0-9][0-9]*\.[0-9][0-9]*/v$VERSION/g" DEPLOY.md

# dashboard.html
$SED "s/>v[0-9][0-9]*\.[0-9][0-9]*\.[0-9][0-9]*</>v$VERSION</g" src/assets/dashboard.html

# Коммит и тег
git add -A
git commit -m "Release v$VERSION"
git tag "v$VERSION"
git push origin main
git push origin "v$VERSION"

echo "Released v$VERSION"
