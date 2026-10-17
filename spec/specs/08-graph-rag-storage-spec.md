# Spec 08: Graph RAG - Storage & Graph Construction

**Status:** `draft`
**Derived From:** [Graph RAG PRD](../prds/graph-rag-prd.md)

## Objective
Persist extracted entities supporting both semantic similarity search and structural graph traversal utilizing PostgreSQL.

## Requirements

### 1. Storage Backend
- The system must utilize PostgreSQL as the unified storage layer for all relational, vector, and graph data.

### 2. Vector Storage (pgvector)
- Textual descriptions of Knowledge Objects (and future community summaries) must be embedded and stored in standard Postgres tables.
- The system must use the `pgvector` extension to enable semantic similarity search on these embeddings.

### 3. Graph Engine (Apache AGE)
- The system must leverage the **Apache AGE** extension for Postgres to store Nodes (Knowledge Objects) and Edges (Graph Tuples).
- The schema design must support OpenCypher queries to traverse the graph (e.g., multi-hop relationships) directly within the relational database.

### 4. Data Synchronization & Consistency
- The system must maintain consistency between the standard relational/vector tables and the AGE graph structures.
- A unified schema or reliable synchronization mechanism must be implemented to link vector embeddings to their corresponding graph nodes.

## Testing Strategy
- **Integration Tests (`make test-robot`):**
  - Execute Robot Framework integration tests (`make test-robot` / `./integration-tests/run-tests-local.sh`) against the already-running Graphrag backend and PostgreSQL (`pgvector` + `Apache AGE`).
  - Load `.md` files and insert Knowledge Objects and Graph Tuples via the backend API.
  - Verify that `pgvector` semantic queries return expected nearest neighbors.
  - Execute Cypher traversal queries via the API to validate structural integrity, and clean up created graph/vector data in `[Teardown]`.


