-- Graph RAG storage schema for documents, knowledge objects (pgvector), and graph tuples
CREATE EXTENSION IF NOT EXISTS vector;
CREATE TABLE IF NOT EXISTS graph_documents (
    id VARCHAR(255) PRIMARY KEY,
    filename VARCHAR(255) NOT NULL,
    content TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS knowledge_objects (
    id VARCHAR(255) PRIMARY KEY,
    label VARCHAR(128) NOT NULL,
    description TEXT NOT NULL,
    layer INT NOT NULL DEFAULT 0,
    document_id VARCHAR(255),
    embedding vector(64) NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS graph_tuples (
    source_id VARCHAR(255) NOT NULL,
    predicate VARCHAR(128) NOT NULL,
    target_id VARCHAR(255) NOT NULL,
    document_id VARCHAR(255),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (source_id, predicate, target_id)
);

CREATE INDEX IF NOT EXISTS idx_graph_tuples_source ON graph_tuples(source_id);
CREATE INDEX IF NOT EXISTS idx_graph_tuples_target ON graph_tuples(target_id);
CREATE INDEX IF NOT EXISTS idx_knowledge_objects_layer ON knowledge_objects(layer);
