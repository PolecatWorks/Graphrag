-- Autonomous Semantic Ontology metadata columns for knowledge_objects and graph_tuples
ALTER TABLE knowledge_objects
    ADD COLUMN IF NOT EXISTS epistemic_status VARCHAR(64) NOT NULL DEFAULT 'verified',
    ADD COLUMN IF NOT EXISTS confidence FLOAT8 NOT NULL DEFAULT 1.0,
    ADD COLUMN IF NOT EXISTS verified_by VARCHAR(255),
    ADD COLUMN IF NOT EXISTS properties JSONB NOT NULL DEFAULT '{}'::jsonb;

ALTER TABLE graph_tuples
    ADD COLUMN IF NOT EXISTS epistemic_status VARCHAR(64) NOT NULL DEFAULT 'verified',
    ADD COLUMN IF NOT EXISTS confidence FLOAT8 NOT NULL DEFAULT 1.0,
    ADD COLUMN IF NOT EXISTS verified_by VARCHAR(255),
    ADD COLUMN IF NOT EXISTS properties JSONB NOT NULL DEFAULT '{}'::jsonb;

CREATE INDEX IF NOT EXISTS idx_knowledge_objects_epistemic ON knowledge_objects(epistemic_status);
CREATE INDEX IF NOT EXISTS idx_graph_tuples_epistemic ON graph_tuples(epistemic_status);
