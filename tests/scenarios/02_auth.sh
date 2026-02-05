#!/bin/sh
# Test: JWT Authentication
set -e

HOME_URL="http://10.10.0.3:8111"

echo "Testing JWT authentication..."

# Test unauthenticated access to protected endpoint
echo "  Testing unauthenticated access..."
response=$(curl -s -o /dev/null -w "%{http_code}" "$HOME_URL/api/agents")
if [ "$response" = "401" ]; then
    echo "    ✓ Unauthenticated access blocked (HTTP 401)"
else
    echo "    ✗ Expected 401, got HTTP $response"
    exit 1
fi

# Test with invalid token
echo "  Testing invalid token..."
response=$(curl -s -o /dev/null -w "%{http_code}" \
    -H "Authorization: Bearer invalid-token" \
    "$HOME_URL/api/agents")
if [ "$response" = "401" ]; then
    echo "    ✓ Invalid token rejected (HTTP 401)"
else
    echo "    ✗ Expected 401, got HTTP $response"
    exit 1
fi

# Test with malformed authorization header
echo "  Testing malformed auth header..."
response=$(curl -s -o /dev/null -w "%{http_code}" \
    -H "Authorization: NotBearer token" \
    "$HOME_URL/api/agents")
if [ "$response" = "401" ]; then
    echo "    ✓ Malformed auth header rejected (HTTP 401)"
else
    echo "    ✗ Expected 401, got HTTP $response"
    exit 1
fi

echo ""
echo "Authentication tests passed!"
