# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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

## [0.1.16] - 2026-03-10

### Added

- Initial stable release
