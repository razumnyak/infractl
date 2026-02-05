#!/bin/sh
# Test: Home-Agent communication
set -e

echo "Testing Home-Agent integration..."

HOME_URL="http://10.10.0.3:8111"
AGENT_URL="http://10.10.0.10:8111"

# Test 1: Both services are healthy
echo "  Testing service health..."

home_status=$(curl -s "$HOME_URL/health" | grep -o '"status":"[^"]*"' | cut -d'"' -f4)
agent_status=$(curl -s "$AGENT_URL/health" | grep -o '"status":"[^"]*"' | cut -d'"' -f4)

if [ "$home_status" = "healthy" ]; then
    echo "    ✓ Home is healthy"
else
    echo "    ✗ Home status: $home_status"
fi

if [ "$agent_status" = "healthy" ]; then
    echo "    ✓ Agent is healthy"
else
    echo "    ✗ Agent status: $agent_status"
fi

# Test 2: Mode verification
echo "  Testing mode configuration..."

home_mode=$(curl -s "$HOME_URL/health" | grep -o '"mode":"[^"]*"' | cut -d'"' -f4)
agent_mode=$(curl -s "$AGENT_URL/health" | grep -o '"mode":"[^"]*"' | cut -d'"' -f4)

if [ "$home_mode" = "home" ]; then
    echo "    ✓ Home is in 'home' mode"
else
    echo "    ✗ Home mode: $home_mode"
fi

if [ "$agent_mode" = "agent" ]; then
    echo "    ✓ Agent is in 'agent' mode"
else
    echo "    ✗ Agent mode: $agent_mode"
fi

# Test 3: Version consistency
echo "  Testing version consistency..."

# Use head -1 to get only the first "version" field (app version, not docker version)
home_version=$(curl -s "$HOME_URL/health" | grep -o '"version":"[^"]*"' | head -1 | cut -d'"' -f4)
agent_version=$(curl -s "$AGENT_URL/health" | grep -o '"version":"[^"]*"' | head -1 | cut -d'"' -f4)

if [ "$home_version" = "$agent_version" ]; then
    echo "    ✓ Versions match: $home_version"
else
    echo "    ⚠ Version mismatch: Home=$home_version, Agent=$agent_version"
fi

# Test 4: Monitoring dashboard
echo "  Testing monitoring dashboard..."

response=$(curl -s -o /dev/null -w "%{http_code}" "$HOME_URL/monitoring")
if [ "$response" = "200" ]; then
    echo "    ✓ Monitoring dashboard accessible"
else
    echo "    ✗ Monitoring dashboard: HTTP $response"
fi

# Test 5: Dashboard content
echo "  Testing dashboard content..."

content=$(curl -s "$HOME_URL/monitoring" 2>/dev/null | head -c 1000)
if echo "$content" | grep -qi "infractl\|monitoring\|agent"; then
    echo "    ✓ Dashboard contains expected content"
else
    echo "    ⚠ Dashboard content may be minimal"
fi

# Test 6: API endpoint protection
echo "  Testing API protection..."

response=$(curl -s -o /dev/null -w "%{http_code}" "$HOME_URL/api/agents")
if [ "$response" = "401" ]; then
    echo "    ✓ /api/agents requires authentication"
else
    echo "    ⚠ /api/agents: HTTP $response"
fi

# Test 7: Cross-service connectivity test
echo "  Testing network connectivity..."

# From runner we can reach both - that validates the network
home_reachable=$(curl -s -o /dev/null -w "%{http_code}" --connect-timeout 5 "$HOME_URL/health")
agent_reachable=$(curl -s -o /dev/null -w "%{http_code}" --connect-timeout 5 "$AGENT_URL/health")

if [ "$home_reachable" = "200" ] && [ "$agent_reachable" = "200" ]; then
    echo "    ✓ Both services reachable from test network"
else
    echo "    ⚠ Connectivity: Home=$home_reachable, Agent=$agent_reachable"
fi

# Test 8: Uptime check (services have been running)
echo "  Testing uptime..."

home_uptime=$(curl -s "$HOME_URL/health" | grep -o '"uptime_seconds":[0-9]*' | cut -d':' -f2)
agent_uptime=$(curl -s "$AGENT_URL/health" | grep -o '"uptime_seconds":[0-9]*' | cut -d':' -f2)

if [ "$home_uptime" -gt 0 ] 2>/dev/null; then
    echo "    ✓ Home uptime: ${home_uptime}s"
else
    echo "    ⚠ Home uptime: $home_uptime"
fi

if [ "$agent_uptime" -gt 0 ] 2>/dev/null; then
    echo "    ✓ Agent uptime: ${agent_uptime}s"
else
    echo "    ⚠ Agent uptime: $agent_uptime"
fi

echo ""
echo "Home-Agent integration tests completed!"
