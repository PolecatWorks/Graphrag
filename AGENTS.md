# Agent Workspace Guidance & Specification Rules

> [!IMPORTANT]
> **Core Workflow Rule**: **Update PRDs First** (`spec/prds/`) $\rightarrow$ **Derive Spec** (`spec/specs/`) $\rightarrow$ **Build Out** (Strict TDD Implementation).
> Never jump directly into source code implementation without completing and validating the PRD and Spec phases first.

## Communication & Diagramming Standards
- **Relative Markdown Links**: Use relative Markdown links for file cross-references (e.g. `[agent-schema-spec.md](./spec/specs/agent-schema-spec.md)` or `[Master PRD](./spec/prds/agent-as-data-prd.md)`). Do NOT use `file:///` URLs inside Markdown files as IDE previewers cannot click them.
- **Mermaid Diagrams**: When describing domain concepts, system architectures, data flows, sequence flows, or process lifecycles, **always use Mermaid diagrams** (`mermaid` code blocks).

## Input Routing & Classification Rules
- **Process & Workflow Inputs**: When the user provides feedback or instructions regarding *how we work*, development workflows, file locations, or process rules, immediately capture and document them in **[AGENTS.md](./AGENTS.md)**.
- **Research Inputs**: When the user requests exploratory research, technology evaluations, or background analysis, document the findings in **[spec/research/](./spec/research/)**. Use these research documents to inform and synthesize content into PRDs.
- **Product, Concept, & Feature Inputs**: When the user provides feedback or instructions regarding *what the system does*, new concepts, data models, or product capabilities, guide and capture those inputs into **[spec/prds/](./spec/prds/)**.

---

## Product & Task Workflow Rules

```mermaid
flowchart TD
    A[User Input / Request] --> B{Input Classification?}
    B -->|Process / Workflow Rule| C[Update AGENTS.md]
    B -->|Exploratory Research| D[Build Research Notes in spec/research/]
    B -->|Product / Feature / Concept| E[Review & Update PRDs in spec/prds/]
    D -->|Synthesize & Formalize| E
    E --> F{PRD Too Long or Multi-Concept?}
    F -->|Yes| G[Refactor into Separate Focused PRDs]
    F -->|No| H[Consistency & Detail Validation]
    G --> H
    H --> I[Update spec/prds/README.md]
    I --> J[Prompt User for Confirmation to Build Specs]
    J -->|User Approves| K[Create TDD Specs in spec/specs/]
    K --> L[Define Integration & Unit Test Strategies]
    L --> M[Update spec/specs/README.md]
    M --> N[Determine Execution Order & Run TDD Cycle]
```

### Research Phase (`spec/research/`)
1. **Build Research Base**: When research is requested, store technical evaluations, trade-offs, and exploratory findings in `spec/research/`.
2. **Indexing**: Update [spec/research/README.md](./spec/research/README.md) using relative links whenever a research document is created or updated.
3. **Synthesis**: Review research findings with the user and use them to construct or refine persistent PRDs in `spec/prds/`.

### Phase 1: Feature Evaluation & PRD Management (`spec/prds/`)
1. **Review Existing PRDs**: When a new feature or capability is requested, first review existing PRDs in `spec/prds/` to determine if it fits within an existing PRD or warrants a new one.
2. **Modular PRD Structure**: If a PRD becomes too long or begins covering multiple distinct domain concepts, refactor it into separate, focused PRDs.
3. **Consistency & Detail Check**: Verify that new PRD requirements contain sufficient detail and do not contradict existing features or architectural objectives.
4. **Pattern & Journey Sync**: Whenever implementation updates alter design patterns, database constraints, or end-to-end user journeys, immediately sync and update the corresponding PRD files in `spec/prds/`.
5. **User Confirmation**: Once PRDs are updated/created and validated, **explicitly ask the user** if they wish to proceed to creating build spec files (`spec/specs/`).

### Phase 2: Specification & TDD Test Planning (`spec/specs/`)
1. **Drive TDD Execution**: Spec files in `spec/specs/` are derived from PRDs to serve as definitions for a Test-Driven Development (TDD) build process.
2. **Spec Status Lifecycle (`draft` -> `complete` -> `deprecated`)**:
   - **`draft`**: Spec is actively being authored or refined prior to implementation. Modifications are allowed **only** while in `draft` status.
   - **`complete`**: Marked once the spec features and tests are fully implemented. **A complete spec cannot be modified** except to transition its status to `deprecated`.
   - **`deprecated`**: Marked when a historical spec is superseded by a newer spec. Preserved for auditability.
3. **Clear Test Strategy**: Every spec file must clearly define:
   - Expected operational outcomes and payload contracts.
   - Comprehensive test strategy (prioritizing integration tests alongside unit tests).
4. **Preservation**: Spec files are preserved in `spec/specs/` to maintain historical auditability of what was built and why design choices evolved.
5. **Indexing & Linking**: Always update `spec/prds/README.md` and `spec/specs/README.md` using relative Markdown links whenever PRDs or specs are added or modified.


### Phase 3: Ordered Build & Implementation
1. **Dependency Sequencing**: Determine the appropriate build order across all spec files (`01` through `08`).
2. **Feature Branch & PR Workflow**: **Do NOT push directly to the `main` branch**. Always create a dedicated feature branch (e.g. `feat/reversible-migrations`), commit changes to the feature branch, push the feature branch to `origin`, and open a Pull Request (PR) using `gh pr create`.
3. **Strict TDD Execution**: Build out code using a strict TDD approach (Write failing integration/unit test -> Implement code -> Verify test passes -> Refactor).
4. **Per-Spec Verification Gate**: After completing implementation of a spec file, run **all unit tests** and **Robot Framework integration tests** (`/integration-tests/run-tests-local.sh`). Confirm clean pass before transitioning spec status to `complete` and progressing to the next spec file.
5. **Pull Request Validation Checks**: Every PR must pass all required validation status checks (Backend Build & Test, Frontend Build & Test, Helm Chart Lint). These validation checks run in parallel and are conditionally run based on the files changed in the PR to ensure efficiency and safety. Any skipped checks are treated as passing by GitHub branch protection settings.

---

## Dev Server Behaviour

Both dev servers have **watch/hot-reload enabled** — do **NOT** manually kill or restart them after making code changes.

| Server | Make Target | Watch Mechanism |
|---|---|---|
| Backend (Rust) | `make aad-be-watch` | `cargo watch` — recompiles & restarts on file save |
| Frontend (Angular) | `make aad-fe-dev` | `ng serve` with HMR — hot-reloads on file save |

> **Rule**: After editing source files, simply save and let the watch process handle recompilation. Never run `kill`, `lsof`, or restart commands against the dev servers unless explicitly instructed by the user.

## Database Migrations & Watch Loops

- **Manual Intervention Danger**: Never manually alter database schemas (e.g. `ALTER TABLE` via `psql`) if there is an active `sqlx` migration (`.up.sql`) pending. This permanently desyncs the `_sqlx_migrations` table and causes the `make aad-be-watch` process to enter an unrecoverable crash loop. Always let the backend startup automatically execute `.up.sql` migrations sequentially.
- **Strict Entity Dependencies**: Be extremely cautious when enforcing strict non-null database properties (e.g., changing `owner_id` from `Option<Uuid>` to `Uuid`). If the database contains existing rows with `NULL` values, the backend API will successfully return a 500 parsing error, silently breaking frontend components (e.g. Angular dropdowns/filters going empty). Always backfill existing rows before enforcing strict schemas on production tables.

## Configuration Management & Loading Standards

- **CLI Argument Parsing (`clap` Derive)**: All command-line argument parsing MUST use `clap` with the `derive` feature (`#[derive(clap::Parser)]`, `#[derive(clap::Subcommand)]`, `#[derive(clap::Args)]`) in `src/cli.rs`. Manual `std::env::args` inspection or imperative `clap::Command` builder chains are prohibited.
- **Zero Direct Runtime Environment Variables**: Never read environment variables directly at runtime in handler, execution, or business logic code (e.g. via `std::env::var`, `std::env::var_os`, `process.env`, or similar).
- **Centralized Configuration Loader**: ALL configuration MUST be defined in configuration structs, provided via the centralized configuration loader (`AppConfig` / YAML configuration files loaded at startup with environment variable override prefixes like `AAD_BE__`), and validated fail-fast during startup.
- **Modular Config Ownership & Delegated Validation**: Each domain module (e.g., `db`, `llm_tools`) must define and own its domain-specific configuration struct (`DatabaseConfig`, `LlmConfig`), its `.validate()` implementation, and the comprehensive unit tests for its validation rules. `AppConfig` (`src/config.rs`) composes these domain configs, delegates validation to each module (`self.database.validate()?`, `self.llm.validate()?`), and tests top-level config loading, overrides, and delegation.
- **No Hardcoded Defaults in Runtime Code**: Using fallback defaults in runtime code (e.g. `.unwrap_or(...)`, `.unwrap_or_else(...)`, or hardcoded URLs/ports/timeouts) is strictly prohibited. If a configuration value is required for operation, it must be declared in the configuration schema, populated in configuration files, and verified in `AppConfig::validate()`.
- **Fail-Fast Validation**: Any missing, empty, or malformed configuration must fail immediately at service startup before opening database pools, binding listeners, or starting background workers.

## Error Handling Standards (`thiserror`)

- **No Stringly-Typed Errors (`Result<_, String>`)**: Returning `Result<_, String>` from module functions, validation methods, or initialization routines is prohibited.
- **Modular Typed Error Enums**: Each domain module (`config`, `db`, `llm_tools`, `hams_tools`, etc.) must define its own typed error enum deriving `thiserror::Error` (e.g., `ConfigError`, `DatabaseError`, `LlmError`, `HamsError`).
- **Transparent Delegation**: Top-level error types (such as `ConfigError`) must wrap domain errors using `#[error(transparent)]` and `#[from]` so callers can both propagate errors ergonomically with `?` and inspect specific typed variants in tests and error handlers.
