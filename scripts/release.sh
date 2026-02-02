#!/bin/bash
set -euo pipefail

PROD=false
CURRENT=""
TARGET=""

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --prod)
            PROD=true
            shift
            ;;
        *)
            if [[ -z "$CURRENT" ]]; then
                CURRENT="$1"
            elif [[ -z "$TARGET" ]]; then
                TARGET="$1"
            fi
            shift
            ;;
    esac
done

if [[ -z "$CURRENT" || -z "$TARGET" ]]; then
    echo "Usage: ./scripts/release.sh <current> <target> [--prod]"
    echo "Example: ./scripts/release.sh 0.1.3 0.1.4 --prod"
    echo ""
    echo "  --prod  Push to git remote (tag + main branch)"
    exit 1
fi

echo "Releasing v$CURRENT -> v$TARGET..."

# Detect OS for sed compatibility
sedi() {
    if [[ "$OSTYPE" == "darwin"* ]]; then
        sed -i '' "$@"
    else
        sed -i "$@"
    fi
}

# Cargo.toml
sedi "s/^version = \"$CURRENT\"/version = \"$TARGET\"/" Cargo.toml

# Update Cargo.lock
cargo check --quiet

# README.md
sedi "s/v$CURRENT/v$TARGET/g" README.md

# DEPLOY.md
sedi "s/v$CURRENT/v$TARGET/g" DEPLOY.md

# dashboard.html
sedi "s/>v$CURRENT</>v$TARGET</g" src/assets/dashboard.html

# install.sh (example in comment)
sedi "s/v$CURRENT/v$TARGET/g" scripts/install.sh

cargo fmt --all

# Commit and tag
git add -A
git commit -m "Release v$TARGET"
git tag "v$TARGET"

if [[ "$PROD" == "true" ]]; then
    git push origin main
    git push origin "v$TARGET"
    echo "Released v$TARGET (pushed to remote)"
else
    echo "Released v$TARGET (local only, use --prod to push)"
fi
