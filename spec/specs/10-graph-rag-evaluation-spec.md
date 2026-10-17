# Spec 10: Graph RAG - Baseline Evaluation & Benchmarking

**Status:** `draft`
**Derived From:** [Graph RAG PRD](../prds/graph-rag-prd.md)

## Objective
Ensure the Graph RAG pipeline can be empirically tested for accuracy, performance, and regression through automated test harnesses and baseline data.

## Requirements

### 1. Curated Dataset
- The system must include a curated subset of interconnected markdown documents stored within the repository (e.g., `tests/data/graph_rag_baseline/`).
- This dataset should be derived from complex narrative wikis or established multi-hop reasoning datasets (e.g., HotpotQA style) to adequately test graph extraction and traversal.

### 2. Automated Test Harness (`make test-robot` + Ragas)
- Integration tests must be implemented as Robot Framework suites (`integration-tests/tests/*.robot`, executed via `make test-robot` / `./integration-tests/run-tests-local.sh` against the already-running Graphrag service) using this baseline dataset and a custom **Ragas** keyword library (`integration-tests/lib/RagasEvalLibrary.py`) to validate the entire Graph RAG pipeline.
- **Validation Criteria:**
  - **Extraction Quality:** Prove that loading the `.md` files via the API yields a consistent and expected graph structure.
  - **Query Accuracy & Ragas AI Response Grading:** Issue questions against the Graphrag query API and evaluate responses using **Ragas** metrics (`Faithfulness`, `FactualCorrectness`, `ResponseRelevancy`, `LLMContextRecall`) against the baseline ground truth and retrieved graph context.
  - **Regression Prevention:** Ensure that modifications to chunking logic, prompts, or the LLM model do not regress the quality of the extracted knowledge graph or query results.

## Testing Strategy
- **End-to-End Evaluation Tests (`make test-robot`):**
  - Load the baseline `.md` dataset into the running Graphrag server via the Robot Framework test runner (`make test-robot`).
  - Perform automated assertions against the resulting graph structure (node counts, edge types).
  - Execute a suite of single-hop and multi-hop questions, grading the AI responses with Ragas keywords (`RagasEvalLibrary.py`) against expected ground-truth facts and thresholds, and clean up created test data in `[Teardown]`.



