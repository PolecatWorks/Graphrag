# Spec 09: Graph RAG - Hierarchical Summarization

**Status:** `draft`
**Derived From:** [Graph RAG PRD](../prds/graph-rag-prd.md)

## Objective
Enable the system to answer broad, global queries by summarizing dense local graphs into hierarchical community structures.

## Requirements

### 1. Graph Clustering (Communities)
- The system must implement or integrate an external library for community detection algorithms (e.g., Leiden or Louvain).
- The algorithm must process the base graph (Layer 0) and group heavily interconnected Knowledge Objects into clusters known as Communities.

### 2. Hierarchical Summarization
- **Layering System:**
  - **Layer 0:** Represents the base layer of extracted Knowledge Objects and Tuples.
  - **Layer 1..N:** Represents hierarchical layers of community summaries.
- **LLM Summarization:**
  - For each identified community, the system must prompt an LLM to generate a comprehensive summary based on its constituent nodes and edges.
  - These generated summaries must be inserted as new "Summary Nodes" into the graph.
  - Summary Nodes must be hierarchically linked (via edges) to the lower-level nodes they summarize.

### 3. Embeddings for Summaries
- Summary Nodes must have their textual summaries embedded and stored via `pgvector`.
- This enables high-level semantic search across broad topics, facilitating global queries.

## Testing Strategy
- **Unit Tests:**
  - Provide a mock graph (Nodes and Edges) to the clustering algorithm and verify it correctly identifies expected communities.
  - Mock the LLM summarization response and verify that the system creates new Summary Nodes and correctly links them to the underlying community nodes.
- **Integration Tests (`make test-robot`):**
  - Execute a Robot Framework suite (`make test-robot`) against the running Graphrag API that triggers hierarchical community summarization over ingested `.md` nodes and verifies that Summary Nodes and their `pgvector` embeddings are queryable via the API.


