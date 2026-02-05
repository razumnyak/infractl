#!/bin/sh
# Run all test scenarios
# Usage: ./run-all.sh [group]
# Groups: basic, security, deploy, integration, all (default)

GROUP="${1:-all}"
PASSED=0
FAILED=0
SKIPPED=0
SCENARIOS_DIR="/scenarios"

# Colors (using printf for proper escape sequence handling)
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Helper functions for colored output
print_red()    { printf "${RED}%s${NC}\n" "$1"; }
print_green()  { printf "${GREEN}%s${NC}\n" "$1"; }
print_yellow() { printf "${YELLOW}%s${NC}\n" "$1"; }
print_blue()   { printf "${BLUE}%s${NC}\n" "$1"; }

run_test() {
    local script="$1"
    local name=$(basename "$script" .sh)

    printf "\n"
    print_blue "==========================================="
    print_blue "Running: $name"
    print_blue "==========================================="

    if [ ! -f "$script" ]; then
        print_yellow "⚠ Skipped: $name (not found)"
        SKIPPED=$((SKIPPED + 1))
        return
    fi

    if sh "$script"; then
        PASSED=$((PASSED + 1))
        print_green "✓ $name PASSED"
    else
        FAILED=$((FAILED + 1))
        print_red "✗ $name FAILED"
    fi
}

print_header() {
    printf "\n"
    print_blue "============================================"
    printf "${BLUE}  infractl Integration Tests - %s${NC}\n" "$1"
    print_blue "============================================"
}

# Basic tests (quick sanity checks)
run_basic() {
    print_header "Basic Tests"
    run_test "$SCENARIOS_DIR/01_health.sh"
    run_test "$SCENARIOS_DIR/02_auth.sh"
    run_test "$SCENARIOS_DIR/06_metrics.sh"
}

# Security tests
run_security() {
    print_header "Security Tests"
    run_test "$SCENARIOS_DIR/09_security.sh"
    run_test "$SCENARIOS_DIR/10_security_injection.sh"
    run_test "$SCENARIOS_DIR/11_security_jwt.sh"
    run_test "$SCENARIOS_DIR/12_security_network.sh"
}

# Deploy workflow tests
run_deploy() {
    print_header "Deploy Workflow Tests"
    run_test "$SCENARIOS_DIR/03_git_deploy.sh"
    run_test "$SCENARIOS_DIR/05_webhook.sh"
    run_test "$SCENARIOS_DIR/20_deploy_git.sh"
    run_test "$SCENARIOS_DIR/21_deploy_docker.sh"
    run_test "$SCENARIOS_DIR/22_deploy_script.sh"
    run_test "$SCENARIOS_DIR/23_deploy_webhook.sh"
}

# Integration tests
run_integration() {
    print_header "Integration Tests"
    run_test "$SCENARIOS_DIR/30_integration_metrics.sh"
    run_test "$SCENARIOS_DIR/31_integration_home_agent.sh"
    run_test "$SCENARIOS_DIR/32_integration_config.sh"
}

# Run selected group
case "$GROUP" in
    basic)
        run_basic
        ;;
    security)
        run_security
        ;;
    deploy)
        run_deploy
        ;;
    integration)
        run_integration
        ;;
    all)
        run_basic
        run_security
        run_deploy
        run_integration
        ;;
    *)
        echo "Unknown group: $GROUP"
        echo "Usage: $0 [basic|security|deploy|integration|all]"
        exit 1
        ;;
esac

# Print results
printf "\n"
print_blue "==========================================="
print_blue "  Test Results"
print_blue "==========================================="
printf "  ${GREEN}Passed:${NC}  %d\n" "$PASSED"
printf "  ${RED}Failed:${NC}  %d\n" "$FAILED"
printf "  ${YELLOW}Skipped:${NC} %d\n" "$SKIPPED"
printf "\n"

if [ $FAILED -gt 0 ]; then
    print_red "TESTS FAILED"
    exit 1
else
    print_green "ALL TESTS PASSED"
    exit 0
fi
