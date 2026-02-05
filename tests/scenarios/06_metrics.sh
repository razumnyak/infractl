#!/bin/sh
# Test: Metrics collection
set -e

AGENT_URL="http://10.10.0.10:8111"

echo "Testing metrics collection..."

# Get health metrics from agent
echo "  Fetching agent metrics..."
response=$(curl -s "$AGENT_URL/health")

# Check for expected fields
echo "  Validating metrics structure..."

# Check CPU metrics (in system.cpu_usage)
if echo "$response" | grep -q '"cpu_usage"'; then
    echo "    ✓ CPU metrics present"
else
    echo "    ✗ CPU metrics missing"
    exit 1
fi

# Check memory metrics (in system.memory_total)
if echo "$response" | grep -q '"memory_total"'; then
    echo "    ✓ Memory metrics present"
else
    echo "    ✗ Memory metrics missing"
    exit 1
fi

# Check hostname
if echo "$response" | grep -q '"hostname"'; then
    echo "    ✓ Hostname present"
else
    echo "    ⚠ Hostname missing (optional)"
fi

# Check status
if echo "$response" | grep -q '"status":"healthy"'; then
    echo "    ✓ Status is healthy"
else
    echo "    ⚠ Status not healthy"
fi

echo ""
echo "Metrics tests passed!"
