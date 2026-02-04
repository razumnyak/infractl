#!/bin/bash
# infractl installation script
# Usage: curl -fsSL https://raw.githubusercontent.com/razumnyak/infractl/main/scripts/install.sh | bash
# Or: ./install.sh [--version v0.1.14] [--mode agent|home]
#
# Alpine Linux: apk add bash curl before running

set -euo pipefail

# Check bash is available
if [ -z "${BASH_VERSION:-}" ]; then
    echo "Error: This script requires bash. On Alpine: apk add bash"
    exit 1
fi

# Configuration
REPO="razumnyak/infractl"
INSTALL_DIR="/usr/local/bin"
CONFIG_DIR="/etc/infractl"
DATA_DIR="/var/lib/infractl"
LOG_DIR="/var/log/infractl"
APPS_DIR="/var/www"
SERVICE_USER="infractl"
VERSION="${VERSION:-latest}"
MODE="${MODE:-agent}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --version)
            VERSION="$2"
            shift 2
            ;;
        --mode)
            MODE="$2"
            shift 2
            ;;
        --help)
            echo "Usage: $0 [--version VERSION] [--mode agent|home]"
            echo ""
            echo "Options:"
            echo "  --version   Version to install (default: latest)"
            echo "  --mode      Operation mode: agent or home (default: agent)"
            exit 0
            ;;
        *)
            log_error "Unknown option: $1"
            exit 1
            ;;
    esac
done

# Check root
if [[ $EUID -ne 0 ]]; then
    log_error "This script must be run as root"
    exit 1
fi

# Detect OS and architecture
detect_platform() {
    local os arch

    case "$(uname -s)" in
        Linux)  os="unknown-linux-musl" ;;
        Darwin) os="apple-darwin" ;;
        *)      log_error "Unsupported OS: $(uname -s)"; exit 1 ;;
    esac

    case "$(uname -m)" in
        x86_64|amd64)   arch="x86_64" ;;
        aarch64|arm64)  arch="aarch64" ;;
        *)              log_error "Unsupported architecture: $(uname -m)"; exit 1 ;;
    esac

    echo "${arch}-${os}"
}

# Detect init system
detect_init_system() {
    if command -v systemctl &>/dev/null && systemctl --version &>/dev/null; then
        echo "systemd"
    elif command -v rc-service &>/dev/null; then
        echo "openrc"
    else
        echo "unknown"
    fi
}

# Get latest version from GitHub
get_latest_version() {
    curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" | \
        sed -n 's/.*"tag_name": "\([^"]*\)".*/\1/p' | head -1
}

# Download binary
download_binary() {
    local version="$1"
    local platform="$2"
    local url="https://github.com/${REPO}/releases/download/${version}/infractl-${platform}"

    log_info "Downloading infractl ${version} for ${platform}..."

    if command -v curl &>/dev/null; then
        curl -fsSL -o "${INSTALL_DIR}/infractl" "$url"
    elif command -v wget &>/dev/null; then
        wget -q -O "${INSTALL_DIR}/infractl" "$url"
    else
        log_error "Neither curl nor wget found"
        exit 1
    fi

    chmod +x "${INSTALL_DIR}/infractl"
}

# Create service user
create_user() {
    if id "${SERVICE_USER}" &>/dev/null; then
        log_info "User ${SERVICE_USER} already exists"
    else
        log_info "Creating user ${SERVICE_USER}..."
        useradd -r -s /sbin/nologin -d "${DATA_DIR}" -m "${SERVICE_USER}"
    fi

    # Add to docker group if docker is installed
    if getent group docker &>/dev/null; then
        usermod -aG docker "${SERVICE_USER}"
        log_info "Added ${SERVICE_USER} to docker group"
    fi
}

# Create directories
create_directories() {
    log_info "Creating directories..."
    mkdir -p "${CONFIG_DIR}" "${DATA_DIR}" "${DATA_DIR}/.ssh" "${LOG_DIR}" "${APPS_DIR}"

    chown -R "${SERVICE_USER}:${SERVICE_USER}" "${DATA_DIR}" "${LOG_DIR}" "${APPS_DIR}"
    chown -R "${SERVICE_USER}:${SERVICE_USER}" "${CONFIG_DIR}"

    chmod 755 "${CONFIG_DIR}" "${DATA_DIR}" "${LOG_DIR}" "${APPS_DIR}"
    chmod 700 "${DATA_DIR}/.ssh"
}

# Generate SSH deploy key
create_ssh_key() {
    local key_path="${DATA_DIR}/.ssh/deploy"

    if [[ -f "${key_path}" ]]; then
        log_info "SSH deploy key already exists"
    else
        log_info "Generating SSH deploy key..."
        sudo -u "${SERVICE_USER}" ssh-keygen -t ed25519 -f "${key_path}" -N "" -C "infractl-deploy"

        # Create default SSH config
        cat > "${DATA_DIR}/.ssh/config" << EOF
# Default deploy key for all GitHub repos
Host github.com
    HostName github.com
    User git
    IdentityFile ${key_path}
    IdentitiesOnly yes
    StrictHostKeyChecking accept-new

# Example: per-repo key (uncomment and customize)
# Host github-myapp
#     HostName github.com
#     User git
#     IdentityFile ${DATA_DIR}/.ssh/myapp
#     IdentitiesOnly yes
EOF
        chown "${SERVICE_USER}:${SERVICE_USER}" "${DATA_DIR}/.ssh/config"
        chmod 600 "${DATA_DIR}/.ssh/config"

        echo ""
        log_info "Deploy key public key (add to GitHub repo as Deploy Key):"
        echo ""
        cat "${key_path}.pub"
        echo ""
    fi
}

# Create default config
create_config() {
    if [[ -f "${CONFIG_DIR}/config.yaml" ]]; then
        log_warn "Config already exists at ${CONFIG_DIR}/config.yaml, skipping..."
        return
    fi

    log_info "Creating default config for ${MODE} mode..."

    if [[ "$MODE" == "home" ]]; then
        cat > "${CONFIG_DIR}/config.yaml" << 'EOF'
# infractl Home mode configuration
mode: home

server:
  bind: "0.0.0.0"
  port: 8111
  isolation_mode: true
  allowed_networks:
    - "10.0.0.0/8"
    - "172.16.0.0/12"
    - "192.168.0.0/16"
    - "127.0.0.1/32"

auth:
  jwt_secret: "CHANGE_ME_TO_A_SECURE_SECRET"
  token_ttl: "24h"

# List your agents here
agents: []
#  - name: "server-1"
#    address: "http://10.0.0.10:8111"
#    timeout: "10s"

modules:
  metrics:
    enabled: true
    collect_interval: "30s"
    docker_stats: true

  storage:
    enabled: true
    db_path: "/var/lib/infractl/metrics.db"
    retention:
      raw_data: "7d"
      hourly_data: "30d"
      daily_data: "365d"

  deploy:
    enabled: true
    work_dir: "/var/www"
    deployments: []

  webhooks:
    enabled: true
    endpoints: []

logging:
  level: "info"
  format: "json"
  file: "/var/log/infractl/infractl.log"
EOF
    else
        cat > "${CONFIG_DIR}/config.yaml" << 'EOF'
# infractl Agent mode configuration
mode: agent

server:
  bind: "0.0.0.0"
  port: 8111
  isolation_mode: true
  allowed_networks:
    - "10.0.0.0/8"
    - "172.16.0.0/12"
    - "192.168.0.0/16"
    - "127.0.0.1/32"

auth:
  jwt_secret: "CHANGE_ME_TO_A_SECURE_SECRET"
  token_ttl: "24h"

modules:
  metrics:
    enabled: true
    collect_interval: "30s"
    docker_stats: true
    compose_projects: true

  storage:
    enabled: false

  deploy:
    enabled: true
    work_dir: "/var/www"
    deployments: []
    #  - name: "myapp"
    #    type: git_pull
    #    path: "/opt/apps/myapp"
    #    branch: "main"
    #    post_deploy:
    #      - "docker compose up -d"

  webhooks:
    enabled: true
    endpoints: []

logging:
  level: "info"
  format: "json"
  file: "/var/log/infractl/infractl.log"
EOF
    fi

    chmod 600 "${CONFIG_DIR}/config.yaml"
    log_warn "Please edit ${CONFIG_DIR}/config.yaml and set a secure jwt_secret!"
}

# Install systemd service
install_systemd() {
    log_info "Installing systemd service..."

    local service_url="https://raw.githubusercontent.com/${REPO}/main/infractl.service"

    if command -v curl &>/dev/null; then
        curl -fsSL -o /etc/systemd/system/infractl.service "$service_url"
    elif command -v wget &>/dev/null; then
        wget -q -O /etc/systemd/system/infractl.service "$service_url"
    fi

    systemctl daemon-reload
    systemctl enable infractl
}

# Install OpenRC service
install_openrc() {
    log_info "Installing OpenRC service..."

    local service_url="https://raw.githubusercontent.com/${REPO}/main/infractl.openrc"

    if command -v curl &>/dev/null; then
        curl -fsSL -o /etc/init.d/infractl "$service_url"
    elif command -v wget &>/dev/null; then
        wget -q -O /etc/init.d/infractl "$service_url"
    fi

    chmod +x /etc/init.d/infractl
    rc-update add infractl default
}

# Main installation
main() {
    log_info "Installing infractl..."

    local platform init_system

    platform=$(detect_platform)
    init_system=$(detect_init_system)

    # Get version
    if [[ "$VERSION" == "latest" ]]; then
        VERSION=$(get_latest_version)
        log_info "Latest version: ${VERSION}"
    fi

    # Create service user
    create_user

    # Create directories
    create_directories

    # Generate SSH deploy key
    create_ssh_key

    # Download binary
    download_binary "$VERSION" "$platform"

    # Verify installation
    if ! "${INSTALL_DIR}/infractl" --version &>/dev/null; then
        log_error "Binary verification failed"
        exit 1
    fi

    # Create config
    create_config

    # Install service
    case "$init_system" in
        systemd) install_systemd ;;
        openrc)  install_openrc ;;
        *)       log_warn "Unknown init system, skipping service installation" ;;
    esac

    log_info "Installation complete!"
    echo ""
    echo "Next steps:"
    echo "  1. Edit the configuration: ${CONFIG_DIR}/config.yaml"
    echo "  2. Set a secure JWT secret"
    echo "  3. Start the service:"
    if [[ "$init_system" == "systemd" ]]; then
        echo "     systemctl start infractl"
        echo "     systemctl status infractl"
    elif [[ "$init_system" == "openrc" ]]; then
        echo "     rc-service infractl start"
        echo "     rc-service infractl status"
    fi
    echo ""
    echo "View logs:"
    echo "  journalctl -u infractl -f"
    echo "  tail -f ${LOG_DIR}/infractl.log"
}

main
