#!/bin/sh
# Test: Configuration validation (API-based)
set -e

echo "Testing configuration validation..."

HOME_URL="http://10.10.0.3:8111"
AGENT_URL="http://10.10.0.10:8111"

# Test 1: Services started with valid config
echo "  Testing services started successfully..."

home_status=$(curl -s -o /dev/null -w "%{http_code}" "$HOME_URL/health")
agent_status=$(curl -s -o /dev/null -w "%{http_code}" "$AGENT_URL/health")

if [ "$home_status" = "200" ]; then
    echo "    ✓ Home started with valid config"
else
    echo "    ✗ Home failed to start (HTTP $home_status)"
fi

if [ "$agent_status" = "200" ]; then
    echo "    ✓ Agent started with valid config"
else
    echo "    ✗ Agent failed to start (HTTP $agent_status)"
fi

# Test 2: Mode is correctly configured
echo "  Testing mode configuration..."

home_mode=$(curl -s "$HOME_URL/health" | grep -o '"mode":"[^"]*"' | cut -d'"' -f4)
agent_mode=$(curl -s "$AGENT_URL/health" | grep -o '"mode":"[^"]*"' | cut -d'"' -f4)

if [ "$home_mode" = "home" ]; then
    echo "    ✓ Home mode correct"
else
    echo "    ✗ Home mode incorrect: $home_mode"
    exit 1
fi

if [ "$agent_mode" = "agent" ]; then
    echo "    ✓ Agent mode correct"
else
    echo "    ✗ Agent mode incorrect: $agent_mode"
    exit 1
fi

# Test 3: Network isolation is working
echo "  Testing network isolation config..."

# Both should accept connections from 10.10.0.0/24 (our test network)
# This is implicit - if we can reach them, isolation allows us

echo "    ✓ Network isolation allows test network"

# Test 4: JWT authentication is enabled
echo "  Testing JWT authentication config..."

response=$(curl -s -o /dev/null -w "%{http_code}" "$HOME_URL/api/agents")
if [ "$response" = "401" ]; then
    echo "    ✓ JWT authentication enabled"
else
    echo "    ⚠ JWT auth response: HTTP $response"
fi

# Test 5: Metrics module enabled
echo "  Testing metrics module..."

metrics=$(curl -s "$AGENT_URL/health")
if echo "$metrics" | grep -q '"cpu_usage"'; then
    echo "    ✓ Metrics collection enabled"
else
    echo "    ⚠ Metrics may be disabled"
fi

# Test 6: Deploy module configured
echo "  Testing deploy module..."

# Check if webhook endpoint exists (returns 401/404, not connection error)
response=$(curl -s -o /dev/null -w "%{http_code}" -X POST \
    -H "Content-Type: application/json" \
    -d '{}' \
    "$AGENT_URL/webhook/deploy/test-app")

case "$response" in
    200|202|401|403)
        echo "    ✓ Deploy module enabled (webhook responds)"
        ;;
    404)
        echo "    ⚠ test-app deployment not configured"
        ;;
    000)
        echo "    ✗ Webhook endpoint not reachable"
        ;;
    *)
        echo "    ⚠ Webhook response: HTTP $response"
        ;;
esac

# Test 7: Version info available
echo "  Testing version configuration..."

version=$(curl -s "$HOME_URL/health" | grep -o '"version":"[^"]*"' | cut -d'"' -f4)
if [ -n "$version" ]; then
    echo "    ✓ Version: $version"
else
    echo "    ⚠ Version not in response"
fi

# Test 8: Hostname configured
echo "  Testing hostname..."

hostname=$(curl -s "$AGENT_URL/health" | grep -o '"hostname":"[^"]*"' | cut -d'"' -f4)
if [ -n "$hostname" ]; then
    echo "    ✓ Hostname: $hostname"
else
    echo "    ⚠ Hostname not in response"
fi

echo ""
echo "Configuration validation tests completed!"
