# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).


## [0.1.19] - 2026-03-13

### Added

- **`--force` CLI flag** (`infractl deploy --force -n <name>`) — bypasses path restrictions for a single deployment run. Only works from localhost, ignored via external webhook. Path traversal is always checked
- **`allowed_deploy_paths`** config option — extend default allowed directories (`/opt/apps`, `/srv`, `/var/www`, `/home`, `/tmp`) with custom paths like `/etc/infractl`

## [0.1.18] - 2026-03-13

### Added

- **Protected deployment category** (`category: protected`) — CLI-only deployments for security-critical configs (tokens, .env, allowed_networks). Cannot be triggered via webhook or by non-protected deployments. Protected-to-protected triggers are allowed.

### Fixed

- **Infinite trigger loop** — system deployments no longer fire global `on_success`/`on_error` triggers, preventing recursive loops (e.g., telegram-notifier triggering itself)
- Global triggers now only apply to `app` category deployments

### Changed

- `deploy --list` shows `[protected]` label for protected deployments
- Webhook returns 403 for both `system` and `protected` deployments

## [0.1.17] - 2026-03-12

### Added

- **Deployment categories** (`category: app | system`) — system deployments can only be triggered by other deployments, not via webhook, CLI, or cron
- **Three-level trigger system**:
  - **Deployment-level**: `on_success` and `on_error` triggers per deployment (replaces `trigger` field, backward-compatible via alias)
  - **Global-level**: `deploy.on_success` and `deploy.on_error` triggers that fire on any app deployment result
  - **Pipeline-level**: `pipeline.on_start` and `pipeline.on_finish` hooks for wrapping entire deployment chains (e.g., maintenance windows)
- **Telegram notifications** (`type: telegram`) — built-in Telegram Bot API integration with auto-silent mode (success = silent, error = loud), customizable templates, zero new dependencies
- **Pipeline tracking** — `pipeline_id` groups all chained deployments; query via `GET /api/pipeline/{id}`
- **Context environment variables** for triggered deployments: `DEPLOY_NAME`, `DEPLOY_STATUS`, `DEPLOY_ERROR`, `AGENT_NAME`, `PIPELINE_ID`, `TRIGGER_TYPE`

### Changed

- `trigger` field renamed to `on_success` (alias preserved for backward compatibility)
- Deploy worker now accepts `DeployConfig` instead of `Vec<DeploymentConfig>`
- Webhook response includes `pipeline_id`
- `deploy --list` shows `[system]` label for system deployments

## [0.1.15-16] - 2026-02-05

### Added

- SSH key security validation (permissions check 0600/0400)
- Command injection prevention in deploy scripts
- Webhook signature verification (GitHub HMAC-SHA256, GitLab token)
- `GET /api/deployments/:name` endpoint for fetching deployment config
- Agent → Home config fetch for deployments not found locally
- Self-update checksum verification

### Changed

- Improved git deploy: SSH key handling via `GIT_SSH_COMMAND`
- Enhanced script runner with timeout support and user switching (`sudo`)
- Refactored webhook handler with proper signature detection (GitHub, GitLab, Bitbucket)

## [0.1.8-14] - 2026-02-04

### Added

- External deployments support (`deployments.yaml`, `deployments.d/*.yaml`)
- Docker deploy strategies: `default`, `force_recreate`, `restart`
- `git_files` — fetch specific files from git without full clone
- `prune` option for cleaning old Docker images
- Path security validation (traversal protection, allowed directories whitelist)
- `shutdown` commands and `POST /webhook/shutdown/:name` endpoint
- `continue_on_failure` for pipeline resilience

### Changed

- Refactored deploy executor with separate git, docker, script modules
- Improved git pull: detect changes via commit hash comparison, skip post-deploy if no changes

## [0.1.7] - 2026-02-03

### Added

- Git pull deploy: auto-clone on first deploy, then fetch+reset
- Docker pull deploy: `git_files` fetch + compose up with strategy
- Deployment trigger pipelines (`trigger` field)

### Fixed

- `bytes` crate updated to 1.11.1 (RUSTSEC-2026-0007)

### Changed

- Improved release script (cargo.lock update, cargo fmt)

## [0.1.5] - 2026-02-02

### Added

- Dashboard JWT authentication (embedded token for API calls)
- `infractl deploy` CLI command with agent forwarding

### Changed

- Dashboard API calls now include `Authorization` header

## [0.1.3-4] - 2026-02-01

### Added

- Initial release
- Home/Agent architecture with JWT auth and network isolation
- System metrics collection (CPU, RAM, Docker stats)
- SQLite storage with retention and aggregation
- Monitoring dashboard (rust-embed)
- Webhook-based deployment triggers
- Self-update from GitHub Releases
- Release script for version management
