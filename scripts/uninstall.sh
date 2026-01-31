#!/bin/bash
# infractl uninstallation script
# Usage: ./uninstall.sh [--purge]

set -euo pipefail

# Configuration
INSTALL_DIR="/usr/local/bin"
CONFIG_DIR="/etc/infractl"
DATA_DIR="/var/lib/infractl"
LOG_DIR="/var/log/infractl"
PURGE=false

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
        --purge)
            PURGE=true
            shift
            ;;
        --help)
            echo "Usage: $0 [--purge]"
            echo ""
            echo "Options:"
            echo "  --purge   Remove all data, configs, and logs"
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

# Stop service
stop_service() {
    local init_system
    init_system=$(detect_init_system)

    log_info "Stopping infractl service..."

    case "$init_system" in
        systemd)
            if systemctl is-active --quiet infractl 2>/dev/null; then
                systemctl stop infractl || true
            fi
            ;;
        openrc)
            if rc-service infractl status &>/dev/null; then
                rc-service infractl stop || true
            fi
            ;;
    esac
}

# Remove service
remove_service() {
    local init_system
    init_system=$(detect_init_system)

    log_info "Removing service files..."

    case "$init_system" in
        systemd)
            systemctl disable infractl 2>/dev/null || true
            rm -f /etc/systemd/system/infractl.service
            systemctl daemon-reload
            ;;
        openrc)
            rc-update del infractl default 2>/dev/null || true
            rm -f /etc/init.d/infractl
            ;;
    esac
}

# Remove binary
remove_binary() {
    log_info "Removing binary..."
    rm -f "${INSTALL_DIR}/infractl"
}

# Remove data (only with --purge)
remove_data() {
    if [[ "$PURGE" == "true" ]]; then
        log_warn "Removing all data, configs, and logs..."

        # Backup config before removal
        if [[ -f "${CONFIG_DIR}/config.yaml" ]]; then
            local backup="/tmp/infractl-config-backup-$(date +%Y%m%d%H%M%S).yaml"
            cp "${CONFIG_DIR}/config.yaml" "$backup"
            log_info "Config backed up to: $backup"
        fi

        rm -rf "${CONFIG_DIR}"
        rm -rf "${DATA_DIR}"
        rm -rf "${LOG_DIR}"
    else
        log_info "Keeping config, data, and logs (use --purge to remove)"
        log_info "  Config: ${CONFIG_DIR}"
        log_info "  Data:   ${DATA_DIR}"
        log_info "  Logs:   ${LOG_DIR}"
    fi
}

# Main
main() {
    log_info "Uninstalling infractl..."

    # Confirm purge
    if [[ "$PURGE" == "true" ]]; then
        echo ""
        log_warn "WARNING: This will permanently delete all infractl data!"
        read -p "Are you sure? (yes/no): " confirm
        if [[ "$confirm" != "yes" ]]; then
            log_info "Aborted"
            exit 0
        fi
    fi

    stop_service
    remove_service
    remove_binary
    remove_data

    log_info "Uninstallation complete!"

    if [[ "$PURGE" != "true" ]]; then
        echo ""
        echo "To completely remove all data, run:"
        echo "  $0 --purge"
    fi
}

main
