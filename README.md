# GraphRAG-RS

[![Rust](https://img.shields.io/badge/rust-1.80%2B-blue.svg)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Build Status](https://github.com/your-org/graphrag-rs/actions/workflows/rust.yml/badge.svg)](https://github.com/your-org/graphrag-rs/actions)

**GraphRAG-RS** is a high-performance Rust microservice that ingests documents and generates a PostgreSQL graph database, enabling advanced Retrieval-Augmented Generation (RAG) using semantic search and graph-based relationships.

## Table of Contents

- [Architecture Overview](#architecture-overview)
- [Quickstart & Local Setup](#quickstart--local-setup)
- [Configuration](#configuration)
- [Testing](#testing)
- [Documentation & Guidelines](#documentation--guidelines)
- [Contributing](#contributing)

---

## Architecture Overview

This project is built as a highly concurrent Rust microservice with a focus on reliability and advanced LLM integrations.

- **Web Framework:** Built on [Axum](https://github.com/tokio-rs/axum) with explicitly configured Tokio runtimes.
- **Database Layer:** Utilizes PostgreSQL powered by `sqlx`. It heavily relies on **pgvector** for semantic similarity search and the **Apache AGE** extension for storing structural graph data and supporting OpenCypher queries.
- **LLM Integration:** Leverages the [`rig-core`](https://github.com/0xPlaygrounds/rig) crate to interface seamlessly with LLM providers (including OpenAI and local models via Ollama) for text extraction and analysis.
- **Configuration:** Employs [`figment`](https://github.com/SergioBenitez/Figment) for robust, layered configuration management.
- **Health Monitoring:** Runs a dedicated Health Monitoring Sidecar (HaMS) on a separate port for liveness, readiness, and metrics.

## Quickstart & Local Setup

The core developer workflow is standardized around the `Makefile`. Before starting, ensure you have Rust, Cargo, and a local PostgreSQL instance (with pgvector and Apache AGE) running.

### 1. Database Setup & Migrations
First, ensure your database is running, and then apply all migrations:
```bash
make migrate
```
*This command will install `sqlx-cli` if it's missing, create the database, and run all pending migrations.*

### 2. Running the Development Server
To start the backend server on the standard ports (8080 for Axum, 8079 for HaMS):
```bash
make dev
```
*Note: The `make dev` target automatically attempts to kill any stale processes holding ports 8080 or 8079 to prevent bind errors.*

### 3. Hot-Reloading
For rapid development, you can run the server with hot-reloading via `cargo watch`:
```bash
make watch
```
*(Requires `cargo-watch` installed: `cargo install cargo-watch`)*

## Configuration

The application uses `figment` for configuration management. Configuration is typically loaded from environment variables and configuration files (e.g., `AppConfig`). The Tokio runtime threads, stack size, and metrics interval are initialized centrally using these parameters rather than relying on default macros.

Ensure the following essential environment variables are set in your local environment or `.env` file (e.g., `DATABASE_URL`).

## Testing

We focus heavily on rigorous automated testing in Rust.

Run all unit and integration tests with:
```bash
make test
```
*This will execute `cargo test` sequentially (`--test-threads=1`) and trigger local integration scripts if present.*

## Documentation & Guidelines

This repository follows a strict documentation and development lifecycle.

- **Product Requirements (PRDs):** See `spec/prds/` for high-level feature requirements and suite descriptions.
- **Agent Instructions:** Refer to `AGENTS.md` for coding conventions, architectural guidelines, and programmatic checks. **If you are an AI assistant or human contributor, you must obey the instructions in `AGENTS.md`.**
- **Domain Modeling:** We use Mermaid diagrams in markdown to describe domain concepts and system architecture.

## Contributing

We strictly adhere to a **Test-Driven Development (TDD)** implementation cycle.

1. **Review PRDs:** Ensure any new feature is covered by a PRD in `spec/prds/`.
2. **Derive Specs:** Technical specifications must be created from PRDs before coding begins.
3. **Branch & Build:** Create a feature branch.
4. **Pull Request:** Once implementation (with passing tests) is complete, commit your code, push to the remote branch, and create a Pull Request via GitHub.
