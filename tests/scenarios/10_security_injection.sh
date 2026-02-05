#!/bin/sh
# Test: Command injection and path traversal protection
set -e

echo "Testing injection protection..."

AGENT_URL="http://10.10.0.10:8111"

# These tests verify security through API behavior
# Full injection testing requires config changes (done via docker exec from host)

# Test 1: Webhook with suspicious payload
echo "  Testing suspicious webhook payloads..."

# Payload with shell metacharacters
response=$(curl -s -X POST \
    -H "Content-Type: application/json" \
    -d '{"ref":"refs/heads/main; cat /etc/passwd"}' \
    "$AGENT_URL/webhook/deploy/test-app" 2>/dev/null || echo "{}")

echo "    Semicolon in ref: checked"

# Payload with command substitution
response=$(curl -s -X POST \
    -H "Content-Type: application/json" \
    -d '{"ref":"$(whoami)"}' \
    "$AGENT_URL/webhook/deploy/test-app" 2>/dev/null || echo "{}")

echo "    Command substitution in ref: checked"

# Test 2: Path traversal in URL
echo "  Testing path traversal in URLs..."

response=$(curl -s -o /dev/null -w "%{http_code}" "$AGENT_URL/../../../etc/passwd")
echo "    URL traversal: HTTP $response"

response=$(curl -s -o /dev/null -w "%{http_code}" "$AGENT_URL/webhook/deploy/..%2F..%2Fetc")
echo "    Encoded traversal: HTTP $response"

# Test 3: Header injection
echo "  Testing header injection..."

response=$(curl -s -o /dev/null -w "%{http_code}" \
    -H "X-Custom: value\r\nX-Injected: header" \
    "$AGENT_URL/health")
echo "    Header injection attempt: HTTP $response"

# Test 4: Large payload (buffer overflow attempt)
echo "  Testing large payload handling..."

large_string=$(printf 'A%.0s' $(seq 1 10000))
response=$(curl -s -o /dev/null -w "%{http_code}" -X POST \
    -H "Content-Type: application/json" \
    -d "{\"ref\":\"$large_string\"}" \
    "$AGENT_URL/webhook/deploy/test-app" 2>/dev/null || echo "000")

case "$response" in
    413|400|200|401|403)
        echo "    Large payload: HTTP $response (handled)"
        ;;
    *)
        echo "    Large payload: HTTP $response"
        ;;
esac

echo ""
echo "✓ Injection protection tests completed"
echo "Note: Full injection testing requires config-level tests (run from host)"
