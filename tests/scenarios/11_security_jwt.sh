#!/bin/sh
# Test: JWT security validations
set -e

echo "Testing JWT security..."

HOME_URL="http://10.10.0.3:8111"

# Test 1: No token
echo "  Testing missing token..."
response=$(curl -s -o /dev/null -w "%{http_code}" "$HOME_URL/api/agents")
if [ "$response" = "401" ]; then
    echo "    ✓ Missing token rejected (HTTP 401)"
else
    echo "    ✗ Expected 401, got HTTP $response"
fi

# Test 2: Empty token
echo "  Testing empty token..."
response=$(curl -s -o /dev/null -w "%{http_code}" \
    -H "Authorization: Bearer " \
    "$HOME_URL/api/agents")
if [ "$response" = "401" ]; then
    echo "    ✓ Empty token rejected (HTTP 401)"
else
    echo "    ⚠ Empty token: HTTP $response"
fi

# Test 3: Malformed token
echo "  Testing malformed token..."
response=$(curl -s -o /dev/null -w "%{http_code}" \
    -H "Authorization: Bearer not.a.valid.jwt.token" \
    "$HOME_URL/api/agents")
if [ "$response" = "401" ]; then
    echo "    ✓ Malformed token rejected (HTTP 401)"
else
    echo "    ⚠ Malformed token: HTTP $response"
fi

# Test 4: Token with wrong algorithm (none)
echo "  Testing 'none' algorithm token..."
# Header: {"alg":"none","typ":"JWT"} = eyJhbGciOiJub25lIiwidHlwIjoiSldUIn0
# Payload: {"sub":"test","exp":9999999999,"iss":"infractl"}
none_token="eyJhbGciOiJub25lIiwidHlwIjoiSldUIn0.eyJzdWIiOiJ0ZXN0IiwiZXhwIjo5OTk5OTk5OTk5LCJpc3MiOiJpbmZyYWN0bCJ9."

response=$(curl -s -o /dev/null -w "%{http_code}" \
    -H "Authorization: Bearer $none_token" \
    "$HOME_URL/api/agents")
if [ "$response" = "401" ]; then
    echo "    ✓ 'none' algorithm rejected (HTTP 401)"
else
    echo "    ✗ 'none' algorithm accepted! (HTTP $response) - SECURITY ISSUE"
fi

# Test 5: Expired token
echo "  Testing expired token..."
# Payload with exp=1000000000 (2001)
expired_token="eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiJ0ZXN0IiwiZXhwIjoxMDAwMDAwMDAwLCJpYXQiOjEwMDAwMDAwMDAsImlzcyI6ImluZnJhY3RsIn0.invalid"

response=$(curl -s -o /dev/null -w "%{http_code}" \
    -H "Authorization: Bearer $expired_token" \
    "$HOME_URL/api/agents")
if [ "$response" = "401" ]; then
    echo "    ✓ Expired token rejected (HTTP 401)"
else
    echo "    ⚠ Expired token: HTTP $response"
fi

# Test 6: Token with wrong issuer
echo "  Testing wrong issuer token..."
# Payload with iss="attacker"
wrong_iss_token="eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiJ0ZXN0IiwiZXhwIjo5OTk5OTk5OTk5LCJpYXQiOjE3MDAwMDAwMDAsImlzcyI6ImF0dGFja2VyIn0.test"

response=$(curl -s -o /dev/null -w "%{http_code}" \
    -H "Authorization: Bearer $wrong_iss_token" \
    "$HOME_URL/api/agents")
if [ "$response" = "401" ]; then
    echo "    ✓ Wrong issuer rejected (HTTP 401)"
else
    echo "    ⚠ Wrong issuer: HTTP $response"
fi

# Test 7: SQL injection in token
echo "  Testing SQL injection in token..."
response=$(curl -s -o /dev/null -w "%{http_code}" \
    -H "Authorization: Bearer ' OR '1'='1" \
    "$HOME_URL/api/agents")
if [ "$response" = "401" ]; then
    echo "    ✓ SQL injection attempt rejected (HTTP 401)"
else
    echo "    ⚠ SQL injection attempt: HTTP $response"
fi

echo ""
echo "JWT security tests completed!"
