# Spec 13: CLI Argument Parsing (`clap` Derive)

**Status:** `complete`
**Derived From:** [Backend Foundations PRD](../prds/backend-foundations-prd.md)

## Objective
Ensure all command-line argument parsing in `src/cli.rs` is declaratively defined using `clap` with the `derive` feature (`#[derive(clap::Parser)]`) and verified with comprehensive unit tests.

## Requirements

### 1. Declarative `clap` Derive Struct (`src/cli.rs`)
- `Cli` in `src/cli.rs` must derive `clap::Parser`, `Debug`, and `PartialEq`.
- Must support the optional `-c` / `--config <FILE>` argument (`pub config: Option<String>`).
- Imperative `clap::Command` builder chains or direct `std::env::args` reads are prohibited.

### 2. Startup Integration (`src/main.rs`)
- `main()` invokes `Cli::parse()` and passes `cli.config.as_deref()` into `AppConfig::load(...)`.

## Testing Strategy
- **Unit Tests (`src/cli.rs`):**
  - `test_cli_default_no_args`: `Cli::try_parse_from(["app"])` succeeds with `config == None`.
  - `test_cli_short_config_flag`: `Cli::try_parse_from(["app", "-c", "config.yaml"])` succeeds with `config == Some("config.yaml".to_string())`.
  - `test_cli_long_config_flag`: `Cli::try_parse_from(["app", "--config", "custom.yaml"])` succeeds with `config == Some("custom.yaml".to_string())`.
  - `test_cli_unknown_flag_fails`: `Cli::try_parse_from(["app", "--unknown-flag"])` returns an error.
