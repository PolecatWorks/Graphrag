.PHONY: kill-ports dev watch aad-be-watch migrate test test-robot docker build-docker docker-run helm-lint garden-up garden-test

CONFIG_FILE ?= config.yaml
DATABASE_URL ?= postgres://postgres:mysecretpassword@localhost:5432/graphrag

# Helper target to cleanly kill any existing process running on the backend ports
kill-ports:
	@echo "Killing processes on ports 8080 and 8079..."
	-@lsof -t -i:8080 | xargs -r kill -9 2>/dev/null || true
	-@lsof -t -i:8079 | xargs -r kill -9 2>/dev/null || true

# Starts the backend server in a standard development mode
dev: kill-ports
	cd graphrag-container && cargo run -- -c $(CONFIG_FILE)

# Starts the backend server with hot-reloading enabled via cargo watch
watch:
	cd graphrag-container && cargo watch -x "run -- -c $(CONFIG_FILE)"

aad-be-watch: watch

# Executes pending database migrations
migrate:
	@if ! command -v sqlx >/dev/null 2>&1; then \
		echo "sqlx could not be found, installing..."; \
		cargo install sqlx-cli --no-default-features --features rustls,postgres; \
	fi
	sqlx database create --database-url $(DATABASE_URL)
	cd graphrag-container && sqlx migrate run --database-url $(DATABASE_URL)

# Runs all unit tests
test:
	cd graphrag-container && cargo test -- --test-threads=1

# Runs Robot Framework + Ragas integration tests against the already-running Graphrag service
test-robot:
	./integration-tests/run-tests-local.sh

# Docker build
docker: build-docker
build-docker:
	docker build -t graphrag:latest graphrag-container

# Run Docker locally mapping 8080 and 8079
docker-run:
	docker run -p 8080:8080 -p 8079:8079 graphrag:latest

# Helm lint
helm-lint:
	helm lint charts/graphrag

# Garden deployment
garden-up:
	garden deploy --env local

# Garden integration tests
garden-test:
	garden test --env local
