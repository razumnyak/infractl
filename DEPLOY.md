# Deployment Guide

This guide covers deploying infractl on your servers.

## Table of Contents

- [Architecture Overview](#architecture-overview)
- [Installation Methods](#installation-methods)
- [Agent Setup](#agent-setup)
- [Home Server Setup](#home-server-setup)
- [Docker Deployment](#docker-deployment)
- [Security Configuration](#security-configuration)
- [Service Management](#service-management)
- [Troubleshooting](#troubleshooting)

---

## Architecture Overview

```
Internet ──X──┐
              │ (blocked by isolation_mode)
              ▼
         ┌─────────┐
         │  HOME   │  ← Dashboard, storage, aggregation
         │ :8111   │
         └────┬────┘
              │ Internal Network (10.0.0.0/8)
     ┌────────┼────────┐
     ▼        ▼        ▼
┌─────────┐ ┌─────────┐ ┌─────────┐
│ AGENT 1 │ │ AGENT 2 │ │ AGENT N │
│ :8111   │ │ :8111   │ │ :8111   │
└─────────┘ └─────────┘ └─────────┘
```

- **Home**: Central server with dashboard, metrics storage, agent polling
- **Agent**: Worker nodes reporting metrics, executing deployments

---

## Installation Methods

### 1. Automated Script (Recommended)

```bash
# Install latest as agent
curl -fsSL https://github.com/razumnyak/infractl/releases/latest/download/install.sh | sudo bash

# Install specific version as home
curl -fsSL https://github.com/razumnyak/infractl/releases/latest/download/install.sh | sudo bash -s -- --version v0.1.3 --mode home
```

### 2. Manual Installation

```bash
# Download
ARCH=$(uname -m | sed 's/x86_64/x86_64/;s/aarch64/aarch64/;s/arm64/aarch64/')
wget "https://github.com/razumnyak/infractl/releases/latest/download/infractl-${ARCH}-unknown-linux-musl" -O /usr/local/bin/infractl
chmod +x /usr/local/bin/infractl

# Create directories
mkdir -p /etc/infractl /var/lib/infractl /var/log/infractl

# Create config (see CONFIG.md for all options)
cat > /etc/infractl/config.yaml << 'EOF'
mode: agent
auth:
  jwt_secret: "CHANGE_ME_TO_SECURE_SECRET_MIN_32_CHARS"
EOF

# Install systemd service
cp infractl.service /etc/systemd/system/
systemctl daemon-reload
systemctl enable infractl
```

### 3. From Source

```bash
# Requires Rust toolchain
cargo build --release
cp target/release/infractl /usr/local/bin/
```

---

## Agent Setup

Agents collect metrics and execute deployments on worker servers.

### Minimal Config

```yaml
mode: agent

auth:
  jwt_secret: "${JWT_SECRET}"  # Same as Home server!

server:
  port: 8111
  isolation_mode: true
  allowed_networks:
    - "10.0.0.0/8"      # Your internal network
    - "127.0.0.1/32"

modules:
  metrics:
    enabled: true
    docker_stats: true

  deploy:
    enabled: true
    deployments:
      - name: "myapp"
        type: git_pull
        path: "/opt/apps/myapp"
        branch: "main"
        post_deploy:
          - "docker compose up -d --build"
```

### With Docker Monitoring

```yaml
modules:
  metrics:
    enabled: true
    collect_interval: "30s"
    docker_stats: true
    docker_socket: "/var/run/docker.sock"  # Optional, auto-detected
    compose_projects: true
```

Ensure infractl can access Docker:

```bash
# Option 1: Run as root (default)
# Option 2: Add to docker group
usermod -aG docker infractl
```

### With Git Deployments (SSH)

```yaml
modules:
  deploy:
    deployments:
      - name: "private-repo"
        type: git_pull
        path: "/opt/apps/private-repo"
        remote: "origin"
        branch: "main"
        ssh_key: "/root/.ssh/deploy_key"
        post_deploy:
          - "docker compose up -d"
```

Generate deploy key:

```bash
ssh-keygen -t ed25519 -f /root/.ssh/deploy_key -N ""
# Add public key to repo deploy keys
```

---

## Home Server Setup

Home server aggregates metrics and provides the dashboard.

### Full Config

```yaml
mode: home

auth:
  jwt_secret: "${JWT_SECRET}"
  webhook_secrets:
    github: "${GITHUB_WEBHOOK_SECRET}"

server:
  port: 8111
  isolation_mode: true
  allowed_networks:
    - "10.0.0.0/8"
    - "127.0.0.1/32"

# List all your agents
agents:
  - name: "web-1"
    address: "http://10.0.0.10:8111"
    timeout: "10s"
    health_interval: "30s"
  - name: "web-2"
    address: "http://10.0.0.11:8111"
  - name: "db-1"
    address: "http://10.0.0.20:8111"

modules:
  metrics:
    enabled: true
    collect_interval: "30s"

  storage:
    enabled: true
    db_path: "/var/lib/infractl/metrics.db"
    retention:
      raw_data: "7d"
      hourly_data: "30d"
      daily_data: "365d"

  deploy:
    enabled: false  # Home typically doesn't deploy locally

  webhooks:
    enabled: true

logging:
  level: "info"
  format: "json"
  file: "/var/log/infractl/infractl.log"
```

### Accessing Dashboard

```
http://<home-ip>:8111/monitoring
```

To expose externally (with reverse proxy):

```nginx
# /etc/nginx/sites-available/infractl
server {
    listen 443 ssl;
    server_name monitoring.example.com;

    ssl_certificate /etc/ssl/certs/example.com.pem;
    ssl_certificate_key /etc/ssl/private/example.com.key;

    location / {
        proxy_pass http://127.0.0.1:8111;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;

        # Basic auth or other auth mechanism recommended
        auth_basic "Monitoring";
        auth_basic_user_file /etc/nginx/.htpasswd;
    }
}
```

---

## Docker Deployment

### Single Container

```bash
docker run -d \
  --name infractl \
  --restart unless-stopped \
  -p 8111:8111 \
  -v /var/run/docker.sock:/var/run/docker.sock:ro \
  -v /etc/infractl:/etc/infractl:ro \
  -v /var/lib/infractl:/var/lib/infractl \
  -v /var/log/infractl:/var/log/infractl \
  -e JWT_SECRET=your-secret-here \
  ghcr.io/razumnyak/infractl:latest
```

### Docker Compose

```yaml
# docker-compose.yml
version: '3.8'

services:
  infractl:
    image: ghcr.io/razumnyak/infractl:latest
    container_name: infractl
    restart: unless-stopped
    ports:
      - "8111:8111"
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock:ro
      - ./config.yaml:/etc/infractl/config.yaml:ro
      - infractl-data:/var/lib/infractl
      - infractl-logs:/var/log/infractl
    environment:
      - JWT_SECRET=${JWT_SECRET}
      - RUST_LOG=info

volumes:
  infractl-data:
  infractl-logs:
```

---

## Security Configuration

### 1. JWT Secret

Generate a strong secret:

```bash
openssl rand -base64 32
```

**Important**: Use the SAME secret on Home and all Agents!

### 2. Network Isolation

Only allow internal network access:

```yaml
server:
  isolation_mode: true
  allowed_networks:
    - "10.0.0.0/8"       # Your internal network
    - "172.16.0.0/12"    # Docker networks (if needed)
    - "127.0.0.1/32"     # Localhost
```

### 3. Firewall Rules

```bash
# Allow only from internal network
iptables -A INPUT -p tcp --dport 8111 -s 10.0.0.0/8 -j ACCEPT
iptables -A INPUT -p tcp --dport 8111 -j DROP
```

### 4. Webhook Secrets

For GitHub webhooks:

```yaml
auth:
  webhook_secrets:
    github: "your-github-webhook-secret"

modules:
  webhooks:
    endpoints:
      - path: "/webhook/github/myapp"
        deployment: "myapp"
        secret: "${GITHUB_WEBHOOK_SECRET}"
        allowed_ips:
          - "140.82.112.0/20"  # GitHub IPs
          - "143.55.64.0/20"
```

---

## Service Management

### Systemd (Ubuntu/Debian/RHEL)

```bash
# Start
systemctl start infractl

# Stop
systemctl stop infractl

# Restart
systemctl restart infractl

# Status
systemctl status infractl

# Logs
journalctl -u infractl -f
```

### OpenRC (Alpine)

```bash
# Start
rc-service infractl start

# Stop
rc-service infractl stop

# Status
rc-service infractl status
```

### Log Files

- Main log: `/var/log/infractl/infractl.log`
- Suspicious requests: `/var/log/infractl/suspicious.log`

---

## Troubleshooting

### Connection Refused

```bash
# Check if running
systemctl status infractl

# Check port binding
ss -tlnp | grep 8111

# Check firewall
iptables -L -n | grep 8111
```

### Permission Denied (Docker)

```bash
# Verify socket permissions
ls -la /var/run/docker.sock

# Add infractl to docker group (if not running as root)
usermod -aG docker root
systemctl restart infractl
```

### Config Validation

```bash
# Test config syntax
infractl --config /etc/infractl/config.yaml --validate

# Check for missing env vars
grep '\${' /etc/infractl/config.yaml
```

### Agent Not Responding

```bash
# From Home server, test connectivity
curl -v http://10.0.0.10:8111/health

# Check agent logs
ssh agent-server journalctl -u infractl -n 50
```

### High Memory Usage

Reduce metrics collection interval:

```yaml
modules:
  metrics:
    collect_interval: "60s"  # Increase from 30s
```

For Home, adjust retention:

```yaml
modules:
  storage:
    retention:
      raw_data: "3d"    # Reduce from 7d
      hourly_data: "14d" # Reduce from 30d
```

---

## Uninstallation

```bash
# Keep config and data
sudo /usr/local/bin/uninstall.sh

# Remove everything
sudo /usr/local/bin/uninstall.sh --purge
```

Manual:

```bash
systemctl stop infractl
systemctl disable infractl
rm /etc/systemd/system/infractl.service
rm /usr/local/bin/infractl
rm -rf /etc/infractl /var/lib/infractl /var/log/infractl
```
