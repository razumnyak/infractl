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
- **Deploy**: git_pull | docker_pull | custom_script, SSH keys
- **Updater**: self-update из GitHub Releases, config sync
- **Web**: axum, embedded HTML (rust-embed), JWT middleware

## Ключевые endpoints
| Endpoint | Mode | Назначение |
|----------|------|------------|
| GET /health | Agent | JSON метрики |
| GET /monitoring | Home | Dashboard UI |
| POST /webhook/deploy/{name} | Both | Trigger deploy |
| GET /api/agents | Home | Статус агентов |

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
    deployments:
      - name: "api"
        type: git_pull
        path: "/opt/apps/api"
        post_deploy: ["docker compose up -d"]
```

## Security
- JWT Bearer tokens
- Network isolation (только allowed_networks)
- Suspicious requests → отдельный лог

## Файлы проекта (по запуску программы)
- `/etc/infractl/config.yaml` — конфиг
- `/var/lib/infractl/metrics.db` — SQLite (Home)
- `/var/log/infractl/` — логи
