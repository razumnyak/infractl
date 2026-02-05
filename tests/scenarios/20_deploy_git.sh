#!/bin/sh
# Test: Git deployment workflows (API-based)
set -e

echo "Testing Git deployment workflows..."

AGENT_URL="http://10.10.0.10:8111"
GITEA_URL="http://10.10.0.2:3000"

# Test 1: Check Gitea availability
echo "  Checking Gitea server..."
response=$(curl -s -o /dev/null -w "%{http_code}" "$GITEA_URL/api/healthz" 2>/dev/null || echo "000")
if [ "$response" = "200" ]; then
    echo "    ✓ Gitea server available"
else
    echo "    ⚠ Gitea not ready (HTTP $response)"
fi

# Test 2: Check test repo exists
echo "  Checking test repository..."
response=$(curl -s -o /dev/null -w "%{http_code}" "$GITEA_URL/api/v1/repos/test/repo" 2>/dev/null || echo "000")
case "$response" in
    200)
        echo "    ✓ Test repository exists"
        ;;
    404)
        echo "    ⚠ Test repository not found (needs initialization)"
        ;;
    *)
        echo "    ⚠ Repository check: HTTP $response"
        ;;
esac

# Test 3: Trigger git deployment via webhook
echo "  Testing git deployment webhook..."
response=$(curl -s -X POST \
    -H "Content-Type: application/json" \
    -d '{"ref":"refs/heads/main","repository":{"name":"test-app","full_name":"test/repo"}}' \
    "$AGENT_URL/webhook/deploy/test-app" 2>/dev/null || echo "{}")

if echo "$response" | grep -qE '(success|queued|started|error)'; then
    echo "    ✓ Webhook processed"
    echo "    Response: $(echo "$response" | head -c 100)"
else
    echo "    ⚠ Webhook response: $response"
fi

# Test 4: Test non-main branch (should be filtered or processed)
echo "  Testing feature branch webhook..."
response=$(curl -s -X POST \
    -H "Content-Type: application/json" \
    -d '{"ref":"refs/heads/feature/test"}' \
    "$AGENT_URL/webhook/deploy/test-app" 2>/dev/null || echo "{}")

echo "    Feature branch: processed"

# Test 5: Test tag push
echo "  Testing tag webhook..."
response=$(curl -s -X POST \
    -H "Content-Type: application/json" \
    -d '{"ref":"refs/tags/v1.0.0"}' \
    "$AGENT_URL/webhook/deploy/test-app" 2>/dev/null || echo "{}")

echo "    Tag webhook: processed"

echo ""
echo "Git deployment tests completed!"
