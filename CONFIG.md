# Configuration Reference

Configuration file: `/etc/infractl/config.yaml`

Supports environment variable substitution: `${VAR_NAME}`

## Table of Contents

- [Core Settings](#core-settings)
- [Server](#server)
- [Authentication](#authentication)
- [Updates](#updates)
- [Agents](#agents) (Home mode only)
- [Modules](#modules)
  - [Metrics](#metrics)
  - [Storage](#storage) (Home mode only)
  - [Deploy](#deploy)
  - [Webhooks](#webhooks)
- [Logging](#logging)
- [Notifications](#notifications)

---

## Core Settings

```yaml
mode: agent          # Required: "home" or "agent"
version: "0.1.0"     # Optional, auto-detected from binary
```

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `mode` | string | **Yes** | - | Operation mode: `home` (central server) or `agent` (worker) |
| `version` | string | No | binary version | Config version for tracking |

---

## Server

```yaml
server:
  bind: "0.0.0.0"
  port: 8111
  isolation_mode: true
  allowed_networks:
    - "10.0.0.0/8"
    - "172.16.0.0/12"
    - "192.168.0.0/16"
    - "127.0.0.1/32"
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `bind` | string | `0.0.0.0` | Bind address |
| `port` | integer | `8111` | Listen port |
| `isolation_mode` | boolean | `true` | Enable network isolation (reject requests from non-allowed networks) |
| `allowed_networks` | list | private networks | CIDR list of allowed source networks |

---

## Authentication

```yaml
auth:
  jwt_secret: "${JWT_SECRET}"    # Required!
  token_ttl: "24h"
  webhook_secrets:
    github: "${GITHUB_WEBHOOK_SECRET}"
    gitlab: "${GITLAB_WEBHOOK_SECRET}"
```

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `jwt_secret` | string | **Yes** | - | Secret for JWT signing (min 32 chars recommended) |
| `token_ttl` | duration | No | `24h` | Token expiration time |
| `webhook_secrets` | map | No | `{}` | Named secrets for webhook signature validation |

---

## Updates

```yaml
updates:
  enabled: true
  self_update:
    enabled: true
    github_repo: "razumnyak/infractl"
    check_interval: "6h"
    prerelease: false
  config_update:
    enabled: false
    github_raw_url: "https://raw.githubusercontent.com/razumnyak/infractl/main/infractl.yaml"
    check_interval: "1h"
    backup: true
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | boolean | `false` | Enable auto-update system |

### self_update

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | boolean | `false` | Enable binary self-update |
| `github_repo` | string | - | GitHub repository (owner/repo) |
| `check_interval` | duration | `6h` | Check for updates interval |
| `prerelease` | boolean | `false` | Include pre-release versions |

### config_update

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | boolean | `false` | Enable remote config sync |
| `github_raw_url` | string | - | Raw URL to config file |
| `check_interval` | duration | `1h` | Check interval |
| `backup` | boolean | `true` | Backup config before update |

---

## Agents

**Home mode only.** List of agents to monitor.

```yaml
agents:
  - name: "server-1"
    address: "http://10.0.0.10:8111"
    timeout: "10s"
    health_interval: "30s"
  - name: "server-2"
    address: "http://10.0.0.11:8111"
```

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `name` | string | **Yes** | - | Agent display name |
| `address` | string | **Yes** | - | Agent URL (http://host:port) |
| `timeout` | duration | No | `10s` | Request timeout |
| `health_interval` | duration | No | `30s` | Health check polling interval |

---

## Modules

### Metrics

```yaml
modules:
  metrics:
    enabled: true
    collect_interval: "30s"
    docker_stats: true
    docker_socket: "/var/run/docker.sock"
    compose_projects: true
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | boolean | `true` | Enable metrics collection |
| `collect_interval` | duration | `30s` | Collection interval |
| `docker_stats` | boolean | `true` | Collect Docker container stats |
| `docker_socket` | string | auto-detect | Docker socket path |
| `compose_projects` | boolean | `true` | Track Docker Compose projects |

---

### Storage

**Home mode only.** SQLite storage for metrics history.

```yaml
modules:
  storage:
    enabled: true
    db_path: "/var/lib/infractl/metrics.db"
    retention:
      raw_data: "7d"
      hourly_data: "30d"
      daily_data: "365d"
    aggregation:
      hourly: "0 * * * *"
      daily: "0 0 * * *"
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | boolean | `true` | Enable storage module |
| `db_path` | string | `/var/lib/infractl/metrics.db` | SQLite database path |

#### retention

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `raw_data` | duration | `7d` | Keep raw metrics for |
| `hourly_data` | duration | `30d` | Keep hourly aggregates for |
| `daily_data` | duration | `365d` | Keep daily aggregates for |

#### aggregation

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `hourly` | cron | `0 * * * *` | Hourly aggregation schedule |
| `daily` | cron | `0 0 * * *` | Daily aggregation schedule |

---

### Deploy

```yaml
modules:
  deploy:
    enabled: true
    work_dir: "/opt/apps"
    default_timeout: "300s"
    deployments:
      - name: "myapp"
        type: git_pull
        path: "/opt/apps/myapp"
        branch: "main"
        post_deploy:
          - "docker compose up -d"
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | boolean | `true` | Enable deploy module |
| `work_dir` | string | `/opt/apps` | Default working directory |
| `default_timeout` | duration | `300s` | Default deployment timeout |
| `deployments` | list | `[]` | Deployment configurations |

#### deployment

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | **Yes** | Unique deployment name (used in webhook URL) |
| `type` | enum | **Yes** | `git_pull`, `docker_pull`, or `custom_script` |

**Type-specific fields:**

| Field | For Type | Required | Description |
|-------|----------|----------|-------------|
| `path` | git_pull | **Yes** | Local repository path |
| `branch` | git_pull | No | Git branch (default: current) |
| `remote` | git_pull | No | Git remote (default: origin) |
| `ssh_key` | git_pull | No | Path to SSH private key |
| `compose_file` | docker_pull | **Yes** | Path to docker-compose.yml |
| `services` | docker_pull | No | Specific services to pull |
| `prune` | docker_pull | No | Prune old images after pull |
| `script` | custom_script | **Yes** | Script path or inline command |
| `working_dir` | custom_script | No | Script working directory |
| `user` | custom_script | No | Run script as user |

**Common fields:**

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `env` | map | `{}` | Environment variables |
| `pre_deploy` | list | `[]` | Commands to run before deploy |
| `post_deploy` | list | `[]` | Commands to run after deploy |
| `timeout` | duration | `default_timeout` | Deployment timeout |

---

### Webhooks

```yaml
modules:
  webhooks:
    enabled: true
    endpoints:
      - path: "/webhook/github"
        deployment: "myapp"
        event: "push"
        secret: "${GITHUB_WEBHOOK_SECRET}"
        allowed_ips:
          - "140.82.112.0/20"
        schedule_constraint:
          allowed_hours: [9, 10, 11, 12, 13, 14, 15, 16, 17]
          timezone: "Europe/Moscow"
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | boolean | `true` | Enable webhooks module |
| `endpoints` | list | `[]` | Custom webhook endpoints |

#### endpoint

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `path` | string | **Yes** | Webhook URL path |
| `deployment` | string | No | Deployment to trigger |
| `event` | string | No | Filter by event type (e.g., "push") |
| `secret` | string | No | HMAC secret for signature validation |
| `allowed_ips` | list | No | Allowed source IPs (CIDR) |
| `schedule_constraint` | object | No | Time-based restrictions |

#### schedule_constraint

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `allowed_hours` | list | all | Allowed hours (0-23) |
| `timezone` | string | `UTC` | Timezone for hour check |

---

## Logging

```yaml
logging:
  level: "info"
  format: "json"
  file: "/var/log/infractl/infractl.log"
  suspicious_requests: "/var/log/infractl/suspicious.log"
  rotation:
    max_size: "100MB"
    max_files: 5
    compress: true
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `level` | string | `info` | Log level: trace, debug, info, warn, error |
| `format` | string | `json` | Output format: json, pretty |
| `file` | string | `/var/log/infractl/infractl.log` | Log file path (null for stdout only) |
| `suspicious_requests` | string | `/var/log/infractl/suspicious.log` | Suspicious requests log |

#### rotation

| Field | Type | Description |
|-------|------|-------------|
| `max_size` | string | Max file size before rotation |
| `max_files` | integer | Number of rotated files to keep |
| `compress` | boolean | Compress rotated files |

---

## Notifications

```yaml
notifications:
  enabled: true
  on_deploy:
    success: true
    failure: true
  channels:
    - type: slack
      webhook_url: "${SLACK_WEBHOOK_URL}"
      channel: "#deployments"
    - type: telegram
      url: "https://api.telegram.org/bot${TG_TOKEN}/sendMessage"
      headers:
        Content-Type: "application/json"
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | boolean | `false` | Enable notifications |

#### on_deploy

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `success` | boolean | `false` | Notify on successful deploy |
| `failure` | boolean | `false` | Notify on failed deploy |

#### channel

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `type` | string | **Yes** | Channel type: slack, telegram, webhook |
| `webhook_url` | string | Slack | Slack webhook URL |
| `url` | string | Others | Target URL |
| `channel` | string | No | Channel/chat ID |
| `headers` | map | No | Custom HTTP headers |

---

## Duration Format

Durations use Go-style format:
- `30s` - 30 seconds
- `5m` - 5 minutes
- `6h` - 6 hours
- `7d` - 7 days
- `1h30m` - 1 hour 30 minutes

---

## Example Configurations

### Minimal Agent

```yaml
mode: agent
auth:
  jwt_secret: "your-secret-min-32-characters-long"
```

### Examples

See [examples/](examples/) for complete configurations:

| File | Mode | Use Case |
|------|------|----------|
| `agent.minimal.yaml` | Agent | Development/testing |
| `agent.standard.yaml` | Agent | Typical setup with deploys |
| `agent.production.yaml` | Agent | Hardened with full security |
| `home.minimal.yaml` | Home | Development/testing |
| `home.standard.yaml` | Home | Multi-agent monitoring |
| `home.production.yaml` | Home | Full fleet with alerting |
