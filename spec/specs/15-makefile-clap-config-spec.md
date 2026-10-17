# Spec 15: Developer Tooling Configuration & Clap Arguments (Makefile)

**Status:** `complete`
**Derived From:** [Backend Foundations PRD](../prds/backend-foundations-prd.md)
**Supersedes:** [Spec 01: Developer Tooling (Makefile)](./01-developer-tooling-spec.md)

## Objective
Provide ergonomic, standard developer tooling in the root `Makefile` that supplies the configuration file path via the `-c` / `--config` clap argument to both `cargo run` and `cargo watch`, ensuring local development and watch mode seamlessly start with a valid default configuration file (`config.yaml`).

## Requirements

### 1. Root Makefile Configuration Variables (`Makefile`)
- `CONFIG_FILE ?= config.yaml`: Configurable path to the YAML configuration file, defaulting to `config.yaml`.
- `DATABASE_URL ?= postgres://postgres:mysecretpassword@localhost:5432/graphrag`: Configurable connection URL for database creation and migrations.

### 2. Developer Targets (`Makefile`)
- `make kill-ports`: Kills lingering processes listening on ports `8080` (Axum) and `8079` (HaMS).
- `make dev`: Invokes `kill-ports` and executes `cargo run -- -c $(CONFIG_FILE)`.
- `make watch` & `make aad-be-watch`: Starts `cargo watch` configured with `-x "run -- -c $(CONFIG_FILE)"` to hot-reload on source edits while passing the clap config argument.
- `make migrate`: Creates database and runs migrations using `--database-url $(DATABASE_URL)`.
- `make test`: Runs unit tests via `cargo test -- --test-threads=1`.
- `make test-robot`: Runs Robot Framework and Ragas integration tests via `./integration-tests/run-tests-local.sh`.

### 3. Baseline Development Configuration (`config.yaml`)
- A baseline `config.yaml` is provided at the repository root with valid development configurations:
  - `database`: PostgreSQL connection URL, pool size `10`, and acquisition timeout `30s`.
  - `server_host`: `"127.0.0.1"`, `server_port`: `8080`.
  - `hams`: Address `"127.0.0.1:8079"`, service name `"graphrag"`, version `"0.1.0"`.
  - `llm`: Provider `"ollama"`, base URL `"http://localhost:11434/v1"`, model `"llama3"`, timeout `60s`.

## Testing Strategy
1. **Makefile Target Syntax Verification**:
   - Verify `make --dry-run dev`, `make --dry-run watch`, `make --dry-run migrate`, and `make --dry-run test`.
2. **Configuration Load & Startup Test**:
   - Verify `cargo run -- -c config.yaml` starts successfully and binds ports `8080` and `8079`.
