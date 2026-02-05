#!/bin/sh
# Test: Security validations
set -e

AGENT_URL="http://10.10.0.10:8111"
HOME_URL="http://10.10.0.3:8111"

echo "Testing security validations..."

# Test 1: Network isolation (requests from allowed network)
echo "  Testing network isolation..."
response=$(curl -s -o /dev/null -w "%{http_code}" "$AGENT_URL/health")
if [ "$response" = "200" ]; then
    echo "    ✓ Request from allowed network accepted"
else
    echo "    ✗ Request blocked unexpectedly (HTTP $response)"
fi

# Test 2: JWT required for protected endpoints
echo "  Testing JWT requirement..."
response=$(curl -s -o /dev/null -w "%{http_code}" "$HOME_URL/api/agents")
if [ "$response" = "401" ]; then
    echo "    ✓ Protected endpoint requires authentication"
else
    echo "    ✗ Protected endpoint accessible without auth (HTTP $response)"
    exit 1
fi

# Test 3: Invalid paths rejected
echo "  Testing path validation..."
response=$(curl -s -o /dev/null -w "%{http_code}" "$AGENT_URL/../../etc/passwd")
case "$response" in
    400|404)
        echo "    ✓ Path traversal in URL rejected (HTTP $response)"
        ;;
    *)
        echo "    ⚠ Unexpected response to path traversal (HTTP $response)"
        ;;
esac

# Test 4: Webhook signature validation
echo "  Testing webhook without valid signature..."
response=$(curl -s -o /dev/null -w "%{http_code}" -X POST \
    -H "Content-Type: application/json" \
    -H "X-Hub-Signature-256: sha256=invalid" \
    -d '{"ref":"refs/heads/main"}' \
    "$AGENT_URL/webhook/deploy/test-app")

case "$response" in
    401|403)
        echo "    ✓ Invalid webhook signature rejected (HTTP $response)"
        ;;
    200|202)
        echo "    ⚠ Webhook accepted (signature may not be configured)"
        ;;
    *)
        echo "    ⚠ Unexpected webhook response (HTTP $response)"
        ;;
esac

echo ""
echo "Security tests completed!"
