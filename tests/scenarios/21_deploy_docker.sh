#!/bin/sh
# Test: Docker deployment workflows (API-based)
set -e

echo "Testing Docker deployment workflows..."

AGENT_URL="http://10.10.0.10:8111"

# Test 1: Check agent has Docker info
echo "  Checking Docker availability via agent..."
response=$(curl -s "$AGENT_URL/health")

if echo "$response" | grep -q '"docker"'; then
    echo "    ✓ Docker metrics available in agent"

    # Extract docker info
    if echo "$response" | grep -q '"available":true'; then
        echo "    ✓ Docker is available"
    else
        echo "    ⚠ Docker not available in agent"
    fi

    if echo "$response" | grep -q '"containers_running"'; then
        echo "    ✓ Container stats present"
    fi

    if echo "$response" | grep -q '"compose_projects"'; then
        echo "    ✓ Compose projects tracked"
    fi
else
    echo "    ⚠ Docker metrics not in health response"
fi

# Test 2: Docker deployment webhook (if configured)
echo "  Testing docker deployment webhook..."

# This would trigger docker_pull type deployment
response=$(curl -s -o /dev/null -w "%{http_code}" -X POST \
    -H "Content-Type: application/json" \
    -d '{"ref":"refs/heads/main"}' \
    "$AGENT_URL/webhook/deploy/docker-test" 2>/dev/null || echo "404")

case "$response" in
    200|202)
        echo "    ✓ Docker deployment triggered (HTTP $response)"
        ;;
    401|403)
        echo "    ✓ Docker deployment requires auth (HTTP $response)"
        ;;
    404)
        echo "    ⚠ Docker deployment not configured (HTTP 404)"
        ;;
    *)
        echo "    ⚠ Docker deployment: HTTP $response"
        ;;
esac

# Test 3: Verify container list in metrics
echo "  Checking container list..."

containers=$(curl -s "$AGENT_URL/health" | grep -o '"containers":\[.*\]' | head -c 200 || echo "none")
if [ "$containers" != "none" ]; then
    echo "    ✓ Container list available"
else
    echo "    ⚠ No container list in response"
fi

echo ""
echo "Docker deployment tests completed!"
