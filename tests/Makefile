.PHONY: up down build test test-basic test-security test-deploy test-integration logs clean help shell-home shell-agent

# Default target
help:
	@echo "infractl Test Environment"
	@echo ""
	@echo "Setup:"
	@echo "  make build           - Build test containers"
	@echo "  make up              - Start test environment"
	@echo "  make down            - Stop test environment"
	@echo "  make clean           - Remove all test data and images"
	@echo ""
	@echo "Testing:"
	@echo "  make test            - Run ALL tests"
	@echo "  make test-basic      - Run basic health/auth tests"
	@echo "  make test-security   - Run security tests"
	@echo "  make test-deploy     - Run deploy workflow tests"
	@echo "  make test-integration - Run integration tests"
	@echo ""
	@echo "Debugging:"
	@echo "  make logs            - Show container logs"
	@echo "  make shell-home      - Shell into Home container"
	@echo "  make shell-agent     - Shell into Agent container"
	@echo ""

# Build containers
build:
	docker compose -f docker-compose.test.yml build

# Start test environment
up: build
	docker compose -f docker-compose.test.yml up -d
	@echo "Waiting for services to be healthy..."
	@sleep 15
	@echo "Initializing Gitea..."
	@docker compose -f docker-compose.test.yml exec -T gitea sh /etc/gitea/init.sh 2>/dev/null || true
	@echo ""
	@echo "Test environment is ready!"
	@echo "  Home:    http://localhost:8111/monitoring"
	@echo "  Gitea:   http://localhost:3000 (admin/admin123)"
	@echo ""

# Stop test environment
down:
	docker compose -f docker-compose.test.yml down

# Run ALL tests
test: up
	@echo "Running all test scenarios..."
	@docker compose -f docker-compose.test.yml exec -T runner sh /scenarios/run-all.sh all

# Run basic tests only
test-basic: up
	@echo "Running basic tests..."
	@docker compose -f docker-compose.test.yml exec -T runner sh /scenarios/run-all.sh basic

# Run security tests only
test-security: up
	@echo "Running security tests..."
	@docker compose -f docker-compose.test.yml exec -T runner sh /scenarios/run-all.sh security

# Run deploy workflow tests only
test-deploy: up
	@echo "Running deploy workflow tests..."
	@docker compose -f docker-compose.test.yml exec -T runner sh /scenarios/run-all.sh deploy

# Run integration tests only
test-integration: up
	@echo "Running integration tests..."
	@docker compose -f docker-compose.test.yml exec -T runner sh /scenarios/run-all.sh integration

# Show logs
logs:
	docker compose -f docker-compose.test.yml logs -f

# Shell access
shell-home:
	docker compose -f docker-compose.test.yml exec home sh

shell-agent:
	docker compose -f docker-compose.test.yml exec agent-1 sh

# Clean up everything
clean: down
	docker compose -f docker-compose.test.yml down -v --rmi local
	docker system prune -f

# Quick rebuild and test
rebuild: down build up test
