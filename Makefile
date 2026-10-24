.PHONY: help all dev watch gr-be-watch gr-be-dev gr-be-migrate gr-be-test gr-be-docker gr-be-docker-run \
        graphrag-be-watch graphrag-be-dev graphrag-be-migrate graphrag-be-test graphrag-be-docker graphrag-be-docker-run \
        aad-be-watch aad-be-dev aad-be-migrate aad-be-test aad-be-docker aad-be-docker-run \
        migrate test test-robot docker build-docker docker-run helm-lint \
        garden-up garden-test garden-down robot-test kill-ports

CONFIG_FILE ?= graphrag-container/config.yaml
DATABASE_URL ?= postgres://postgres:mysecretpassword@localhost:5432/graphrag

BACKEND_PORT ?= 8080
BACKEND_HEALTH_PORT ?= 8079
ROBOT_REPORT_DIR ?= integration-tests/reports

all: build-docker

help:
	@echo "GraphRAG Make Targets:"
	@echo "  help                     - Display this help message"
	@echo "  dev                      - Run Rust backend dev server (alias for gr-be-dev)"
	@echo "  gr-be-dev                - Run Rust backend dev server with port cleanup"
	@echo "  watch                    - Run Rust backend with cargo watch (alias for gr-be-watch)"
	@echo "  gr-be-watch              - Run Rust backend with cargo watch auto-recompilation"
	@echo "  migrate                  - Run database migrations (alias for gr-be-migrate)"
	@echo "  gr-be-migrate            - Run database migrations against PostgreSQL"
	@echo "  test                     - Run backend unit tests via cargo test (alias for gr-be-test)"
	@echo "  gr-be-test               - Run backend unit tests via cargo test"
	@echo "  docker                   - Build backend Docker image (alias for build-docker)"
	@echo "  build-docker             - Build backend Docker image (graphrag:latest)"
	@echo "  docker-run               - Run backend container locally"
	@echo "  gr-be-docker             - Alias for build-docker"
	@echo "  gr-be-docker-run         - Alias for docker-run"
	@echo "  test-robot               - Run Robot Framework integration tests"
	@echo "  robot-test               - Alias for test-robot"
	@echo "  helm-lint                - Lint Helm chart"
	@echo "  garden-up                - Deploy environment via Garden"
	@echo "  garden-test              - Run Garden integration tests"
	@echo "  garden-down              - Tear down Garden environment"

# Helper target to cleanly kill any existing process running on the backend ports
kill-ports:
	@echo "Killing processes on ports $(BACKEND_PORT) and $(BACKEND_HEALTH_PORT)..."
	-@lsof -t -i:$(BACKEND_PORT) | xargs -r kill -9 2>/dev/null || true
	-@lsof -t -i:$(BACKEND_HEALTH_PORT) | xargs -r kill -9 2>/dev/null || true

# Starts the backend server in a standard development mode
dev: gr-be-dev
gr-be-dev: kill-ports
	cargo run --manifest-path graphrag-container/Cargo.toml -- -c $(CONFIG_FILE)

# Compatibility aliases
graphrag-be-dev: gr-be-dev
aad-be-dev: gr-be-dev

# Starts the backend server with hot-reloading enabled via cargo watch
watch: gr-be-watch
gr-be-watch:
	-@lsof -t -i:$(BACKEND_PORT) | xargs -r kill -9 2>/dev/null || true
	-@lsof -t -i:$(BACKEND_HEALTH_PORT) | xargs -r kill -9 2>/dev/null || true
	cd graphrag-container && GRAPHRAG_BE__LOGGING__LEVEL=debug cargo watch -w src -w config.yaml -x "run -- -c config.yaml"

# Compatibility aliases
graphrag-be-watch: gr-be-watch
aad-be-watch: gr-be-watch

# Executes pending database migrations
migrate: gr-be-migrate
gr-be-migrate:
	@if ! command -v sqlx >/dev/null 2>&1; then \
		echo "sqlx could not be found, installing..."; \
		cargo install sqlx-cli --no-default-features --features rustls,postgres; \
	fi
	sqlx database create --database-url $(DATABASE_URL)
	cd graphrag-container && sqlx migrate run --database-url $(DATABASE_URL)

# Compatibility aliases
graphrag-be-migrate: gr-be-migrate
aad-be-migrate: gr-be-migrate

# Runs all unit tests
test: gr-be-test
gr-be-test:
	cd graphrag-container && cargo test -- --test-threads=1

# Compatibility aliases
graphrag-be-test: gr-be-test
aad-be-test: gr-be-test

# Runs Robot Framework + Ragas integration tests against the already-running Graphrag service
test-robot:
	./integration-tests/run-tests-local.sh

robot-test: test-robot

# Docker build
docker: build-docker
gr-be-docker: build-docker
graphrag-be-docker: build-docker
aad-be-docker: build-docker
build-docker:
	docker build -t graphrag:latest graphrag-container

# Run Docker locally mapping 8080 and 8079
docker-run:
	docker run -it --rm --name graphrag \
		-p $(BACKEND_PORT):8080 \
		-p $(BACKEND_HEALTH_PORT):8079 \
		graphrag:latest

gr-be-docker-run: docker-run
graphrag-be-docker-run: docker-run
aad-be-docker-run: docker-run

# Helm lint
helm-lint:
	helm lint charts/graphrag

# Garden deployment
garden-up:
	garden deploy --env local

# Garden integration tests
garden-test:
	garden test --env local

# Garden tear down
garden-down:
	garden cleanup env --env local
