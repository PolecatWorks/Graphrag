.PHONY: help all dev watch graphrag-be-watch graphrag-be-dev graphrag-be-migrate graphrag-be-test graphrag-be-docker graphrag-be-docker-run \
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
	@echo "  dev                      - Run Rust backend dev server (alias for graphrag-be-dev)"
	@echo "  graphrag-be-dev          - Run Rust backend dev server with port cleanup"
	@echo "  watch                    - Run Rust backend with cargo watch (alias for graphrag-be-watch)"
	@echo "  graphrag-be-watch        - Run Rust backend with cargo watch auto-recompilation"
	@echo "  migrate                  - Run database migrations (alias for graphrag-be-migrate)"
	@echo "  graphrag-be-migrate      - Run database migrations against PostgreSQL"
	@echo "  test                     - Run backend unit tests via cargo test (alias for graphrag-be-test)"
	@echo "  graphrag-be-test         - Run backend unit tests via cargo test"
	@echo "  docker                   - Build backend Docker image (alias for build-docker)"
	@echo "  build-docker             - Build backend Docker image (graphrag:latest)"
	@echo "  docker-run               - Run backend container locally"
	@echo "  graphrag-be-docker       - Alias for build-docker"
	@echo "  graphrag-be-docker-run   - Alias for docker-run"
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
dev: graphrag-be-dev
graphrag-be-dev: kill-ports
	cargo run --manifest-path graphrag-container/Cargo.toml -- -c $(CONFIG_FILE)

# Backward-compatibility alias
aad-be-dev: graphrag-be-dev

# Starts the backend server with hot-reloading enabled via cargo watch
watch: graphrag-be-watch
graphrag-be-watch:
	-@lsof -t -i:$(BACKEND_PORT) | xargs -r kill -9 2>/dev/null || true
	-@lsof -t -i:$(BACKEND_HEALTH_PORT) | xargs -r kill -9 2>/dev/null || true
	cd graphrag-container && cargo watch -w src -w config.yaml -x "run -- -c config.yaml"

# Backward-compatibility alias
aad-be-watch: graphrag-be-watch

# Executes pending database migrations
migrate: graphrag-be-migrate
graphrag-be-migrate:
	@if ! command -v sqlx >/dev/null 2>&1; then \
		echo "sqlx could not be found, installing..."; \
		cargo install sqlx-cli --no-default-features --features rustls,postgres; \
	fi
	sqlx database create --database-url $(DATABASE_URL)
	cd graphrag-container && sqlx migrate run --database-url $(DATABASE_URL)

# Backward-compatibility alias
aad-be-migrate: graphrag-be-migrate

# Runs all unit tests
test: graphrag-be-test
graphrag-be-test:
	cd graphrag-container && cargo test -- --test-threads=1

# Backward-compatibility alias
aad-be-test: graphrag-be-test

# Runs Robot Framework + Ragas integration tests against the already-running Graphrag service
test-robot:
	./integration-tests/run-tests-local.sh

robot-test: test-robot

# Docker build
docker: build-docker
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
