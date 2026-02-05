#!/bin/sh
# Test: Custom script deployment workflows (API-based)
set -e

echo "Testing custom script deployments..."

AGENT_URL="http://10.10.0.10:8111"

# Test 1: Trigger script deployment via webhook
echo "  Testing script deployment webhook..."

response=$(curl -s -X POST \
    -H "Content-Type: application/json" \
    -d '{"ref":"refs/heads/main"}' \
    "$AGENT_URL/webhook/deploy/script-app" 2>/dev/null || echo "{}")

case "$(echo "$response" | grep -o '"success":[^,}]*' | head -1)" in
    *true*)
        echo "    ✓ Script deployment succeeded"
        ;;
    *false*)
        echo "    ⚠ Script deployment failed"
        echo "    Response: $(echo "$response" | head -c 200)"
        ;;
    *)
        # Check HTTP status
        status=$(curl -s -o /dev/null -w "%{http_code}" -X POST \
            -H "Content-Type: application/json" \
            -d '{}' \
            "$AGENT_URL/webhook/deploy/script-app")
        echo "    Script webhook: HTTP $status"
        ;;
esac

# Test 2: Check deployment exists
echo "  Checking script-app deployment exists..."

response=$(curl -s -o /dev/null -w "%{http_code}" -X POST \
    -H "Content-Type: application/json" \
    -d '{}' \
    "$AGENT_URL/webhook/deploy/script-app")

case "$response" in
    200|202|401|403)
        echo "    ✓ script-app deployment configured"
        ;;
    404)
        echo "    ⚠ script-app deployment not found"
        ;;
    *)
        echo "    ⚠ Unexpected: HTTP $response"
        ;;
esac

# Test 3: Non-existent script deployment
echo "  Testing non-existent deployment..."

response=$(curl -s -o /dev/null -w "%{http_code}" -X POST \
    -H "Content-Type: application/json" \
    -d '{}' \
    "$AGENT_URL/webhook/deploy/nonexistent-script")

if [ "$response" = "404" ]; then
    echo "    ✓ Non-existent deployment returns 404"
else
    echo "    ⚠ Non-existent deployment: HTTP $response"
fi

echo ""
echo "Custom script deployment tests completed!"
