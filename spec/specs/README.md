# Specifications (Specs)

This directory contains the detailed technical specifications and Test-Driven Development (TDD) contracts derived from the Product Requirements Documents (PRDs).

## Spec Lifecycle

*   **`draft`**: Actively being authored or refined prior to implementation.
*   **`complete`**: Marked once the spec features and tests are fully implemented. Cannot be modified.
*   **`deprecated`**: Marked when a historical spec is superseded.

## Index of Specifications

*   [01. Developer Tooling (Makefile)](./01-developer-tooling-spec.md) - `deprecated` (Superseded by Spec 15)
*   [02. Configuration Loader](./02-config-loader-spec.md) - `complete`
*   [03. Database & Storage Layer](./03-database-layer-spec.md) - `complete`
*   [04. Axum Web Service](./04-axum-service-spec.md) - `complete`
*   [05. Health Monitoring Sidecar (HaMS)](./05-hams-sidecar-spec.md) - `complete`
*   [06. LLM Tools Integration](./06-llm-tools-spec.md) - `complete`
*   [07. Graph RAG - Ingestion & LLM Extraction](./07-graph-rag-ingestion-extraction-spec.md) - `complete`
*   [08. Graph RAG - Storage & Graph Construction](./08-graph-rag-storage-spec.md) - `complete`
*   [09. Graph RAG - Hierarchical Summarization](./09-graph-rag-hierarchical-spec.md) - `complete`
*   [10. Graph RAG - Baseline Evaluation & Benchmarking](./10-graph-rag-evaluation-spec.md) - `complete`
*   [11. Modular Configuration Ownership & Delegated Validation](./11-modular-config-validation-spec.md) - `complete`
*   [12. Typed Error Handling (`thiserror`)](./12-typed-error-handling-spec.md) - `complete`
*   [13. CLI Argument Parsing (`clap` Derive)](./13-cli-clap-derive-spec.md) - `complete`
*   [14. Robot Framework & Ragas Integration Test Harness (`make test-robot`)](./14-robot-ragas-integration-harness-spec.md) - `complete`
*   [15. Developer Tooling Configuration & Clap Arguments (Makefile)](./15-makefile-clap-config-spec.md) - `complete`
*   [16. Graph RAG Data Management (BREAD API)](./16-graph-rag-bread-api-spec.md) - `complete`
*   [17. Graph RAG Model Context Protocol (MCP) Service](./17-graph-rag-mcp-service-spec.md) - `complete`
*   [18. CI/CD Container Integration](./18-ci-cd-container-integration-spec.md) - `complete`
*   [19. API & MCP Tracing, Observability & Configurable Logging](./19-api-mcp-tracing-observability-spec.md) - `complete`
*   [20. Configurable API Prefix & Istio VirtualService Composition](./20-configurable-api-prefix-and-virtualservice-spec.md) - `complete`
*   [21. File-Based Database Secrets & Config Resolution](./21-file-based-secrets-config-spec.md) - `complete`
*   [22. Autonomous Ontology Schema, Epistemic Lifecycle & Dataset Sync (`POST /ontology/sync-dataset`)](./22-ontology-schema-and-dataset-sync-spec.md) - `complete`
*   [23. Multi-Hop Steiner Join-Path Planner, `FORBIDDEN_JOIN` Blocking & Fan-Out Detector (`POST /ontology/join-path`)](./23-ontology-join-path-planner-spec.md) - `complete`
*   [24. Epistemic Statistical Traits, Known Confounders & Join-Path Statistical Caveats](./24-epistemic-statistical-traits-and-confounders-spec.md) - `complete`
*   [25. Bounded Ontology Subgraph Export for Topology Visualization (`GET /ontology/subgraph` & MCP `graphrag_export_subgraph`)](./25-ontology-subgraph-export-spec.md) - `complete`
