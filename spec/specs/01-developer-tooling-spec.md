# Spec 01: Developer Tooling (Makefile)

**Status:** `deprecated` (Superseded by [Spec 15: Developer Tooling Configuration & Clap Arguments](./15-makefile-clap-config-spec.md))
**Derived From:** [Backend Foundations PRD](../prds/backend-foundations-prd.md)

## Objective
Provide a unified, ergonomic entry point for developers to run, test, and manage the Rust backend application via a root-level `Makefile`.

## Requirements

The `Makefile` must exist at the repository root and define the following targets exactly as named, conforming to the behaviors specified.

### 1. `make dev`
- **Behavior:** Starts the backend server in a standard development mode.
- **Pre-execution:** Must cleanly kill any existing process running on the backend port (e.g., `8080` and `8079`) to prevent `EADDRINUSE` errors.
- **Command:** Uses `cargo run` (or equivalent) to boot the application.

### 2. `make watch` or `make aad-be-watch`
- **Behavior:** Starts the backend server with hot-reloading enabled via `cargo watch`.
- **Command:** `cargo watch -x run` (or equivalent parameters to ensure restarting on file save).
- **Constraints:** As per `AGENTS.md`, developers should not manually kill or restart this process; they should simply save files.

### 3. `make migrate`
- **Behavior:** Executes pending database migrations.
- **Command:** Uses `sqlx database create` and `sqlx migrate run` (or equivalent depending on the setup) to ensure the database schema is up-to-date.

### 4. `make test`
- **Behavior:** Runs all unit and integration tests.
- **Command:** Typically maps to `cargo test` and potentially triggers an integration script (e.g., `/integration-tests/run-tests-local.sh`).

## Testing Strategy
- **Verification:**
  1. Parse the `Makefile` to ensure all required targets are present.
  2. Execute `make --dry-run <target>` for each target in a CI environment to verify syntax and tool presence.
