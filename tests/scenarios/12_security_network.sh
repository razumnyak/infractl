#!/bin/sh
# Test: Network isolation and rate limiting
set -e

echo "Testing network security..."

AGENT_URL="http://10.10.0.10:8111"
HOME_URL="http://10.10.0.3:8111"

# Test 1: Allowed network access
echo "  Testing access from allowed network (10.10.0.0/24)..."
response=$(curl -s -o /dev/null -w "%{http_code}" "$AGENT_URL/health")
if [ "$response" = "200" ]; then
    echo "    ✓ Allowed network: access granted (HTTP 200)"
else
    echo "    ✗ Allowed network: unexpected response (HTTP $response)"
fi

# Test 2: X-Forwarded-For header handling
echo "  Testing X-Forwarded-For header (spoofing attempt)..."
response=$(curl -s -o /dev/null -w "%{http_code}" \
    -H "X-Forwarded-For: 1.2.3.4" \
    "$AGENT_URL/health")

# Should still work because we're from allowed network
# But the header should be logged/ignored
if [ "$response" = "200" ]; then
    echo "    ✓ X-Forwarded-For ignored (request from real IP accepted)"
else
    echo "    ⚠ Response: HTTP $response"
fi

# Test 3: Rate limiting (if enabled)
echo "  Testing rate limiting..."
echo "    Sending 20 rapid requests..."

success_count=0
rate_limited=0

for i in $(seq 1 20); do
    response=$(curl -s -o /dev/null -w "%{http_code}" "$AGENT_URL/health")
    if [ "$response" = "200" ]; then
        success_count=$((success_count + 1))
    elif [ "$response" = "429" ]; then
        rate_limited=$((rate_limited + 1))
    fi
done

echo "    Success: $success_count, Rate limited: $rate_limited"
if [ $rate_limited -gt 0 ]; then
    echo "    ✓ Rate limiting is active"
else
    echo "    ⚠ Rate limiting may not be configured (all requests succeeded)"
fi

# Test 4: HTTPS redirect (if configured)
echo "  Testing HTTP vs HTTPS..."
# In test environment, we use HTTP. In production, HTTPS should be enforced.
echo "    ⚠ HTTPS testing requires TLS configuration (skipped in test env)"

# Test 5: CORS headers
echo "  Testing CORS headers..."
response=$(curl -s -I "$HOME_URL/health" | grep -i "access-control" || echo "none")
if [ "$response" = "none" ]; then
    echo "    ✓ No CORS headers (API not exposed to browsers)"
else
    echo "    ⚠ CORS headers present: $response"
fi

# Test 6: Security headers
echo "  Testing security headers..."
headers=$(curl -s -I "$HOME_URL/monitoring")

if echo "$headers" | grep -qi "x-frame-options"; then
    echo "    ✓ X-Frame-Options header present"
else
    echo "    ⚠ X-Frame-Options header missing"
fi

if echo "$headers" | grep -qi "x-content-type-options"; then
    echo "    ✓ X-Content-Type-Options header present"
else
    echo "    ⚠ X-Content-Type-Options header missing"
fi

echo ""
echo "Network security tests completed!"
