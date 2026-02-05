#!/bin/sh
# Test: Health endpoints
set -e

HOME_URL="http://10.10.0.3:8111"
AGENT_URL="http://10.10.0.10:8111"

echo "Testing health endpoints..."

# Test Home health
echo "  Checking Home /health..."
response=$(curl -sf "$HOME_URL/health")
echo "    Response: $response"

if echo "$response" | grep -q '"status"'; then
    echo "    ✓ Home health OK"
else
    echo "    ✗ Home health FAILED"
    exit 1
fi

# Test Agent health
echo "  Checking Agent /health..."
response=$(curl -sf "$AGENT_URL/health")
echo "    Response: $response"

if echo "$response" | grep -q '"status"'; then
    echo "    ✓ Agent health OK"
else
    echo "    ✗ Agent health FAILED"
    exit 1
fi

# Test Home monitoring page
echo "  Checking Home /monitoring..."
response=$(curl -sf -o /dev/null -w "%{http_code}" "$HOME_URL/monitoring")
if [ "$response" = "200" ]; then
    echo "    ✓ Monitoring page OK (HTTP $response)"
else
    echo "    ✗ Monitoring page FAILED (HTTP $response)"
    exit 1
fi

echo ""
echo "Health check tests passed!"
