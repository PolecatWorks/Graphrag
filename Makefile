.PHONY: kill-ports dev watch aad-be-watch migrate test test-robot

# Helper target to cleanly kill any existing process running on the backend ports
kill-ports:
	@echo "Killing processes on ports 8080 and 8079..."
	-@lsof -t -i:8080 | xargs -r kill -9 2>/dev/null || true
	-@lsof -t -i:8079 | xargs -r kill -9 2>/dev/null || true

# Starts the backend server in a standard development mode
dev: kill-ports
	cargo run

# Starts the backend server with hot-reloading enabled via cargo watch
watch:
	cargo watch -x run

aad-be-watch: watch

# Executes pending database migrations
migrate:
	@if ! command -v sqlx >/dev/null 2>&1; then \
		echo "sqlx could not be found, installing..."; \
		cargo install sqlx-cli --no-default-features --features rustls,postgres; \
	fi
	sqlx database create
	sqlx migrate run

# Runs all unit tests
test:
	cargo test -- --test-threads=1

# Runs Robot Framework + Ragas integration tests against the already-running Graphrag service
test-robot:
	./integration-tests/run-tests-local.sh

