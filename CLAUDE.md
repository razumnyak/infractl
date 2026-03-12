# infractl - Infrastructure Control Agent

## Суть
Rust-бинарник для мониторинга серверов и автодеплоя.
Два режима: **Home** (центральный) и **Agent** (воркер). 
Общение по internal network 10.0.0.0/8, JWT auth.

# Structure current repository
```
deployer/                       #infractl
├── .git/
├── src/                        # for development
├── .github/
│   └── workflows/
│       └── ci.yml              # CI/CD pipeline
├── docs/
│   ├── API.md                  # API спецификация
│   ├── Architecture.md         # Техническая архитектура
│   ├── Config.example          # Пример конфигурации (yaml)
│   ├── PRD.md                  # Product Requirements Document
│   └── Roadmap.md              # План разработки
├── Cargo.toml                  # Rust manifest
├── Dockerfile                  # Container build
└── infractl.service            # Systemd unit
```

## Архитектура
```
Home (:8111/monitoring, /webhook) → internal net → Agent(s) (:8111/health, /webhook)
```

## Модули
- **Config**: YAML, env substitution `${VAR}`, hot-reload
- **Metrics**: CPU/RAM/Docker stats (sysinfo, bollard)
- **Storage**: SQLite, только Home, retention + aggregation
- **Deploy**: git_pull | docker_pull | custom_script | telegram, SSH keys
- **Triggers**: on_success/on_error (per-deploy, global, pipeline-level)
- **Categories**: app (default) | system (internal only, no webhook/CLI)
- **Updater**: self-update из GitHub Releases, config sync
- **Web**: axum, embedded HTML (rust-embed), JWT middleware

## Ключевые endpoints
| Endpoint | Mode | Назначение |
|----------|------|------------|
| GET /health | Both | JSON метрики |
| GET /monitoring | Home | Dashboard UI |
| POST /webhook/deploy/{name} | Both | Trigger deploy |
| POST /webhook/shutdown/{name} | Both | Stop deploy |
| GET /webhook/status/{job_id} | Both | Job status |
| GET /webhook/queue | Both | Queue + history |
| GET /api/pipeline/{id} | Both | Pipeline status |
| GET /api/agents | Home | Статус агентов |
| GET /api/deployments/{name} | Both | Deployment config |

## Stack
tokio, axum, serde_yaml, rusqlite, bollard, git2, jsonwebtoken, sysinfo, rust-embed, clap

## Сборка
```bash
cross build --release --target x86_64-unknown-linux-musl
```

## Конфиг (ключевое)
```yaml
mode: agent|home
server:
  port: 8111
  isolation_mode: true
  allowed_networks: ["10.0.0.0/8"]
modules:
  deploy:
    on_error: "telegram-notifier"     # global error trigger
    deployments:
      - name: "api"
        type: git_pull
        path: "/opt/apps/api"
        post_deploy: ["docker compose up -d"]
        on_success: "frontend"
        on_error: "api-rollback"
        pipeline:
          on_start: "maintenance-on"
          on_finish: "maintenance-off"
      - name: "telegram-notifier"
        category: system
        type: telegram
        telegram:
          bot_token: "${TG_BOT_TOKEN}"
          chat_id: "${TG_CHAT_ID}"
```

## Security
- JWT Bearer tokens
- Network isolation (только allowed_networks)
- Suspicious requests → отдельный лог

## Файлы проекта (по запуску программы)
- `/etc/infractl/config.yaml` — конфиг
- `/var/lib/infractl/metrics.db` — SQLite (Home)
- `/var/log/infractl/` — логи
