#!/bin/sh
# Test: Webhook endpoints
set -e

AGENT_URL="http://10.10.0.10:8111"

echo "Testing webhook endpoints..."

# Test webhook endpoint exists
echo "  Testing webhook endpoint availability..."
response=$(curl -s -o /dev/null -w "%{http_code}" -X POST \
    -H "Content-Type: application/json" \
    -d '{}' \
    "$AGENT_URL/webhook/deploy/test-app")

# 401/403 = auth required (expected), 200/202 = success, 404 = not found
case "$response" in
    401|403|200|202)
        echo "    ✓ Webhook endpoint exists (HTTP $response)"
        ;;
    *)
        echo "    ✗ Webhook endpoint issue (HTTP $response)"
        exit 1
        ;;
esac

# Test non-existent deployment
echo "  Testing non-existent deployment..."
response=$(curl -s -o /dev/null -w "%{http_code}" -X POST \
    -H "Content-Type: application/json" \
    -d '{}' \
    "$AGENT_URL/webhook/deploy/nonexistent-deployment")

# 401 = auth checked first (secure), 404 = not found
case "$response" in
    401)
        echo "    ✓ Auth required before checking deployment (secure)"
        ;;
    404)
        echo "    ✓ Non-existent deployment returns 404"
        ;;
    *)
        echo "    ⚠ Got HTTP $response for non-existent deployment"
        ;;
esac

# Test invalid HTTP method
echo "  Testing GET on webhook (should fail)..."
response=$(curl -s -o /dev/null -w "%{http_code}" \
    "$AGENT_URL/webhook/deploy/test-app")

# 401 = auth first, 404/405 = method not allowed
case "$response" in
    401)
        echo "    ✓ Auth required even for wrong method (secure)"
        ;;
    404|405)
        echo "    ✓ GET method rejected (HTTP $response)"
        ;;
    *)
        echo "    ⚠ Unexpected response for GET (HTTP $response)"
        ;;
esac

echo ""
echo "Webhook tests completed!"
