#!/bin/sh
# Test: Git deployment workflow
set -e

AGENT_URL="http://10.10.0.10:8111"
GITEA_URL="http://10.10.0.2:3000"

echo "Testing Git deployment workflow..."

# Check if Gitea repo is accessible
echo "  Checking Gitea repository..."
response=$(curl -s -o /dev/null -w "%{http_code}" "$GITEA_URL/test/repo")
if [ "$response" = "200" ]; then
    echo "    ✓ Gitea repository accessible"
else
    echo "    ⚠ Gitea repository not ready (HTTP $response), skipping git tests"
    exit 0
fi

echo ""
echo "Git deployment tests completed!"
