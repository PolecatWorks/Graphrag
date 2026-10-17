# Spec 07: Graph RAG - Ingestion & LLM Extraction

**Status:** `draft`
**Derived From:** [Graph RAG PRD](../prds/graph-rag-prd.md)

## Objective
Parse raw markdown text into structured graph entities (Knowledge Objects and Graph Tuples) using an LLM.

## Requirements

### 1. Document Chunking
- **Input:** Raw markdown files containing thematic or narrative data.
- **Process:** The system must implement a chunking mechanism to split markdown files into manageable semantic chunks before sending them to the LLM.
- **Strategy:** Chunking should support splitting by headers, paragraphs, or fixed token length to maintain semantic coherence.

### 2. LLM Extraction (rig-core)
- **Integration:** The system must utilize the `rig-core` crate to prompt an LLM for structured data extraction from each chunk.
- **Knowledge Objects (Nodes):**
  - The LLM must identify key entities, concepts, or descriptive units of knowledge.
  - Each object must include:
    - A unique identifier.
    - A label/type.
    - A coherent textual description.
- **Graph Tuples (Edges):**
  - The LLM must extract relationships between identified objects.
  - Tuples must be structured as: `(Source Node, Relationship Type/Predicate, Target Node)`.

### 3. Idempotency
- Re-ingesting the same markdown document should update or merge existing nodes and edges rather than creating duplicates.
- The system must ensure consistent unique identifiers are generated or resolved for entities to facilitate this.

## Testing Strategy
- **Unit Tests:**
  - Verify the document chunking logic correctly splits sample markdown texts according to the configured strategy.
  - Mock the `rig-core` LLM response to validate that the parsing logic correctly structures the returned Knowledge Objects and Tuples.
- **Integration Tests (`make test-robot`):**
  - Execute a Robot Framework suite (`integration-tests/tests/*.robot` via `make test-robot`) against the running Graphrag service that loads a sample markdown file via the backend API, ensuring idempotency on repeated runs and cleaning up created entities in `[Teardown]`.


