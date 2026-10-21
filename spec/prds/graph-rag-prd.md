# Graph RAG Product Requirements Document (PRD)

## Overview
This PRD outlines the architecture and requirements for the **Graph Retrieval-Augmented Generation (Graph RAG)** capability within the `agent-as-data` platform. The goal is to move beyond standard semantic search by ingesting markdown documents, extracting structured knowledge, and building a traversable knowledge graph. This enables the system to answer complex, multi-hop queries that require synthesizing information across multiple interconnected concepts and documents.

## High-Level Pipeline Architecture

The Graph RAG pipeline consists of three primary phases: Ingestion & Extraction, Storage & Graph Construction, and Querying.

```mermaid
flowchart TD
    subgraph "Phase 1: Ingestion & Extraction"
        MD[Markdown Documents]
        Chunker[Document Splitter]
        LLMExtract[LLM Extraction\n(rig-core)]
    end

    subgraph "Phase 2: Storage & Graph Construction"
        RelDB[(PostgreSQL\nRelational Data)]
        AGE[(Apache AGE\nGraph Engine)]
        VecDB[(pgvector\nEmbeddings)]
    end

    subgraph "Phase 3: Hierarchical Summarization (Layers)"
        GraphClustering[Graph Clustering\n(Communities)]
        LLMSummarize[LLM Summarization\n(Community Nodes)]
    end

    subgraph "Phase 4: Querying"
        UserQuery[User Request]
        QueryAnalyzer[Query Router / Analyzer]
        Cypher[Cypher Graph Traversal]
        VectorSearch[Semantic Vector Search]
        Synthesizer[LLM Synthesizer]
    end

    MD --> Chunker
    Chunker -->|Text Chunks| LLMExtract
    LLMExtract -->|Structured Output:\nNodes (Knowledge Objects)\nEdges (Graph Tuples)| RelDB
    RelDB -.-> AGE
    RelDB -.-> VecDB

    AGE --> GraphClustering
    GraphClustering --> LLMSummarize
    LLMSummarize -->|Summary Nodes & Hierarchy Edges| AGE
    LLMSummarize -->|Summary Embeddings| VecDB

    UserQuery --> QueryAnalyzer
    QueryAnalyzer --> VectorSearch
    QueryAnalyzer --> Cypher
    VectorSearch --> Synthesizer
    Cypher --> Synthesizer
    Synthesizer -->|Final Answer| UserQuery
```

## Core Components & Requirements

### 1. Ingestion & LLM Extraction (Knowledge Objects & Tuples)
**Objective:** Parse raw markdown text into structured graph entities.
- **Input:** Raw markdown files containing thematic or narrative data.
- **Chunking:** The system must split markdown files into manageable semantic chunks (e.g., by headers, paragraphs, or fixed token length) before sending to the LLM.
- **LLM Prompting (rig-core):**
  - We use the `rig-core` crate to prompt an LLM to extract structured data from each chunk.
  - **Knowledge Objects (Nodes):** The LLM must identify key entities, concepts, or descriptive units of knowledge from the text. Each object must have a unique identifier, a label/type, and a coherent textual description.
  - **Graph Tuples (Edges):** The LLM must extract relationships between these objects in the form of tuples (Source Node, Relationship Type/Predicate, Target Node).
- **Idempotency:** Re-ingesting the same document should update or merge nodes/edges rather than creating duplicates.

### 2. Graph Storage (Apache AGE + pgvector)
**Objective:** Persist the extracted entities in a way that supports both semantic similarity search and structural graph traversal.
- **Storage Backend:** We utilize PostgreSQL as the unified storage layer.
- **Vector Storage:** The textual descriptions of Knowledge Objects (and later, community summaries) are embedded and stored in standard Postgres tables using the `pgvector` extension for semantic search.
- **Graph Engine:** We leverage the **Apache AGE** extension for Postgres to store the Nodes and Edges. This allows us to use OpenCypher queries to traverse the graph (e.g., finding all concepts connected to "Entity X" within 2 hops) directly within our relational database.
- **Data Synchronization:** The system must maintain consistency between the standard relational/vector tables and the AGE graph structures (if stored separately) or utilize a unified schema supported by AGE.

### 3. Hierarchical Knowledge (Layers & Communities)
**Objective:** Enable the system to answer broad, global queries ("What are the main themes?") by summarizing dense local graphs.
- **Graph Clustering:** The system must implement (or utilize an external library for) community detection algorithms (e.g., Leiden or Louvain) to group heavily interconnected Knowledge Objects into clusters (Communities).
- **Hierarchical Summarization:**
  - **Layer 0:** The base layer of extracted Knowledge Objects and Tuples.
  - **Layer 1..N:** For each identified community, an LLM is prompted to write a comprehensive summary of that community based on its constituent nodes and edges.
  - These summaries become new "Summary Nodes" in the graph, hierarchically linked to the lower-level nodes they summarize.
- **Embeddings:** Summary Nodes are also embedded via `pgvector`, allowing high-level semantic search across broad topics.
- *Note: This hierarchical component is documented here as a core capability but will be broken out into a separate technical specification during the implementation phase.*

### 4. Baseline Evaluation Data & Integration Testing (`make test-robot` + Ragas)
**Objective:** Ensure the Graph RAG pipeline can be empirically tested for accuracy and regression via automated Robot Framework integration suites (`make test-robot`) and **Ragas** RAG evaluation metrics.
- **Dataset:** Curated interconnected markdown documents stored in `tests/data/graph_rag_baseline/` (`01_apollo_program.md` through `05_john_f_kennedy.md`).
- **Integration Test Harness (`make test-robot`):** Executed via `make test-robot` (`./integration-tests/run-tests-local.sh` running Robot Framework `.robot` suites under `integration-tests/tests/`) against an already-running Graphrag service:
  1. **Load Markdown Files:** Upload/ingest the `.md` files from `tests/data/graph_rag_baseline/` via the running Graphrag ingestion API.
  2. **Graph & Vector Verification:** Query the Graphrag APIs to verify that Knowledge Objects (nodes) and Graph Tuples (edges) were extracted and stored in Apache AGE and `pgvector`.
  3. **AI Question & Response Verification via Ragas (`RagasEvalLibrary.py`):** Issue single-hop and multi-hop natural-language questions against the Graphrag query API and evaluate the returned AI answer and retrieved graph context using **Ragas** (`Faithfulness`, `FactualCorrectness`, `ResponseRelevancy`, and `LLMContextRecall`) against ground-truth reference answers derived from the `.md` files.
  4. **Teardown:** Clean up all ingested test artifacts during `[Teardown]` so test runs remain repeatable and isolated.

### 5. Data Element Management (Full BREAD REST API)
**Objective:** Expose full **Browse, Read, Edit, Add, Delete (BREAD)** HTTP endpoints for managing all Graph RAG data elements directly via the API:
- **Documents (`/documents`):** Browse, Read, Edit (re-chunk & re-extract), Add, and Delete source Markdown documents.
- **Knowledge Objects / Nodes (`/nodes`):** Browse (with label, layer, keyword, and semantic vector filters), Read (with incident edges), Edit (with automatic `pgvector` re-embedding), Add, and Delete (with cascading edge cleanup).
- **Graph Tuples / Edges (`/edges`):** Browse, Read, Edit, Add, and Delete directed `(source, predicate, target)` relationships.
- **Communities (`/communities`):** Browse, Read, Edit, Rebuild (`POST /communities/rebuild`), and Delete Layer 1+ hierarchical community summaries.
- See the dedicated **[Graph RAG Data Management (BREAD API) PRD](./graph-rag-api-bread-prd.md)** for full resource schemas, status codes, and cascading synchronization rules.

### 6. Model Context Protocol (MCP) Service Exposure
**Objective:** Expose GraphRAG-RS pipeline and BREAD capabilities to AI agents, IDEs, and desktop assistants via the **Model Context Protocol (MCP)** over HTTP JSON-RPC 2.0 (`POST /mcp` and `POST /api/v1/mcp`), following the patterns established in `aad-mcp-container`:
- **`rmcp` + `schemars` Tool Router:** Auto-generated Draft-07 JSON Schemas via `#[tool_router]` and `#[tool]` macros.
- **Baseline & Pipeline Tools:** `hello`, `graphrag_ingest`, and `graphrag_query`.
- **Full BREAD MCP Tools:** `document_*`, `node_*`, `edge_*`, and `community_*` tools operating directly against the in-process `GraphStore`.
- See the dedicated **[Graph RAG MCP Service PRD](./graph-rag-mcp-prd.md)** for full protocol lifecycle diagrams, tool schemas, and error contracts.

## Future Considerations
- Implementing a specialized Query Router LLM that decides whether an incoming user query requires a local search (Layer 0 nodes), a global search (Community Summaries), or a hybrid approach.


