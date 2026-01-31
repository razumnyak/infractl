# Configuration Examples

## Agent Mode

| File | Description |
|------|-------------|
| `agent.minimal.yaml` | Development - no security, metrics only |
| `agent.standard.yaml` | Typical setup with git deployments |
| `agent.production.yaml` | Hardened with webhooks, auto-update, notifications |

## Home Mode

| File | Description |
|------|-------------|
| `home.minimal.yaml` | Development - local testing with SQLite |
| `home.standard.yaml` | Multi-agent monitoring |
| `home.production.yaml` | Full fleet (9 agents), alerting, long retention |

## Usage

```bash
# Copy and customize
cp examples/agent.standard.yaml /etc/infractl/config.yaml
vim /etc/infractl/config.yaml

# Set secrets via environment
export JWT_SECRET="your-32-char-secret-here"
export GITHUB_WEBHOOK_SECRET="webhook-secret"

# Run
infractl --config /etc/infractl/config.yaml
```

See [CONFIG.md](../CONFIG.md) for full option reference.
