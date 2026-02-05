#!/bin/sh
# Initialize Gitea with test repository
# This script runs inside the Gitea container

set -e

GITEA_URL="http://localhost:3000"
ADMIN_USER="admin"
ADMIN_PASS="admin123"
ADMIN_EMAIL="admin@test.local"

# Wait for Gitea to be ready
echo "Waiting for Gitea..."
until curl -sf "$GITEA_URL/api/healthz" > /dev/null 2>&1; do
    sleep 2
done

echo "Gitea is ready"

# Create admin user if not exists
gitea admin user create \
    --username "$ADMIN_USER" \
    --password "$ADMIN_PASS" \
    --email "$ADMIN_EMAIL" \
    --admin \
    --must-change-password=false 2>/dev/null || echo "Admin user already exists"

# Create test organization
curl -sf -X POST "$GITEA_URL/api/v1/orgs" \
    -u "$ADMIN_USER:$ADMIN_PASS" \
    -H "Content-Type: application/json" \
    -d '{"username":"test","description":"Test organization"}' 2>/dev/null || echo "Org already exists"

# Create test repository
curl -sf -X POST "$GITEA_URL/api/v1/orgs/test/repos" \
    -u "$ADMIN_USER:$ADMIN_PASS" \
    -H "Content-Type: application/json" \
    -d '{"name":"repo","description":"Test repository","auto_init":true}' 2>/dev/null || echo "Repo already exists"

# Add some test files
echo "Test file content" | curl -sf -X POST "$GITEA_URL/api/v1/repos/test/repo/contents/README.md" \
    -u "$ADMIN_USER:$ADMIN_PASS" \
    -H "Content-Type: application/json" \
    -d '{"message":"Initial commit","content":"VGVzdCBmaWxlIGNvbnRlbnQK"}' 2>/dev/null || echo "File already exists"

echo "Gitea initialization complete"
