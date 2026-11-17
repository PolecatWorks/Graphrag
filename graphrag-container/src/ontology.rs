use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};

use rmcp::schemars;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::PgPool;
use tracing::warn;

use crate::graph_rag::{
    GraphRagError, GraphStore, GraphTuple, KnowledgeObject, compute_embedding_64d,
    normalize_entity_id,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
pub struct OntologyElementMeta {
    pub epistemic_status: String,
    pub confidence: f64,
    pub verified_by: Option<String>,
    pub properties: Value,
}

impl Default for OntologyElementMeta {
    fn default() -> Self {
        Self {
            epistemic_status: "verified".to_string(),
            confidence: 1.0,
            verified_by: None,
            properties: json!({}),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
pub struct OntologyFieldInput {
    pub field_name: String,
    pub arrow_data_type: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub null_ratio: f64,
    #[serde(default)]
    pub uniqueness_ratio: f64,
    #[serde(default)]
    pub inferred_semantic_domain: String,
    #[serde(default = "default_discovered")]
    pub epistemic_status: String,
}

fn default_discovered() -> String {
    "discovered".to_string()
}

fn default_one() -> f64 {
    1.0
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
pub struct OntologyJoinEdgeInput {
    pub left_dataset: String,
    pub left_field: String,
    pub right_dataset: String,
    pub right_field: String,
    pub observed_cardinality: String,
    #[serde(default = "default_one")]
    pub inclusion_ratio: f64,
    #[serde(default = "default_one")]
    pub row_multiplication_factor: f64,
    #[serde(default)]
    pub fan_out_risk: bool,
    #[serde(default)]
    pub homonym_trap_warning: bool,
    #[serde(default = "default_one")]
    pub confidence: f64,
    #[serde(default = "default_discovered")]
    pub epistemic_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
pub struct OntologySyncDatasetRequest {
    pub dataset_name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub row_count: usize,
    #[serde(default)]
    pub schema_fingerprint_sha256: Option<String>,
    #[serde(default = "default_discovered")]
    pub epistemic_status: String,
    #[serde(default)]
    pub fields: Vec<OntologyFieldInput>,
    #[serde(default)]
    pub join_edges: Vec<OntologyJoinEdgeInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
pub struct OntologySyncDatasetResponse {
    pub dataset_node_id: String,
    pub fields_synced: usize,
    pub join_edges_synced: usize,
    pub skipped_forbidden_joins: usize,
    pub epistemic_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
pub struct StewardEdgeRequest {
    pub left_dataset: String,
    pub left_field: String,
    pub right_dataset: String,
    pub right_field: String,
    /// Either "verify" or "reject"
    pub action: String,
    pub steward_id: String,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
pub struct StewardEdgeResponse {
    pub source_field_id: String,
    pub predicate: String,
    pub target_field_id: String,
    pub epistemic_status: String,
    pub verified_by: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
pub struct JoinPathRequest {
    pub datasets_or_fields: Vec<String>,
    #[serde(default = "default_min_conf")]
    pub min_confidence: f64,
    #[serde(default = "default_include_unverified")]
    pub include_unverified: bool,
}

fn default_min_conf() -> f64 {
    0.50
}

fn default_include_unverified() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
pub struct PlannedJoinStep {
    pub left_dataset: String,
    pub left_field: String,
    pub right_dataset: String,
    pub right_field: String,
    pub cardinality: String,
    pub row_multiplication_factor: f64,
    pub confidence: f64,
    pub epistemic_status: String,
    pub verified_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
pub struct BlockedForbiddenJoin {
    pub left_field: String,
    pub right_field: String,
    pub verified_by: Option<String>,
    pub rejection_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
pub struct JoinPathResponse {
    pub resolved_datasets: Vec<String>,
    pub join_steps: Vec<PlannedJoinStep>,
    pub blocked_forbidden_joins: Vec<BlockedForbiddenJoin>,
    pub fan_out_warnings: Vec<String>,
    pub drifted_elements: Vec<String>,
    pub unverified_elements: Vec<String>,
    pub all_rules_human_verified: bool,
    pub min_confidence: f64,
    pub requires_steward_review: bool,
}

pub fn canonical_field_node_id(dataset: &str, field: &str) -> String {
    let ds = normalize_entity_id(dataset);
    let fl = normalize_entity_id(field);
    if fl.starts_with(&format!("{}_", ds)) {
        fl
    } else {
        format!("{}_{}", ds, fl)
    }
}

impl GraphStore {
    pub async fn sync_dataset_ontology(
        &self,
        pool: &PgPool,
        req: OntologySyncDatasetRequest,
    ) -> Result<OntologySyncDatasetResponse, GraphRagError> {
        let ds_id = normalize_entity_id(&req.dataset_name);
        if ds_id.is_empty() {
            return Err(GraphRagError::EmptyNodeId);
        }

        let ds_desc = req.description.unwrap_or_else(|| {
            format!(
                "Dataset '{}' ({} rows)",
                req.dataset_name, req.row_count
            )
        });

        let mut fields_synced = 0_usize;
        let mut join_edges_synced = 0_usize;
        let mut skipped_forbidden_joins = 0_usize;

        {
            let mut guard = self.memory.write().await;

            // Preserve human-verified description if already verified and not drifted
            let final_ds_desc = match guard.nodes.get(&ds_id) {
                Some(existing) => {
                    let existing_meta = guard.node_meta.get(&ds_id);
                    if existing_meta.map(|m| m.epistemic_status.as_str()) == Some("verified")
                        && req.epistemic_status != "drifted"
                    {
                        existing.description.clone()
                    } else {
                        ds_desc.clone()
                    }
                }
                None => ds_desc.clone(),
            };

            let ds_emb = compute_embedding_64d(&format!("{} Dataset {}", ds_id, final_ds_desc));
            guard.nodes.insert(
                ds_id.clone(),
                KnowledgeObject {
                    id: ds_id.clone(),
                    label: "Dataset".to_string(),
                    description: final_ds_desc,
                    layer: 0,
                    document_id: None,
                    embedding: ds_emb,
                },
            );
            guard.node_meta.insert(
                ds_id.clone(),
                OntologyElementMeta {
                    epistemic_status: req.epistemic_status.clone(),
                    confidence: if req.epistemic_status == "drifted" {
                        0.40
                    } else {
                        1.0
                    },
                    verified_by: None,
                    properties: json!({
                        "dataset_name": ds_id,
                        "row_count": req.row_count,
                        "schema_fingerprint_sha256": req.schema_fingerprint_sha256,
                    }),
                },
            );

            for field in &req.fields {
                let field_node_id = canonical_field_node_id(&ds_id, &field.field_name);
                let field_desc = field.description.clone().unwrap_or_else(|| {
                    format!(
                        "Field '{}.{}' (type: {}, domain: {})",
                        ds_id, field.field_name, field.arrow_data_type, field.inferred_semantic_domain
                    )
                });

                let emb = compute_embedding_64d(&format!("{} Field {}", field_node_id, field_desc));
                guard.nodes.insert(
                    field_node_id.clone(),
                    KnowledgeObject {
                        id: field_node_id.clone(),
                        label: "Field".to_string(),
                        description: field_desc,
                        layer: 0,
                        document_id: None,
                        embedding: emb,
                    },
                );
                guard.node_meta.insert(
                    field_node_id.clone(),
                    OntologyElementMeta {
                        epistemic_status: field.epistemic_status.clone(),
                        confidence: if field.epistemic_status == "drifted" {
                            0.30
                        } else {
                            0.95
                        },
                        verified_by: None,
                        properties: json!({
                            "dataset": ds_id,
                            "field_name": field.field_name,
                            "arrow_data_type": field.arrow_data_type,
                            "null_ratio": field.null_ratio,
                            "uniqueness_ratio": field.uniqueness_ratio,
                            "inferred_semantic_domain": field.inferred_semantic_domain,
                        }),
                    },
                );

                let has_field_edge = GraphTuple {
                    source: ds_id.clone(),
                    predicate: "HAS_FIELD".to_string(),
                    target: field_node_id.clone(),
                    document_id: None,
                };
                guard.edges.insert(has_field_edge);
                fields_synced += 1;
            }

            for join in &req.join_edges {
                let left_ds = normalize_entity_id(&join.left_dataset);
                let right_ds = normalize_entity_id(&join.right_dataset);
                let left_fid = canonical_field_node_id(&left_ds, &join.left_field);
                let right_fid = canonical_field_node_id(&right_ds, &join.right_field);

                // Check if a FORBIDDEN_JOIN already exists between left_fid and right_fid (in either direction)
                let is_forbidden = guard.edges.iter().any(|e| {
                    e.predicate == "FORBIDDEN_JOIN"
                        && ((e.source == left_fid && e.target == right_fid)
                            || (e.source == right_fid && e.target == left_fid))
                });
                if is_forbidden {
                    skipped_forbidden_joins += 1;
                    continue;
                }

                // Ensure endpoint nodes exist
                if !guard.nodes.contains_key(&right_ds) {
                    let emb = compute_embedding_64d(&format!("{} Dataset", right_ds));
                    guard.nodes.insert(
                        right_ds.clone(),
                        KnowledgeObject {
                            id: right_ds.clone(),
                            label: "Dataset".to_string(),
                            description: format!("Dataset '{}'", right_ds),
                            layer: 0,
                            document_id: None,
                            embedding: emb,
                        },
                    );
                }
                if !guard.nodes.contains_key(&right_fid) {
                    let emb = compute_embedding_64d(&format!("{} Field", right_fid));
                    guard.nodes.insert(
                        right_fid.clone(),
                        KnowledgeObject {
                            id: right_fid.clone(),
                            label: "Field".to_string(),
                            description: format!("Field '{}.{}'", right_ds, join.right_field),
                            layer: 0,
                            document_id: None,
                            embedding: emb,
                        },
                    );
                    guard.node_meta.insert(
                        right_fid.clone(),
                        OntologyElementMeta {
                            epistemic_status: "discovered".to_string(),
                            confidence: 0.90,
                            verified_by: None,
                            properties: json!({
                                "dataset": right_ds,
                                "field_name": join.right_field,
                            }),
                        },
                    );
                    guard.edges.insert(GraphTuple {
                        source: right_ds.clone(),
                        predicate: "HAS_FIELD".to_string(),
                        target: right_fid.clone(),
                        document_id: None,
                    });
                }

                let edge_key = (
                    left_fid.clone(),
                    "JOINABLE_WITH".to_string(),
                    right_fid.clone(),
                );
                // Preserve human verification if already verified
                let existing_verified = guard
                    .edge_meta
                    .get(&edge_key)
                    .filter(|m| m.epistemic_status == "verified")
                    .cloned();

                guard.edges.insert(GraphTuple {
                    source: left_fid.clone(),
                    predicate: "JOINABLE_WITH".to_string(),
                    target: right_fid.clone(),
                    document_id: None,
                });

                let meta = if let Some(ver) = existing_verified {
                    ver
                } else {
                    OntologyElementMeta {
                        epistemic_status: join.epistemic_status.clone(),
                        confidence: join.confidence,
                        verified_by: None,
                        properties: json!({
                            "left_dataset": left_ds,
                            "left_field": left_fid,
                            "right_dataset": right_ds,
                            "right_field": right_fid,
                            "observed_cardinality": join.observed_cardinality,
                            "inclusion_ratio": join.inclusion_ratio,
                            "row_multiplication_factor": join.row_multiplication_factor,
                            "fan_out_risk": join.fan_out_risk,
                            "homonym_trap_warning": join.homonym_trap_warning,
                        }),
                    }
                };
                guard.edge_meta.insert(edge_key, meta);
                join_edges_synced += 1;
            }
        }

        // Best-effort persistence to PostgreSQL if connected
        if let Err(e) = sqlx::query(
            "INSERT INTO knowledge_objects (id, label, description, layer, embedding, epistemic_status, confidence, properties, updated_at)
             VALUES ($1, 'Dataset', $2, 0, '[1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0]'::vector, $3, 1.0, $4, NOW())
             ON CONFLICT (id) DO UPDATE SET epistemic_status = EXCLUDED.epistemic_status, properties = EXCLUDED.properties, updated_at = NOW()"
        )
        .bind(&ds_id)
        .bind(&ds_desc)
        .bind(&req.epistemic_status)
        .bind(json!({"row_count": req.row_count}))
        .execute(pool)
        .await
        {
            warn!("PostgreSQL ontology sync skipped (in-memory ontology active): {}", e);
        }

        Ok(OntologySyncDatasetResponse {
            dataset_node_id: ds_id,
            fields_synced,
            join_edges_synced,
            skipped_forbidden_joins,
            epistemic_status: req.epistemic_status,
        })
    }

    pub async fn steward_decide_join_edge(
        &self,
        _pool: &PgPool,
        req: StewardEdgeRequest,
    ) -> Result<StewardEdgeResponse, GraphRagError> {
        let left_ds = normalize_entity_id(&req.left_dataset);
        let right_ds = normalize_entity_id(&req.right_dataset);
        let left_fid = canonical_field_node_id(&left_ds, &req.left_field);
        let right_fid = canonical_field_node_id(&right_ds, &req.right_field);

        let mut guard = self.memory.write().await;

        let action_norm = req.action.trim().to_ascii_lowercase();
        if action_norm == "reject" || action_norm == "forbidden_join" {
            // Remove any JOINABLE_WITH between these two fields
            guard.edges.retain(|e| {
                !(e.predicate == "JOINABLE_WITH"
                    && ((e.source == left_fid && e.target == right_fid)
                        || (e.source == right_fid && e.target == left_fid)))
            });
            guard
                .edge_meta
                .remove(&(left_fid.clone(), "JOINABLE_WITH".to_string(), right_fid.clone()));
            guard
                .edge_meta
                .remove(&(right_fid.clone(), "JOINABLE_WITH".to_string(), left_fid.clone()));

            // Insert FORBIDDEN_JOIN negative constraint
            guard.edges.insert(GraphTuple {
                source: left_fid.clone(),
                predicate: "FORBIDDEN_JOIN".to_string(),
                target: right_fid.clone(),
                document_id: None,
            });
            let reason = req.reason.unwrap_or_else(|| {
                "Rejected by human steward: incompatible entity namespaces".to_string()
            });
            guard.edge_meta.insert(
                (
                    left_fid.clone(),
                    "FORBIDDEN_JOIN".to_string(),
                    right_fid.clone(),
                ),
                OntologyElementMeta {
                    epistemic_status: "rejected".to_string(),
                    confidence: 1.0,
                    verified_by: Some(req.steward_id.clone()),
                    properties: json!({
                        "left_dataset": left_ds,
                        "right_dataset": right_ds,
                        "rejection_reason": reason,
                    }),
                },
            );

            Ok(StewardEdgeResponse {
                source_field_id: left_fid,
                predicate: "FORBIDDEN_JOIN".to_string(),
                target_field_id: right_fid,
                epistemic_status: "rejected".to_string(),
                verified_by: req.steward_id,
            })
        } else {
            let key = (
                left_fid.clone(),
                "JOINABLE_WITH".to_string(),
                right_fid.clone(),
            );
            let mut props = guard
                .edge_meta
                .get(&key)
                .map(|m| m.properties.clone())
                .unwrap_or_else(|| {
                    json!({
                        "left_dataset": left_ds,
                        "right_dataset": right_ds,
                        "observed_cardinality": "N:1",
                    })
                });
            if let Some(reason_note) = &req.reason {
                props["steward_note"] = json!(reason_note);
            }

            guard.edges.insert(GraphTuple {
                source: left_fid.clone(),
                predicate: "JOINABLE_WITH".to_string(),
                target: right_fid.clone(),
                document_id: None,
            });
            guard.edge_meta.insert(
                key,
                OntologyElementMeta {
                    epistemic_status: "verified".to_string(),
                    confidence: 1.0,
                    verified_by: Some(req.steward_id.clone()),
                    properties: props,
                },
            );

            Ok(StewardEdgeResponse {
                source_field_id: left_fid,
                predicate: "JOINABLE_WITH".to_string(),
                target_field_id: right_fid,
                epistemic_status: "verified".to_string(),
                verified_by: req.steward_id,
            })
        }
    }

    pub async fn find_ontology_join_path(
        &self,
        _pool: &PgPool,
        req: JoinPathRequest,
    ) -> Result<JoinPathResponse, GraphRagError> {
        let guard = self.memory.read().await;

        // 1. Resolve inputs to canonical Dataset IDs
        let mut resolved_datasets: Vec<String> = Vec::new();
        let mut drifted_elements: BTreeSet<String> = BTreeSet::new();

        for raw in &req.datasets_or_fields {
            let norm = normalize_entity_id(raw);
            if let Some(node) = guard.nodes.get(&norm) {
                if let Some(meta) = guard.node_meta.get(&norm) {
                    if meta.epistemic_status == "drifted" {
                        drifted_elements.insert(norm.clone());
                    }
                }
                if node.label == "Dataset" {
                    if !resolved_datasets.contains(&norm) {
                        resolved_datasets.push(norm);
                    }
                } else if node.label == "Field" {
                    if let Some(ds) = guard
                        .node_meta
                        .get(&norm)
                        .and_then(|m| m.properties.get("dataset"))
                        .and_then(|v| v.as_str())
                    {
                        let ds_str = ds.to_string();
                        if !resolved_datasets.contains(&ds_str) {
                            resolved_datasets.push(ds_str);
                        }
                    }
                }
            } else {
                // Prefix match against known Dataset nodes
                if let Some(ds_node) = guard
                    .nodes
                    .values()
                    .find(|n| n.label == "Dataset" && norm.starts_with(&format!("{}_", n.id)))
                {
                    if !resolved_datasets.contains(&ds_node.id) {
                        resolved_datasets.push(ds_node.id.clone());
                    }
                }
            }
        }

        // Also check if any fields belonging to resolved_datasets are drifted
        for ds in &resolved_datasets {
            if let Some(meta) = guard.node_meta.get(ds) {
                if meta.epistemic_status == "drifted" {
                    drifted_elements.insert(ds.clone());
                }
            }
            for edge in guard.edges.iter().filter(|e| e.source == *ds && e.predicate == "HAS_FIELD") {
                if let Some(fmeta) = guard.node_meta.get(&edge.target) {
                    if fmeta.epistemic_status == "drifted" {
                        drifted_elements.insert(edge.target.clone());
                    }
                }
            }
        }

        let ds_set: HashSet<String> = resolved_datasets.iter().cloned().collect();

        // 2. Inspect FORBIDDEN_JOIN edges between any fields of the resolved datasets
        let mut blocked_forbidden_joins = Vec::new();
        for edge in guard.edges.iter().filter(|e| e.predicate == "FORBIDDEN_JOIN") {
            let meta = guard.edge_meta.get(&(
                edge.source.clone(),
                "FORBIDDEN_JOIN".to_string(),
                edge.target.clone(),
            ));
            let left_ds = meta
                .and_then(|m| m.properties.get("left_dataset"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let right_ds = meta
                .and_then(|m| m.properties.get("right_dataset"))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            if ds_set.contains(left_ds) && ds_set.contains(right_ds) {
                let reason = meta
                    .and_then(|m| m.properties.get("rejection_reason"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("Forbidden join between incompatible entity domains")
                    .to_string();
                blocked_forbidden_joins.push(BlockedForbiddenJoin {
                    left_field: edge.source.clone(),
                    right_field: edge.target.clone(),
                    verified_by: meta.and_then(|m| m.verified_by.clone()),
                    rejection_reason: reason,
                });
            }
        }

        // 3. Build dataset-to-dataset adjacency over JOINABLE_WITH edges
        let mut adj: HashMap<String, Vec<(String, PlannedJoinStep)>> = HashMap::new();
        for edge in guard.edges.iter().filter(|e| e.predicate == "JOINABLE_WITH") {
            let key = (
                edge.source.clone(),
                "JOINABLE_WITH".to_string(),
                edge.target.clone(),
            );
            let meta = guard.edge_meta.get(&key).cloned().unwrap_or_default();
            if meta.confidence < req.min_confidence {
                continue;
            }
            if !req.include_unverified && meta.epistemic_status != "verified" {
                continue;
            }

            let left_ds = meta
                .properties
                .get("left_dataset")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let right_ds = meta
                .properties
                .get("right_dataset")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if left_ds.is_empty() || right_ds.is_empty() {
                continue;
            }

            let card = meta
                .properties
                .get("observed_cardinality")
                .and_then(|v| v.as_str())
                .unwrap_or("N:1")
                .to_string();
            let mult = meta
                .properties
                .get("row_multiplication_factor")
                .and_then(|v| v.as_f64())
                .unwrap_or(1.0);

            let step_forward = PlannedJoinStep {
                left_dataset: left_ds.clone(),
                left_field: edge.source.clone(),
                right_dataset: right_ds.clone(),
                right_field: edge.target.clone(),
                cardinality: card.clone(),
                row_multiplication_factor: mult,
                confidence: meta.confidence,
                epistemic_status: meta.epistemic_status.clone(),
                verified_by: meta.verified_by.clone(),
            };
            let rev_card = match card.as_str() {
                "N:1" => "1:N".to_string(),
                "1:N" => "N:1".to_string(),
                other => other.to_string(),
            };
            let step_reverse = PlannedJoinStep {
                left_dataset: right_ds.clone(),
                left_field: edge.target.clone(),
                right_dataset: left_ds.clone(),
                right_field: edge.source.clone(),
                cardinality: rev_card,
                row_multiplication_factor: mult,
                confidence: meta.confidence,
                epistemic_status: meta.epistemic_status,
                verified_by: meta.verified_by,
            };

            adj.entry(left_ds.clone())
                .or_default()
                .push((right_ds.clone(), step_forward));
            adj.entry(right_ds)
                .or_default()
                .push((left_ds, step_reverse));
        }

        // 4. Connect resolved_datasets sequentially using BFS shortest path
        let mut join_steps: Vec<PlannedJoinStep> = Vec::new();
        if resolved_datasets.len() >= 2 {
            let mut connected: HashSet<String> = HashSet::new();
            connected.insert(resolved_datasets[0].clone());

            for target_ds in resolved_datasets.iter().skip(1) {
                if connected.contains(target_ds) {
                    continue;
                }
                // BFS from any node in `connected` to `target_ds`
                let mut queue: VecDeque<(String, Vec<PlannedJoinStep>)> = VecDeque::new();
                let mut visited: HashSet<String> = HashSet::new();
                for start in &connected {
                    queue.push_back((start.clone(), Vec::new()));
                    visited.insert(start.clone());
                }

                let mut found_path = None;
                while let Some((curr, path)) = queue.pop_front() {
                    if &curr == target_ds {
                        found_path = Some(path);
                        break;
                    }
                    if let Some(neighbors) = adj.get(&curr) {
                        for (next_ds, step) in neighbors {
                            if visited.insert(next_ds.clone()) {
                                let mut next_path = path.clone();
                                next_path.push(step.clone());
                                queue.push_back((next_ds.clone(), next_path));
                            }
                        }
                    }
                }

                if let Some(steps) = found_path {
                    for s in steps {
                        connected.insert(s.left_dataset.clone());
                        connected.insert(s.right_dataset.clone());
                        join_steps.push(s);
                    }
                }
            }
        }

        // 5. Fan-Out & Chasm Trap Detection + Epistemic Rollup
        let mut fan_out_warnings = Vec::new();
        let mut unverified_elements = Vec::new();
        let mut min_confidence = if join_steps.is_empty() { 0.0 } else { 1.0 };

        for step in &join_steps {
            if step.confidence < min_confidence {
                min_confidence = step.confidence;
            }
            if step.epistemic_status != "verified" {
                unverified_elements.push(format!(
                    "{}->JOINABLE_WITH->{}",
                    step.left_field, step.right_field
                ));
            }
            if step.cardinality == "N:M" || step.row_multiplication_factor > 1.05 {
                fan_out_warnings.push(format!(
                    "pre_aggregate_before_join: joining '{}' ({}) to '{}' ({}) has {} cardinality ({:.2}x row multiplication).",
                    step.left_dataset,
                    step.left_field,
                    step.right_dataset,
                    step.right_field,
                    step.cardinality,
                    step.row_multiplication_factor
                ));
            }
        }

        // Also detect 1:N -> 1:N chasm across consecutive steps sharing a parent
        let mut one_to_many_targets_per_dataset: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for step in &join_steps {
            if step.cardinality == "1:N" {
                one_to_many_targets_per_dataset
                    .entry(step.left_dataset.clone())
                    .or_default()
                    .push(step.right_dataset.clone());
            } else if step.cardinality == "N:1" {
                one_to_many_targets_per_dataset
                    .entry(step.right_dataset.clone())
                    .or_default()
                    .push(step.left_dataset.clone());
            }
        }
        for (parent_ds, children) in one_to_many_targets_per_dataset {
            if children.len() >= 2 {
                fan_out_warnings.push(format!(
                    "pre_aggregate_before_join: chasm trap detected on '{}' joining to multiple 1:N tables ({}). Pre-aggregate child tables by key before joining.",
                    parent_ds,
                    children.join(", ")
                ));
            }
        }

        let drifted_vec: Vec<String> = drifted_elements.into_iter().collect();
        let all_rules_human_verified = !join_steps.is_empty()
            && unverified_elements.is_empty()
            && drifted_vec.is_empty()
            && blocked_forbidden_joins.is_empty();
        let requires_steward_review = !unverified_elements.is_empty()
            || !drifted_vec.is_empty()
            || !blocked_forbidden_joins.is_empty()
            || !fan_out_warnings.is_empty();

        Ok(JoinPathResponse {
            resolved_datasets,
            join_steps,
            blocked_forbidden_joins,
            fan_out_warnings,
            drifted_elements: drifted_vec,
            unverified_elements,
            all_rules_human_verified,
            min_confidence,
            requires_steward_review,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::postgres::PgPoolOptions;

    fn mock_pool() -> PgPool {
        PgPoolOptions::new()
            .acquire_timeout(std::time::Duration::from_millis(50))
            .connect_lazy("postgres://mock:mock@127.0.0.1:1/mock")
            .expect("lazy pool")
    }

    #[tokio::test]
    async fn test_sync_dataset_and_join_path_discovers_trap_02_and_verifies() {
        let store = GraphStore::default();
        let pool = mock_pool();

        // Sync crm_customers and sales_orders with TRAP-02 (cust_ref_key <-> party_uuid)
        store
            .sync_dataset_ontology(
                &pool,
                OntologySyncDatasetRequest {
                    dataset_name: "crm_customers".to_string(),
                    description: Some("Master CRM Customer Party table".to_string()),
                    row_count: 100,
                    schema_fingerprint_sha256: Some("sha_crm".to_string()),
                    epistemic_status: "discovered".to_string(),
                    fields: vec![OntologyFieldInput {
                        field_name: "party_uuid".to_string(),
                        arrow_data_type: "Utf8".to_string(),
                        description: Some("Primary Party UUID".to_string()),
                        null_ratio: 0.0,
                        uniqueness_ratio: 1.0,
                        inferred_semantic_domain: "identifier".to_string(),
                        epistemic_status: "discovered".to_string(),
                    }],
                    join_edges: vec![],
                },
            )
            .await
            .expect("sync crm");

        let sync_orders = store
            .sync_dataset_ontology(
                &pool,
                OntologySyncDatasetRequest {
                    dataset_name: "sales_orders".to_string(),
                    description: Some("Sales Orders fact table".to_string()),
                    row_count: 200,
                    schema_fingerprint_sha256: Some("sha_orders".to_string()),
                    epistemic_status: "discovered".to_string(),
                    fields: vec![OntologyFieldInput {
                        field_name: "cust_ref_key".to_string(),
                        arrow_data_type: "Utf8".to_string(),
                        description: None,
                        null_ratio: 0.0,
                        uniqueness_ratio: 0.5,
                        inferred_semantic_domain: "identifier".to_string(),
                        epistemic_status: "discovered".to_string(),
                    }],
                    join_edges: vec![OntologyJoinEdgeInput {
                        left_dataset: "sales_orders".to_string(),
                        left_field: "cust_ref_key".to_string(),
                        right_dataset: "crm_customers".to_string(),
                        right_field: "party_uuid".to_string(),
                        observed_cardinality: "N:1".to_string(),
                        inclusion_ratio: 0.985,
                        row_multiplication_factor: 1.0,
                        fan_out_risk: false,
                        homonym_trap_warning: false,
                        confidence: 0.98,
                        epistemic_status: "empirically_tested".to_string(),
                    }],
                },
            )
            .await
            .expect("sync orders");

        assert_eq!(sync_orders.fields_synced, 1);
        assert_eq!(sync_orders.join_edges_synced, 1);

        // Plan join path before steward verification (Mode B: Autonomous Pre-HITL)
        let plan_pre = store
            .find_ontology_join_path(
                &pool,
                JoinPathRequest {
                    datasets_or_fields: vec![
                        "sales_orders".to_string(),
                        "crm_customers".to_string(),
                    ],
                    min_confidence: 0.6,
                    include_unverified: true,
                },
            )
            .await
            .expect("plan pre-HITL");

        assert_eq!(plan_pre.join_steps.len(), 1);
        assert_eq!(
            plan_pre.join_steps[0].left_field,
            "sales_orders_cust_ref_key"
        );
        assert_eq!(
            plan_pre.join_steps[0].right_field,
            "crm_customers_party_uuid"
        );
        assert!(!plan_pre.all_rules_human_verified);
        assert!(plan_pre.requires_steward_review);

        // Human steward approves the join on the RHS Shared Canvas (Mode C: Post-HITL)
        store
            .steward_decide_join_edge(
                &pool,
                StewardEdgeRequest {
                    left_dataset: "sales_orders".to_string(),
                    left_field: "cust_ref_key".to_string(),
                    right_dataset: "crm_customers".to_string(),
                    right_field: "party_uuid".to_string(),
                    action: "verify".to_string(),
                    steward_id: "alice@polecatworks.io".to_string(),
                    reason: Some("Confirmed CRM foreign key mapping".to_string()),
                },
            )
            .await
            .expect("steward verify");

        let plan_post = store
            .find_ontology_join_path(
                &pool,
                JoinPathRequest {
                    datasets_or_fields: vec![
                        "sales_orders".to_string(),
                        "crm_customers".to_string(),
                    ],
                    min_confidence: 0.6,
                    include_unverified: false,
                },
            )
            .await
            .expect("plan post-HITL");

        assert_eq!(plan_post.join_steps.len(), 1);
        assert!(plan_post.all_rules_human_verified);
        assert!(!plan_post.requires_steward_review);
        assert_eq!(
            plan_post.join_steps[0].verified_by,
            Some("alice@polecatworks.io".to_string())
        );
    }

    #[tokio::test]
    async fn test_steward_reject_blocks_trap_01_forbidden_join_and_prevents_overwrite() {
        let store = GraphStore::default();
        let pool = mock_pool();

        // Sync TRAP-01 candidate join between billing_subscriptions.account_id and marketing_leads.account_id
        store
            .sync_dataset_ontology(
                &pool,
                OntologySyncDatasetRequest {
                    dataset_name: "billing_subscriptions".to_string(),
                    description: None,
                    row_count: 100,
                    schema_fingerprint_sha256: None,
                    epistemic_status: "discovered".to_string(),
                    fields: vec![OntologyFieldInput {
                        field_name: "account_id".to_string(),
                        arrow_data_type: "Int64".to_string(),
                        description: None,
                        null_ratio: 0.0,
                        uniqueness_ratio: 1.0,
                        inferred_semantic_domain: "identifier".to_string(),
                        epistemic_status: "discovered".to_string(),
                    }],
                    join_edges: vec![OntologyJoinEdgeInput {
                        left_dataset: "billing_subscriptions".to_string(),
                        left_field: "account_id".to_string(),
                        right_dataset: "marketing_leads".to_string(),
                        right_field: "account_id".to_string(),
                        observed_cardinality: "1:1".to_string(),
                        inclusion_ratio: 1.0,
                        row_multiplication_factor: 1.0,
                        fan_out_risk: false,
                        homonym_trap_warning: true,
                        confidence: 0.35,
                        epistemic_status: "pending_review".to_string(),
                    }],
                },
            )
            .await
            .expect("sync trap-01");

        // Steward rejects TRAP-01 on RHS Shared Canvas
        let decision = store
            .steward_decide_join_edge(
                &pool,
                StewardEdgeRequest {
                    left_dataset: "billing_subscriptions".to_string(),
                    left_field: "account_id".to_string(),
                    right_dataset: "marketing_leads".to_string(),
                    right_field: "account_id".to_string(),
                    action: "reject".to_string(),
                    steward_id: "bob@polecatworks.io".to_string(),
                    reason: Some("Disjoint surrogate key sequences (Billing Tax IDs vs Ad Leads)".to_string()),
                },
            )
            .await
            .expect("reject trap-01");
        assert_eq!(decision.predicate, "FORBIDDEN_JOIN");
        assert_eq!(decision.epistemic_status, "rejected");

        // Re-syncing must NOT overwrite FORBIDDEN_JOIN
        let resync = store
            .sync_dataset_ontology(
                &pool,
                OntologySyncDatasetRequest {
                    dataset_name: "billing_subscriptions".to_string(),
                    description: None,
                    row_count: 100,
                    schema_fingerprint_sha256: None,
                    epistemic_status: "discovered".to_string(),
                    fields: vec![],
                    join_edges: vec![OntologyJoinEdgeInput {
                        left_dataset: "billing_subscriptions".to_string(),
                        left_field: "account_id".to_string(),
                        right_dataset: "marketing_leads".to_string(),
                        right_field: "account_id".to_string(),
                        observed_cardinality: "1:1".to_string(),
                        inclusion_ratio: 1.0,
                        row_multiplication_factor: 1.0,
                        fan_out_risk: false,
                        homonym_trap_warning: true,
                        confidence: 0.85,
                        epistemic_status: "empirically_tested".to_string(),
                    }],
                },
            )
            .await
            .expect("resync");
        assert_eq!(resync.skipped_forbidden_joins, 1);
        assert_eq!(resync.join_edges_synced, 0);

        // Requesting a join path must return 0 join_steps and populate blocked_forbidden_joins
        let plan = store
            .find_ontology_join_path(
                &pool,
                JoinPathRequest {
                    datasets_or_fields: vec![
                        "billing_subscriptions".to_string(),
                        "marketing_leads".to_string(),
                    ],
                    min_confidence: 0.1,
                    include_unverified: true,
                },
            )
            .await
            .expect("plan forbidden");
        assert!(plan.join_steps.is_empty());
        assert_eq!(plan.blocked_forbidden_joins.len(), 1);
        assert!(
            plan.blocked_forbidden_joins[0]
                .rejection_reason
                .contains("Disjoint surrogate key sequences")
        );
    }

    #[tokio::test]
    async fn test_join_path_emits_fan_out_warning_on_trap_03_and_drift_on_trap_04() {
        let store = GraphStore::default();
        let pool = mock_pool();

        // Sync TRAP-03 (order_items x order_payments N:M fan-out)
        store
            .sync_dataset_ontology(
                &pool,
                OntologySyncDatasetRequest {
                    dataset_name: "order_items".to_string(),
                    description: None,
                    row_count: 500,
                    schema_fingerprint_sha256: None,
                    epistemic_status: "discovered".to_string(),
                    fields: vec![],
                    join_edges: vec![OntologyJoinEdgeInput {
                        left_dataset: "order_items".to_string(),
                        left_field: "order_id".to_string(),
                        right_dataset: "order_payments".to_string(),
                        right_field: "order_id".to_string(),
                        observed_cardinality: "N:M".to_string(),
                        inclusion_ratio: 1.0,
                        row_multiplication_factor: 2.45,
                        fan_out_risk: true,
                        homonym_trap_warning: false,
                        confidence: 0.65,
                        epistemic_status: "pending_review".to_string(),
                    }],
                },
            )
            .await
            .expect("sync trap-03");

        let plan_fanout = store
            .find_ontology_join_path(
                &pool,
                JoinPathRequest {
                    datasets_or_fields: vec![
                        "order_items".to_string(),
                        "order_payments".to_string(),
                    ],
                    min_confidence: 0.5,
                    include_unverified: true,
                },
            )
            .await
            .expect("plan trap-03");

        assert_eq!(plan_fanout.join_steps.len(), 1);
        assert!(!plan_fanout.fan_out_warnings.is_empty());
        assert!(
            plan_fanout.fan_out_warnings[0].contains("pre_aggregate_before_join")
        );
        assert!(plan_fanout.requires_steward_review);

        // Sync TRAP-04 drifted dataset
        store
            .sync_dataset_ontology(
                &pool,
                OntologySyncDatasetRequest {
                    dataset_name: "payment_events_wave3".to_string(),
                    description: None,
                    row_count: 200,
                    schema_fingerprint_sha256: None,
                    epistemic_status: "drifted".to_string(),
                    fields: vec![OntologyFieldInput {
                        field_name: "amount".to_string(),
                        arrow_data_type: "Float64".to_string(),
                        description: None,
                        null_ratio: 0.0,
                        uniqueness_ratio: 0.2,
                        inferred_semantic_domain: "currency_amount".to_string(),
                        epistemic_status: "drifted".to_string(),
                    }],
                    join_edges: vec![],
                },
            )
            .await
            .expect("sync trap-04");

        let plan_drift = store
            .find_ontology_join_path(
                &pool,
                JoinPathRequest {
                    datasets_or_fields: vec!["payment_events_wave3".to_string()],
                    min_confidence: 0.5,
                    include_unverified: true,
                },
            )
            .await
            .expect("plan trap-04");
        assert!(!plan_drift.drifted_elements.is_empty());
        assert!(plan_drift.requires_steward_review);
    }
}
