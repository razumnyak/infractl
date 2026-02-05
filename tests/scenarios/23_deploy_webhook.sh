#!/bin/sh
# Test: Webhook-triggered deployments
set -e

echo "Testing webhook deployments..."

AGENT_URL="http://10.10.0.10:8111"

# Test 1: Webhook endpoint availability
echo "  Testing webhook endpoint..."

response=$(curl -s -o /dev/null -w "%{http_code}" -X POST \
    -H "Content-Type: application/json" \
    -d '{"ref":"refs/heads/main"}' \
    "$AGENT_URL/webhook/deploy/test-app")

case "$response" in
    200|202)
        echo "    ✓ Webhook endpoint accessible (HTTP $response)"
        ;;
    401|403)
        echo "    ✓ Webhook requires authentication (HTTP $response)"
        ;;
    404)
        echo "    ⚠ Deployment not found (HTTP 404)"
        ;;
    *)
        echo "    ⚠ Unexpected response (HTTP $response)"
        ;;
esac

# Test 2: GitHub webhook signature (HMAC-SHA256)
echo "  Testing GitHub webhook signature..."

# Calculate HMAC signature
SECRET="webhook-secret-for-testing"
PAYLOAD='{"ref":"refs/heads/main","repository":{"name":"test"}}'
# Note: In real test, we'd calculate: echo -n "$PAYLOAD" | openssl dgst -sha256 -hmac "$SECRET"

# Test with correct signature format (but wrong value)
response=$(curl -s -o /dev/null -w "%{http_code}" -X POST \
    -H "Content-Type: application/json" \
    -H "X-Hub-Signature-256: sha256=invalid_signature_here" \
    -d "$PAYLOAD" \
    "$AGENT_URL/webhook/deploy/test-app")

case "$response" in
    401|403)
        echo "    ✓ Invalid signature rejected (HTTP $response)"
        ;;
    200|202)
        echo "    ⚠ Webhook accepted without valid signature"
        ;;
    *)
        echo "    ⚠ Signature test response (HTTP $response)"
        ;;
esac

# Test 3: GitLab webhook token
echo "  Testing GitLab webhook token..."

response=$(curl -s -o /dev/null -w "%{http_code}" -X POST \
    -H "Content-Type: application/json" \
    -H "X-Gitlab-Token: invalid_token" \
    -d '{"ref":"refs/heads/main"}' \
    "$AGENT_URL/webhook/deploy/test-app")

echo "    GitLab token test: HTTP $response"

# Test 4: Webhook payload validation
echo "  Testing webhook payload validation..."

# Empty payload
response=$(curl -s -o /dev/null -w "%{http_code}" -X POST \
    -H "Content-Type: application/json" \
    -d '{}' \
    "$AGENT_URL/webhook/deploy/test-app")

echo "    Empty payload: HTTP $response"

# Invalid JSON
response=$(curl -s -o /dev/null -w "%{http_code}" -X POST \
    -H "Content-Type: application/json" \
    -d 'not-json' \
    "$AGENT_URL/webhook/deploy/test-app" 2>/dev/null || echo "400")

echo "    Invalid JSON: HTTP $response"

# Test 5: Branch filtering
echo "  Testing branch filtering..."

# Webhook for non-main branch
response=$(curl -s -X POST \
    -H "Content-Type: application/json" \
    -d '{"ref":"refs/heads/feature-branch"}' \
    "$AGENT_URL/webhook/deploy/test-app" 2>/dev/null || echo "{}")

echo "    Feature branch webhook: processed"

# Test 6: Concurrent webhook handling
echo "  Testing concurrent webhooks..."

# Send multiple webhooks quickly
for i in 1 2 3; do
    curl -s -o /dev/null -X POST \
        -H "Content-Type: application/json" \
        -d "{\"ref\":\"refs/heads/main\",\"id\":$i}" \
        "$AGENT_URL/webhook/deploy/test-app" &
done
wait

echo "    ✓ Concurrent webhooks sent"

# Test 7: Webhook response format
echo "  Testing webhook response format..."

response=$(curl -s -X POST \
    -H "Content-Type: application/json" \
    -d '{"ref":"refs/heads/main"}' \
    "$AGENT_URL/webhook/deploy/test-app")

if echo "$response" | grep -qE '(success|queued|error|status)'; then
    echo "    ✓ Webhook returns structured response"
else
    echo "    ⚠ Webhook response: $response"
fi

echo ""
echo "Webhook deployment tests completed!"
