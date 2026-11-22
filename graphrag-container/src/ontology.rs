use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};

use rmcp::schemars;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::PgPool;
use thiserror::Error;
use tracing::warn;

use crate::graph_rag::{
    GraphRagError, GraphStore, GraphTuple, KnowledgeObject, compute_embedding_64d,
    normalize_entity_id,
};

#[derive(Debug, Error)]
pub enum OntologyError {
    #[error("ontology.subgraph.default_hops must be between 1 and 5, got {0}")]
    InvalidDefaultHops(u8),
    #[error("ontology.subgraph.max_hops ({max}) must be >= default_hops ({default}) and <= 8")]
    InvalidMaxHops { default: u8, max: u8 },
    #[error("ontology.subgraph.default_max_nodes must be between 10 and 5000, got {0}")]
    InvalidMaxNodes(usize),
    #[error("ontology.subgraph.default_max_edges must be between 10 and 20000, got {0}")]
    InvalidMaxEdges(usize),
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct OntologySubgraphConfig {
    pub default_hops: u8,
    pub max_hops: u8,
    pub default_max_nodes: usize,
    pub default_max_edges: usize,
}

impl Default for OntologySubgraphConfig {
    fn default() -> Self {
        Self {
            default_hops: 2,
            max_hops: 5,
            default_max_nodes: 250,
            default_max_edges: 500,
        }
    }
}

impl OntologySubgraphConfig {
    pub fn validate(&self) -> Result<(), OntologyError> {
        if self.default_hops == 0 || self.default_hops > 5 {
            return Err(OntologyError::InvalidDefaultHops(self.default_hops));
        }
        if self.max_hops < self.default_hops || self.max_hops > 8 {
            return Err(OntologyError::InvalidMaxHops {
                default: self.default_hops,
                max: self.max_hops,
            });
        }
        if self.default_max_nodes < 10 || self.default_max_nodes > 5000 {
            return Err(OntologyError::InvalidMaxNodes(self.default_max_nodes));
        }
        if self.default_max_edges < 10 || self.default_max_edges > 20000 {
            return Err(OntologyError::InvalidMaxEdges(self.default_max_edges));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct OntologyConfig {
    pub subgraph: OntologySubgraphConfig,
}

impl OntologyConfig {
    pub fn validate(&self) -> Result<(), OntologyError> {
        self.subgraph.validate()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
pub struct OntologySubgraphRequest {
    #[serde(default)]
    pub focus_node_id: Option<String>,
    #[serde(default)]
    pub hops: Option<u8>,
    #[serde(default)]
    pub layers: Option<Vec<u8>>,
    #[serde(default)]
    pub dataset_ids: Option<Vec<String>>,
    #[serde(default)]
    pub epistemic_status: Option<String>,
    #[serde(default)]
    pub include_forbidden: Option<bool>,
    #[serde(default)]
    pub max_nodes: Option<usize>,
    #[serde(default)]
    pub max_edges: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
pub struct OntologySubgraphNode {
    pub id: String,
    pub label: String,
    pub layer: u8,
    pub description: String,
    pub epistemic_status: String,
    pub confidence: f64,
    pub verified_by: Option<String>,
    pub properties: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
pub struct OntologySubgraphEdge {
    pub source_id: String,
    pub predicate: String,
    pub target_id: String,
    pub epistemic_status: String,
    pub confidence: f64,
    pub derived: bool,
    pub properties: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
pub struct OntologySubgraphCommunity {
    pub id: String,
    pub label: String,
    pub layer: u8,
    pub member_node_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
pub struct OntologySubgraphResponse {
    pub nodes: Vec<OntologySubgraphNode>,
    pub edges: Vec<OntologySubgraphEdge>,
    pub communities: Vec<OntologySubgraphCommunity>,
    pub truncated: bool,
    pub total_matching_nodes: usize,
    pub total_matching_edges: usize,
}

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
pub struct StatisticalProfile {
    #[serde(default)]
    pub distribution_family: Option<String>,
    #[serde(default)]
    pub is_heavy_tailed: bool,
    #[serde(default)]
    pub skewness: Option<f64>,
    #[serde(default)]
    pub kurtosis: Option<f64>,
    #[serde(default)]
    pub seasonality_period: Option<String>,
    #[serde(default)]
    pub recommended_test_family: Option<String>,
    #[serde(default)]
    pub known_confounders: Vec<String>,
    #[serde(default)]
    pub last_diagnosed_run_id: Option<String>,
    #[serde(default = "default_empirically_tested")]
    pub epistemic_status: String,
    #[serde(default = "default_one")]
    pub confidence: f64,
    #[serde(default)]
    pub verified_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub steward_notes: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_verified_profile: Option<Box<StatisticalProfile>>,
}

fn default_empirically_tested() -> String {
    "empirically_tested".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
pub struct StatisticalCaveat {
    pub canonical_id: String,
    pub distribution_family: Option<String>,
    pub is_heavy_tailed: bool,
    pub seasonality_period: Option<String>,
    pub recommended_test_family: Option<String>,
    pub known_confounders: Vec<String>,
    pub epistemic_status: String,
    pub verified_by: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum StewardAction {
    #[serde(alias = "approve", alias = "verified")]
    Verify,
    #[serde(alias = "rejected", alias = "forbidden_join")]
    Reject,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
pub struct StewardFieldRequest {
    pub dataset_name: String,
    pub field_name: String,
    /// Action: "verify" or "reject"
    pub action: StewardAction,
    pub verified_by: String,
    #[serde(default)]
    pub recommended_test_family: Option<String>,
    #[serde(default)]
    pub known_confounders: Option<Vec<String>>,
    #[serde(default)]
    pub seasonality_period: Option<String>,
    #[serde(default)]
    pub steward_notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
pub struct StewardFieldResponse {
    pub field_node_id: String,
    pub canonical_id: String,
    pub epistemic_status: String,
    pub verified_by: String,
    pub statistical_profile: StatisticalProfile,
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
    #[serde(default)]
    pub statistical_profile: Option<StatisticalProfile>,
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
    pub action: StewardAction,
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
    #[serde(default)]
    pub statistical_caveats: Vec<StatisticalCaveat>,
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

                let existing_meta = guard.node_meta.get(&field_node_id).cloned();
                let existing_stat_profile: Option<StatisticalProfile> = existing_meta
                    .as_ref()
                    .and_then(|m| m.properties.get("statistical_profile"))
                    .and_then(|v| serde_json::from_value::<StatisticalProfile>(v.clone()).ok());

                let merged_stat_profile: Option<StatisticalProfile> =
                    match (existing_stat_profile, field.statistical_profile.clone()) {
                        (Some(existing_prof), Some(incoming_prof)) => {
                            let is_drift = field.epistemic_status == "drifted"
                                || incoming_prof.epistemic_status == "drifted";
                            if existing_prof.epistemic_status == "verified" && !is_drift {
                                // Preserve human-verified traits; update only observational run ID
                                let mut preserved = existing_prof;
                                if incoming_prof.last_diagnosed_run_id.is_some() {
                                    preserved.last_diagnosed_run_id =
                                        incoming_prof.last_diagnosed_run_id;
                                }
                                Some(preserved)
                            } else if existing_prof.epistemic_status == "verified" && is_drift {
                                let mut drifted = incoming_prof;
                                drifted.epistemic_status = "drifted".to_string();
                                drifted.previous_verified_profile = Some(Box::new(existing_prof));
                                Some(drifted)
                            } else {
                                Some(incoming_prof)
                            }
                        }
                        (Some(existing_prof), None) => {
                            if field.epistemic_status == "drifted"
                                && existing_prof.epistemic_status == "verified"
                            {
                                let prev = existing_prof.clone();
                                let mut drifted = existing_prof;
                                drifted.epistemic_status = "drifted".to_string();
                                drifted.previous_verified_profile = Some(Box::new(prev));
                                Some(drifted)
                            } else {
                                Some(existing_prof)
                            }
                        }
                        (None, Some(incoming_prof)) => Some(incoming_prof),
                        (None, None) => None,
                    };

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

                let mut field_props = json!({
                    "dataset": ds_id,
                    "field_name": field.field_name,
                    "arrow_data_type": field.arrow_data_type,
                    "null_ratio": field.null_ratio,
                    "uniqueness_ratio": field.uniqueness_ratio,
                    "inferred_semantic_domain": field.inferred_semantic_domain,
                });
                if let Some(prof) = &merged_stat_profile {
                    if let Some(obj) = field_props.as_object_mut() {
                        obj.insert(
                            "statistical_profile".to_string(),
                            serde_json::to_value(prof).unwrap_or(Value::Null),
                        );
                    }
                }

                let final_epistemic = match (&existing_meta, field.epistemic_status.as_str()) {
                    (Some(em), status) if em.epistemic_status == "verified" && status != "drifted" => {
                        "verified".to_string()
                    }
                    (_, status) => status.to_string(),
                };
                let final_verified_by = if final_epistemic == "verified" {
                    existing_meta.as_ref().and_then(|em| em.verified_by.clone())
                } else {
                    None
                };

                guard.node_meta.insert(
                    field_node_id.clone(),
                    OntologyElementMeta {
                        epistemic_status: final_epistemic,
                        confidence: if field.epistemic_status == "drifted" {
                            0.30
                        } else {
                            0.95
                        },
                        verified_by: final_verified_by,
                        properties: field_props,
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

        match req.action {
            StewardAction::Reject => {
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
            }
            StewardAction::Verify => {
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

        // 6. Aggregate epistemic statistical_caveats from participating datasets and fields
        let mut statistical_caveats: Vec<StatisticalCaveat> = Vec::new();
        let mut seen_caveats: HashSet<String> = HashSet::new();
        let participating_datasets: HashSet<String> = resolved_datasets
            .iter()
            .cloned()
            .chain(join_steps.iter().map(|s| s.left_dataset.clone()))
            .chain(join_steps.iter().map(|s| s.right_dataset.clone()))
            .collect();

        for ds in &participating_datasets {
            for edge in guard
                .edges
                .iter()
                .filter(|e| e.predicate == "HAS_FIELD" && &e.source == ds)
            {
                let fid = &edge.target;
                let Some(meta) = guard.node_meta.get(fid) else {
                    continue;
                };
                let Some(prof_val) = meta.properties.get("statistical_profile") else {
                    continue;
                };
                let Ok(prof) = serde_json::from_value::<StatisticalProfile>(prof_val.clone()) else {
                    continue;
                };
                let field_name = meta
                    .properties
                    .get("field_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or(fid);
                let canonical_id = format!("{}.{}", ds, field_name);
                if seen_caveats.insert(canonical_id.clone()) {
                    statistical_caveats.push(StatisticalCaveat {
                        canonical_id,
                        distribution_family: prof.distribution_family,
                        is_heavy_tailed: prof.is_heavy_tailed,
                        seasonality_period: prof.seasonality_period,
                        recommended_test_family: prof.recommended_test_family,
                        known_confounders: prof.known_confounders,
                        epistemic_status: prof.epistemic_status,
                        verified_by: prof.verified_by,
                    });
                }
            }
        }
        statistical_caveats.sort_by(|a, b| a.canonical_id.cmp(&b.canonical_id));

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
            statistical_caveats,
        })
    }

    pub async fn steward_decide_field(
        &self,
        _pool: &PgPool,
        req: StewardFieldRequest,
    ) -> Result<StewardFieldResponse, GraphRagError> {
        let ds_id = normalize_entity_id(&req.dataset_name);
        if ds_id.is_empty() || req.field_name.trim().is_empty() {
            return Err(GraphRagError::EmptyNodeId);
        }
        let field_node_id = canonical_field_node_id(&ds_id, &req.field_name);
        let canonical_id = format!("{}.{}", ds_id, req.field_name.trim());
        let new_status = match req.action {
            StewardAction::Reject => "rejected".to_string(),
            StewardAction::Verify => "verified".to_string(),
        };

        let mut guard = self.memory.write().await;

        let mut existing_meta = guard
            .node_meta
            .get(&field_node_id)
            .cloned()
            .unwrap_or_else(|| OntologyElementMeta {
                epistemic_status: new_status.clone(),
                confidence: 1.0,
                verified_by: Some(req.verified_by.clone()),
                properties: json!({
                    "dataset": ds_id,
                    "field_name": req.field_name.trim(),
                }),
            });

        let mut profile = existing_meta
            .properties
            .get("statistical_profile")
            .and_then(|v| serde_json::from_value::<StatisticalProfile>(v.clone()).ok())
            .unwrap_or(StatisticalProfile {
                distribution_family: None,
                is_heavy_tailed: false,
                skewness: None,
                kurtosis: None,
                seasonality_period: None,
                recommended_test_family: None,
                known_confounders: Vec::new(),
                last_diagnosed_run_id: None,
                epistemic_status: new_status.clone(),
                confidence: 1.0,
                verified_by: Some(req.verified_by.clone()),
                steward_notes: None,
                previous_verified_profile: None,
            });

        profile.epistemic_status = new_status.clone();
        profile.confidence = 1.0;
        profile.verified_by = Some(req.verified_by.clone());
        if let Some(rec) = req.recommended_test_family {
            profile.recommended_test_family = Some(rec);
        }
        if let Some(conf) = req.known_confounders {
            profile.known_confounders = conf;
        }
        if let Some(period) = req.seasonality_period {
            profile.seasonality_period = Some(period);
        }
        if let Some(notes) = req.steward_notes {
            profile.steward_notes = Some(notes);
        }

        if let Some(obj) = existing_meta.properties.as_object_mut() {
            obj.insert(
                "statistical_profile".to_string(),
                serde_json::to_value(&profile).unwrap_or(Value::Null),
            );
        }
        existing_meta.epistemic_status = new_status.clone();
        existing_meta.confidence = 1.0;
        existing_meta.verified_by = Some(req.verified_by.clone());

        guard.node_meta.insert(field_node_id.clone(), existing_meta);
        if !guard.nodes.contains_key(&field_node_id) {
            let desc = format!("Field '{}'", canonical_id);
            let emb = compute_embedding_64d(&format!("{} Field {}", field_node_id, desc));
            guard.nodes.insert(
                field_node_id.clone(),
                KnowledgeObject {
                    id: field_node_id.clone(),
                    label: "Field".to_string(),
                    description: desc,
                    layer: 0,
                    document_id: None,
                    embedding: emb,
                },
            );
            guard.edges.insert(GraphTuple {
                source: ds_id,
                predicate: "HAS_FIELD".to_string(),
                target: field_node_id.clone(),
                document_id: None,
            });
        }

        Ok(StewardFieldResponse {
            field_node_id,
            canonical_id,
            epistemic_status: new_status,
            verified_by: req.verified_by,
            statistical_profile: profile,
        })
    }

    pub async fn export_ontology_subgraph(
        &self,
        _pool: &PgPool,
        config: &OntologySubgraphConfig,
        req: OntologySubgraphRequest,
    ) -> Result<OntologySubgraphResponse, GraphRagError> {
        let hops = req.hops.unwrap_or(config.default_hops);
        if hops == 0 || hops > config.max_hops {
            return Err(GraphRagError::InvalidSubgraphQuery(format!(
                "hops must be between 1 and {}, got {}",
                config.max_hops, hops
            )));
        }

        if let Some(ref layers) = req.layers {
            for &layer in layers {
                if layer > 2 {
                    return Err(GraphRagError::InvalidSubgraphQuery(format!(
                        "layer must be 0, 1, or 2; got {}",
                        layer
                    )));
                }
            }
        }

        let effective_max_nodes = match req.max_nodes {
            Some(0) => {
                return Err(GraphRagError::InvalidSubgraphQuery(
                    "max_nodes must be greater than 0".to_string(),
                ));
            }
            Some(n) => n.min(config.default_max_nodes),
            None => config.default_max_nodes,
        };

        let effective_max_edges = match req.max_edges {
            Some(0) => {
                return Err(GraphRagError::InvalidSubgraphQuery(
                    "max_edges must be greater than 0".to_string(),
                ));
            }
            Some(n) => n.min(config.default_max_edges),
            None => config.default_max_edges,
        };

        let include_forbidden = req.include_forbidden.unwrap_or(true);
        let layer_filter: Option<HashSet<u8>> = req.layers.as_ref().map(|v| v.iter().copied().collect());
        let status_filter: Option<String> = req
            .epistemic_status
            .as_deref()
            .map(|s| s.trim().to_ascii_lowercase())
            .filter(|s| !s.is_empty());

        let guard = self.memory.read().await;

        // Build undirected adjacency list for BFS (respecting include_forbidden and derived confounders)
        let mut adj: HashMap<String, Vec<String>> = HashMap::new();
        for edge in &guard.edges {
            if !include_forbidden && edge.predicate == "FORBIDDEN_JOIN" {
                continue;
            }
            adj.entry(edge.source.clone()).or_default().push(edge.target.clone());
            adj.entry(edge.target.clone()).or_default().push(edge.source.clone());
        }

        // Also link derived confounder edges so confounders are reachable via BFS
        let mut derived_confounder_edges: Vec<OntologySubgraphEdge> = Vec::new();
        for (node_id, meta) in &guard.node_meta {
            if let Some(confounders) = meta
                .properties
                .get("statistical_profile")
                .and_then(|sp| sp.get("known_confounders"))
                .and_then(|c| c.as_array())
            {
                for conf in confounders {
                    if let Some(conf_raw) = conf.as_str()
                        && let Some(target_id) =
                            resolve_subgraph_node_id(conf_raw, &guard.nodes, &guard.node_meta)
                    {
                        adj.entry(node_id.clone()).or_default().push(target_id.clone());
                        adj.entry(target_id.clone()).or_default().push(node_id.clone());
                        let sp_status = meta
                            .properties
                            .get("statistical_profile")
                            .and_then(|sp| sp.get("epistemic_status"))
                            .and_then(|s| s.as_str())
                            .unwrap_or(&meta.epistemic_status)
                            .to_string();
                        let sp_conf = meta
                            .properties
                            .get("statistical_profile")
                            .and_then(|sp| sp.get("confidence"))
                            .and_then(|c| c.as_f64())
                            .unwrap_or(meta.confidence);
                        derived_confounder_edges.push(OntologySubgraphEdge {
                            source_id: node_id.clone(),
                            predicate: "CONFOUNDED_BY".to_string(),
                            target_id,
                            epistemic_status: sp_status,
                            confidence: sp_conf,
                            derived: true,
                            properties: json!({ "source": "statistical_profile.known_confounders" }),
                        });
                    }
                }
            }
        }

        // Determine candidate node IDs via focus BFS, dataset scoping, or full graph
        let candidate_ids: BTreeSet<String> = if let Some(ref raw_focus) = req.focus_node_id {
            let focus_id = resolve_subgraph_node_id(raw_focus, &guard.nodes, &guard.node_meta)
                .ok_or_else(|| GraphRagError::NodeNotFound(raw_focus.clone()))?;
            bfs_neighborhood(&[focus_id], hops, &adj)
        } else if let Some(ref ds_ids) = req.dataset_ids {
            let mut seeds = Vec::new();
            for ds in ds_ids {
                if let Some(resolved) = resolve_subgraph_node_id(ds, &guard.nodes, &guard.node_meta) {
                    seeds.push(resolved);
                }
            }
            if seeds.is_empty() {
                BTreeSet::new()
            } else {
                bfs_neighborhood(&seeds, hops, &adj)
            }
        } else {
            guard.nodes.keys().cloned().collect()
        };

        // Materialize and filter nodes
        let mut matched_nodes: Vec<OntologySubgraphNode> = Vec::new();
        for node_id in &candidate_ids {
            let Some(node) = guard.nodes.get(node_id) else {
                continue;
            };
            let sem_layer = effective_semantic_layer(node);
            if let Some(ref allowed_layers) = layer_filter
                && !allowed_layers.contains(&sem_layer)
            {
                continue;
            }
            let meta = guard.node_meta.get(node_id).cloned().unwrap_or_default();
            if let Some(ref expected_status) = status_filter
                && meta.epistemic_status.to_ascii_lowercase() != *expected_status
            {
                continue;
            }
            matched_nodes.push(OntologySubgraphNode {
                id: node.id.clone(),
                label: node.label.clone(),
                layer: sem_layer,
                description: node.description.clone(),
                epistemic_status: meta.epistemic_status,
                confidence: meta.confidence,
                verified_by: meta.verified_by,
                properties: meta.properties,
            });
        }

        // Deterministic sort: (layer DESC, epistemic_priority ASC, id ASC)
        matched_nodes.sort_by(|a, b| {
            (Reverse(a.layer), epistemic_priority(&a.epistemic_status), &a.id).cmp(&(
                Reverse(b.layer),
                epistemic_priority(&b.epistemic_status),
                &b.id,
            ))
        });

        let total_matching_nodes = matched_nodes.len();
        let matched_node_set: HashSet<String> = matched_nodes.iter().map(|n| n.id.clone()).collect();

        // Collect all edges between matched nodes (prior to truncation for total count)
        let mut all_matching_edges: Vec<OntologySubgraphEdge> = Vec::new();
        for edge in &guard.edges {
            if !include_forbidden && edge.predicate == "FORBIDDEN_JOIN" {
                continue;
            }
            if !matched_node_set.contains(&edge.source) || !matched_node_set.contains(&edge.target) {
                continue;
            }
            let meta = guard
                .edge_meta
                .get(&(edge.source.clone(), edge.predicate.clone(), edge.target.clone()))
                .cloned()
                .unwrap_or_default();
            if let Some(ref expected_status) = status_filter
                && meta.epistemic_status.to_ascii_lowercase() != *expected_status
            {
                continue;
            }
            all_matching_edges.push(OntologySubgraphEdge {
                source_id: edge.source.clone(),
                predicate: edge.predicate.clone(),
                target_id: edge.target.clone(),
                epistemic_status: meta.epistemic_status,
                confidence: meta.confidence,
                derived: false,
                properties: meta.properties,
            });
        }
        for edge in derived_confounder_edges {
            if matched_node_set.contains(&edge.source_id) && matched_node_set.contains(&edge.target_id) {
                all_matching_edges.push(edge);
            }
        }
        all_matching_edges.sort_by(|a, b| {
            (&a.source_id, &a.predicate, &a.target_id).cmp(&(&b.source_id, &b.predicate, &b.target_id))
        });
        all_matching_edges.dedup_by(|a, b| {
            a.source_id == b.source_id && a.predicate == b.predicate && a.target_id == b.target_id
        });
        let total_matching_edges = all_matching_edges.len();

        // Apply node truncation and drop any edges whose endpoints were truncated (zero dangling edges)
        let nodes: Vec<OntologySubgraphNode> = matched_nodes
            .into_iter()
            .take(effective_max_nodes)
            .collect();
        let kept_node_ids: HashSet<&str> = nodes.iter().map(|n| n.id.as_str()).collect();

        let mut edges: Vec<OntologySubgraphEdge> = all_matching_edges
            .into_iter()
            .filter(|e| {
                kept_node_ids.contains(e.source_id.as_str())
                    && kept_node_ids.contains(e.target_id.as_str())
            })
            .collect();
        edges.truncate(effective_max_edges);

        // Collect Layer 2 CommunitySummary hulls whose members intersect kept nodes
        let mut community_members: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for edge in &guard.edges {
            if edge.predicate == "SUMMARIZES" && kept_node_ids.contains(edge.target.as_str()) {
                community_members
                    .entry(edge.source.clone())
                    .or_default()
                    .insert(edge.target.clone());
            }
        }
        let mut communities: Vec<OntologySubgraphCommunity> = Vec::new();
        for (comm_id, members) in community_members {
            if let Some(comm_node) = guard.nodes.get(&comm_id) {
                communities.push(OntologySubgraphCommunity {
                    id: comm_node.id.clone(),
                    label: comm_node.description.clone(),
                    layer: effective_semantic_layer(comm_node),
                    member_node_ids: members.into_iter().collect(),
                });
            }
        }

        let truncated = total_matching_nodes > nodes.len() || total_matching_edges > edges.len();

        Ok(OntologySubgraphResponse {
            nodes,
            edges,
            communities,
            truncated,
            total_matching_nodes,
            total_matching_edges,
        })
    }
}

fn effective_semantic_layer(node: &KnowledgeObject) -> u8 {
    match node.label.as_str() {
        "Field" => 0,
        "Dataset" => 1,
        "BusinessConcept" | "BusinessMetric" | "CommunitySummary" => 2,
        _ => node.layer.clamp(0, 2) as u8,
    }
}

fn epistemic_priority(status: &str) -> u8 {
    match status {
        "verified" => 0,
        "drifted" => 1,
        "empirically_tested" => 2,
        "pending_review" => 3,
        "hypothesized" => 4,
        "discovered" => 5,
        "rejected" => 6,
        _ => 7,
    }
}

fn resolve_subgraph_node_id(
    raw: &str,
    nodes: &BTreeMap<String, KnowledgeObject>,
    node_meta: &BTreeMap<String, OntologyElementMeta>,
) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if nodes.contains_key(trimmed) {
        return Some(trimmed.to_string());
    }
    let normalized = normalize_entity_id(trimmed);
    if nodes.contains_key(&normalized) {
        return Some(normalized);
    }
    if let Some((ds, fl)) = trimmed.split_once('.') {
        let field_id = canonical_field_node_id(ds, fl);
        if nodes.contains_key(&field_id) {
            return Some(field_id);
        }
    }
    for (id, meta) in node_meta {
        if meta
            .properties
            .get("canonical_id")
            .and_then(|v| v.as_str())
            .map(|c| c.eq_ignore_ascii_case(trimmed))
            .unwrap_or(false)
        {
            return Some(id.clone());
        }
    }
    None
}

fn bfs_neighborhood(seeds: &[String], hops: u8, adj: &HashMap<String, Vec<String>>) -> BTreeSet<String> {
    let mut visited: BTreeSet<String> = BTreeSet::new();
    let mut queue: VecDeque<(String, u8)> = VecDeque::new();
    for seed in seeds {
        if visited.insert(seed.clone()) {
            queue.push_back((seed.clone(), 0));
        }
    }
    while let Some((curr, dist)) = queue.pop_front() {
        if dist >= hops {
            continue;
        }
        if let Some(neighbors) = adj.get(&curr) {
            for next in neighbors {
                if visited.insert(next.clone()) {
                    queue.push_back((next.clone(), dist + 1));
                }
            }
        }
    }
    visited
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
                        statistical_profile: None,
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
                        statistical_profile: None,
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
                    action: StewardAction::Verify,
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
                        statistical_profile: None,
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
                    action: StewardAction::Reject,
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
                        statistical_profile: None,
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

    #[tokio::test]
    async fn test_sync_dataset_persists_statistical_profile_in_field_properties() {
        let store = GraphStore::default();
        let pool = mock_pool();

        store
            .sync_dataset_ontology(
                &pool,
                OntologySyncDatasetRequest {
                    dataset_name: "sales_orders".to_string(),
                    description: Some("Sales orders".to_string()),
                    row_count: 5000,
                    schema_fingerprint_sha256: Some("sha_stat_1".to_string()),
                    epistemic_status: "empirically_tested".to_string(),
                    fields: vec![OntologyFieldInput {
                        field_name: "order_total_usd".to_string(),
                        arrow_data_type: "Float64".to_string(),
                        description: Some("Order value in USD".to_string()),
                        null_ratio: 0.0,
                        uniqueness_ratio: 0.8,
                        inferred_semantic_domain: "currency".to_string(),
                        epistemic_status: "empirically_tested".to_string(),
                        statistical_profile: Some(StatisticalProfile {
                            distribution_family: Some("log_normal_heavy_tailed".to_string()),
                            is_heavy_tailed: true,
                            skewness: Some(4.82),
                            kurtosis: Some(31.4),
                            seasonality_period: None,
                            recommended_test_family: Some(
                                "non_parametric_or_log_transform".to_string(),
                            ),
                            known_confounders: vec!["customers.segment".to_string()],
                            last_diagnosed_run_id: Some("run-001".to_string()),
                            epistemic_status: "empirically_tested".to_string(),
                            confidence: 0.93,
                            verified_by: None,
                            steward_notes: None,
                            previous_verified_profile: None,
                        }),
                    }],
                    join_edges: vec![],
                },
            )
            .await
            .expect("sync statistical_profile");

        let guard = store.memory.read().await;
        let fid = canonical_field_node_id("sales_orders", "order_total_usd");
        let meta = guard.node_meta.get(&fid).expect("field meta");
        let prof: StatisticalProfile = serde_json::from_value(
            meta.properties
                .get("statistical_profile")
                .cloned()
                .expect("statistical_profile present"),
        )
        .expect("parse profile");
        assert!(prof.is_heavy_tailed);
        assert_eq!(prof.known_confounders, vec!["customers.segment".to_string()]);
        assert_eq!(
            prof.distribution_family.as_deref(),
            Some("log_normal_heavy_tailed")
        );
    }

    #[tokio::test]
    async fn test_steward_verify_field_protects_statistical_profile_from_unverified_overwrite() {
        let store = GraphStore::default();
        let pool = mock_pool();

        store
            .sync_dataset_ontology(
                &pool,
                OntologySyncDatasetRequest {
                    dataset_name: "sales_orders".to_string(),
                    description: None,
                    row_count: 1000,
                    schema_fingerprint_sha256: None,
                    epistemic_status: "empirically_tested".to_string(),
                    fields: vec![OntologyFieldInput {
                        field_name: "order_total_usd".to_string(),
                        arrow_data_type: "Float64".to_string(),
                        description: None,
                        null_ratio: 0.0,
                        uniqueness_ratio: 0.8,
                        inferred_semantic_domain: "currency".to_string(),
                        epistemic_status: "empirically_tested".to_string(),
                        statistical_profile: Some(StatisticalProfile {
                            distribution_family: Some("log_normal_heavy_tailed".to_string()),
                            is_heavy_tailed: true,
                            skewness: Some(4.82),
                            kurtosis: Some(31.4),
                            seasonality_period: None,
                            recommended_test_family: Some("welch_t".to_string()),
                            known_confounders: vec![],
                            last_diagnosed_run_id: Some("run-001".to_string()),
                            epistemic_status: "empirically_tested".to_string(),
                            confidence: 0.88,
                            verified_by: None,
                            steward_notes: None,
                            previous_verified_profile: None,
                        }),
                    }],
                    join_edges: vec![],
                },
            )
            .await
            .expect("initial sync");

        let steward_resp = store
            .steward_decide_field(
                &pool,
                StewardFieldRequest {
                    dataset_name: "sales_orders".to_string(),
                    field_name: "order_total_usd".to_string(),
                    action: StewardAction::Verify,
                    verified_by: "steward_alice".to_string(),
                    recommended_test_family: Some("non_parametric_or_log_transform".to_string()),
                    known_confounders: Some(vec!["customers.segment".to_string()]),
                    seasonality_period: None,
                    steward_notes: Some("Whale orders skew mean".to_string()),
                },
            )
            .await
            .expect("steward verify field");
        assert_eq!(steward_resp.epistemic_status, "verified");
        assert_eq!(steward_resp.verified_by, "steward_alice");

        // Re-sync with unverified empirically_tested data attempting to change recommended_test_family
        store
            .sync_dataset_ontology(
                &pool,
                OntologySyncDatasetRequest {
                    dataset_name: "sales_orders".to_string(),
                    description: None,
                    row_count: 1050,
                    schema_fingerprint_sha256: None,
                    epistemic_status: "empirically_tested".to_string(),
                    fields: vec![OntologyFieldInput {
                        field_name: "order_total_usd".to_string(),
                        arrow_data_type: "Float64".to_string(),
                        description: None,
                        null_ratio: 0.0,
                        uniqueness_ratio: 0.8,
                        inferred_semantic_domain: "currency".to_string(),
                        epistemic_status: "empirically_tested".to_string(),
                        statistical_profile: Some(StatisticalProfile {
                            distribution_family: Some("normal".to_string()),
                            is_heavy_tailed: false,
                            skewness: Some(0.1),
                            kurtosis: Some(0.0),
                            seasonality_period: None,
                            recommended_test_family: Some("student_t".to_string()),
                            known_confounders: vec![],
                            last_diagnosed_run_id: Some("run-002".to_string()),
                            epistemic_status: "empirically_tested".to_string(),
                            confidence: 0.75,
                            verified_by: None,
                            steward_notes: None,
                            previous_verified_profile: None,
                        }),
                    }],
                    join_edges: vec![],
                },
            )
            .await
            .expect("re-sync");

        let guard = store.memory.read().await;
        let fid = canonical_field_node_id("sales_orders", "order_total_usd");
        let prof: StatisticalProfile = serde_json::from_value(
            guard.node_meta[&fid].properties["statistical_profile"].clone(),
        )
        .expect("parse preserved profile");
        assert_eq!(prof.epistemic_status, "verified");
        assert_eq!(prof.verified_by.as_deref(), Some("steward_alice"));
        assert_eq!(
            prof.recommended_test_family.as_deref(),
            Some("non_parametric_or_log_transform")
        );
        assert_eq!(prof.known_confounders, vec!["customers.segment".to_string()]);
        assert_eq!(prof.last_diagnosed_run_id.as_deref(), Some("run-002"));
    }

    #[tokio::test]
    async fn test_drifted_sync_transitions_verified_statistical_profile_to_drifted() {
        let store = GraphStore::default();
        let pool = mock_pool();

        store
            .steward_decide_field(
                &pool,
                StewardFieldRequest {
                    dataset_name: "daily_revenue".to_string(),
                    field_name: "net_usd".to_string(),
                    action: StewardAction::Verify,
                    verified_by: "steward_bob".to_string(),
                    recommended_test_family: Some("welch_t".to_string()),
                    known_confounders: None,
                    seasonality_period: Some("7d".to_string()),
                    steward_notes: None,
                },
            )
            .await
            .expect("verify initial");

        store
            .sync_dataset_ontology(
                &pool,
                OntologySyncDatasetRequest {
                    dataset_name: "daily_revenue".to_string(),
                    description: None,
                    row_count: 365,
                    schema_fingerprint_sha256: None,
                    epistemic_status: "drifted".to_string(),
                    fields: vec![OntologyFieldInput {
                        field_name: "net_usd".to_string(),
                        arrow_data_type: "Float64".to_string(),
                        description: None,
                        null_ratio: 0.0,
                        uniqueness_ratio: 0.9,
                        inferred_semantic_domain: "currency".to_string(),
                        epistemic_status: "drifted".to_string(),
                        statistical_profile: Some(StatisticalProfile {
                            distribution_family: Some("bimodal".to_string()),
                            is_heavy_tailed: true,
                            skewness: Some(6.5),
                            kurtosis: Some(40.0),
                            seasonality_period: Some("7d".to_string()),
                            recommended_test_family: Some("non_parametric".to_string()),
                            known_confounders: vec![],
                            last_diagnosed_run_id: Some("run-drift-1".to_string()),
                            epistemic_status: "drifted".to_string(),
                            confidence: 0.40,
                            verified_by: None,
                            steward_notes: None,
                            previous_verified_profile: None,
                        }),
                    }],
                    join_edges: vec![],
                },
            )
            .await
            .expect("sync drifted");

        let guard = store.memory.read().await;
        let fid = canonical_field_node_id("daily_revenue", "net_usd");
        let prof: StatisticalProfile = serde_json::from_value(
            guard.node_meta[&fid].properties["statistical_profile"].clone(),
        )
        .expect("parse drifted profile");
        assert_eq!(prof.epistemic_status, "drifted");
        let prev = prof
            .previous_verified_profile
            .expect("previous_verified_profile preserved");
        assert_eq!(prev.verified_by.as_deref(), Some("steward_bob"));
        assert_eq!(prev.recommended_test_family.as_deref(), Some("welch_t"));
    }

    #[tokio::test]
    async fn test_join_path_returns_statistical_caveats_for_participating_fields() {
        let store = GraphStore::default();
        let pool = mock_pool();

        store
            .sync_dataset_ontology(
                &pool,
                OntologySyncDatasetRequest {
                    dataset_name: "orders".to_string(),
                    description: None,
                    row_count: 2000,
                    schema_fingerprint_sha256: None,
                    epistemic_status: "empirically_tested".to_string(),
                    fields: vec![OntologyFieldInput {
                        field_name: "order_total_usd".to_string(),
                        arrow_data_type: "Float64".to_string(),
                        description: None,
                        null_ratio: 0.0,
                        uniqueness_ratio: 0.9,
                        inferred_semantic_domain: "currency".to_string(),
                        epistemic_status: "empirically_tested".to_string(),
                        statistical_profile: Some(StatisticalProfile {
                            distribution_family: Some("log_normal_heavy_tailed".to_string()),
                            is_heavy_tailed: true,
                            skewness: Some(5.1),
                            kurtosis: Some(29.0),
                            seasonality_period: Some("7d".to_string()),
                            recommended_test_family: Some(
                                "non_parametric_or_log_transform".to_string(),
                            ),
                            known_confounders: vec!["customers.segment".to_string()],
                            last_diagnosed_run_id: Some("run-jp-1".to_string()),
                            epistemic_status: "empirically_tested".to_string(),
                            confidence: 0.92,
                            verified_by: None,
                            steward_notes: None,
                            previous_verified_profile: None,
                        }),
                    }],
                    join_edges: vec![],
                },
            )
            .await
            .expect("sync orders");

        store
            .steward_decide_field(
                &pool,
                StewardFieldRequest {
                    dataset_name: "orders".to_string(),
                    field_name: "order_total_usd".to_string(),
                    action: StewardAction::Verify,
                    verified_by: "steward_alice".to_string(),
                    recommended_test_family: None,
                    known_confounders: None,
                    seasonality_period: None,
                    steward_notes: None,
                },
            )
            .await
            .expect("verify field");

        let plan = store
            .find_ontology_join_path(
                &pool,
                JoinPathRequest {
                    datasets_or_fields: vec!["orders".to_string()],
                    min_confidence: 0.5,
                    include_unverified: true,
                },
            )
            .await
            .expect("join path");

        assert_eq!(plan.statistical_caveats.len(), 1);
        let caveat = &plan.statistical_caveats[0];
        assert_eq!(caveat.canonical_id, "orders.order_total_usd");
        assert!(caveat.is_heavy_tailed);
        assert_eq!(caveat.seasonality_period.as_deref(), Some("7d"));
        assert_eq!(caveat.known_confounders, vec!["customers.segment".to_string()]);
        assert_eq!(caveat.epistemic_status, "verified");
        assert_eq!(caveat.verified_by.as_deref(), Some("steward_alice"));
    }

    #[test]
    fn test_ontology_subgraph_config_validation_rejects_invalid_bounds() {
        assert!(OntologySubgraphConfig::default().validate().is_ok());

        let zero_hops = OntologySubgraphConfig {
            default_hops: 0,
            ..Default::default()
        };
        assert!(matches!(
            zero_hops.validate(),
            Err(OntologyError::InvalidDefaultHops(0))
        ));

        let bad_max_hops = OntologySubgraphConfig {
            default_hops: 3,
            max_hops: 2,
            ..Default::default()
        };
        assert!(matches!(
            bad_max_hops.validate(),
            Err(OntologyError::InvalidMaxHops { default: 3, max: 2 })
        ));

        let zero_nodes = OntologySubgraphConfig {
            default_max_nodes: 5,
            ..Default::default()
        };
        assert!(matches!(
            zero_nodes.validate(),
            Err(OntologyError::InvalidMaxNodes(5))
        ));

        let zero_edges = OntologySubgraphConfig {
            default_max_edges: 0,
            ..Default::default()
        };
        assert!(matches!(
            zero_edges.validate(),
            Err(OntologyError::InvalidMaxEdges(0))
        ));
    }

    async fn seed_multilayer_ontology(store: &GraphStore, pool: &PgPool) {
        // Sync crm_customers + sales_orders
        store
            .sync_dataset_ontology(
                pool,
                OntologySyncDatasetRequest {
                    dataset_name: "crm_customers".to_string(),
                    description: Some("CRM Customers".to_string()),
                    row_count: 100,
                    schema_fingerprint_sha256: Some("sha_crm".to_string()),
                    epistemic_status: "verified".to_string(),
                    fields: vec![
                        OntologyFieldInput {
                            field_name: "party_uuid".to_string(),
                            arrow_data_type: "Utf8".to_string(),
                            description: Some("Primary Party UUID".to_string()),
                            null_ratio: 0.0,
                            uniqueness_ratio: 1.0,
                            inferred_semantic_domain: "identifier".to_string(),
                            epistemic_status: "verified".to_string(),
                            statistical_profile: None,
                        },
                        OntologyFieldInput {
                            field_name: "segment".to_string(),
                            arrow_data_type: "Utf8".to_string(),
                            description: Some("Customer Segment".to_string()),
                            null_ratio: 0.0,
                            uniqueness_ratio: 0.02,
                            inferred_semantic_domain: "categorical".to_string(),
                            epistemic_status: "verified".to_string(),
                            statistical_profile: None,
                        },
                    ],
                    join_edges: vec![],
                },
            )
            .await
            .expect("sync crm");

        store
            .sync_dataset_ontology(
                pool,
                OntologySyncDatasetRequest {
                    dataset_name: "sales_orders".to_string(),
                    description: Some("Sales Orders".to_string()),
                    row_count: 500,
                    schema_fingerprint_sha256: Some("sha_orders".to_string()),
                    epistemic_status: "verified".to_string(),
                    fields: vec![
                        OntologyFieldInput {
                            field_name: "cust_ref_key".to_string(),
                            arrow_data_type: "Utf8".to_string(),
                            description: Some("Customer FK".to_string()),
                            null_ratio: 0.0,
                            uniqueness_ratio: 0.2,
                            inferred_semantic_domain: "identifier".to_string(),
                            epistemic_status: "verified".to_string(),
                            statistical_profile: None,
                        },
                        OntologyFieldInput {
                            field_name: "amount_usd".to_string(),
                            arrow_data_type: "Float64".to_string(),
                            description: Some("Order Amount USD".to_string()),
                            null_ratio: 0.0,
                            uniqueness_ratio: 0.9,
                            inferred_semantic_domain: "currency".to_string(),
                            epistemic_status: "drifted".to_string(),
                            statistical_profile: Some(StatisticalProfile {
                                distribution_family: Some("log_normal_heavy_tailed".to_string()),
                                is_heavy_tailed: true,
                                skewness: Some(4.5),
                                kurtosis: Some(25.0),
                                seasonality_period: None,
                                recommended_test_family: Some("mann_whitney_u".to_string()),
                                known_confounders: vec!["crm_customers.segment".to_string()],
                                last_diagnosed_run_id: Some("run-1".to_string()),
                                epistemic_status: "empirically_tested".to_string(),
                                confidence: 0.93,
                                verified_by: None,
                                steward_notes: None,
                                previous_verified_profile: None,
                            }),
                        },
                    ],
                    join_edges: vec![OntologyJoinEdgeInput {
                        left_dataset: "sales_orders".to_string(),
                        left_field: "cust_ref_key".to_string(),
                        right_dataset: "crm_customers".to_string(),
                        right_field: "party_uuid".to_string(),
                        observed_cardinality: "N:1".to_string(),
                        inclusion_ratio: 0.97,
                        row_multiplication_factor: 1.0,
                        fan_out_risk: false,
                        homonym_trap_warning: false,
                        confidence: 0.97,
                        epistemic_status: "empirically_tested".to_string(),
                    }],
                },
            )
            .await
            .expect("sync orders");

        // Add Layer 2 BusinessMetric, CommunitySummary, and a FORBIDDEN_JOIN edge
        let mut guard = store.memory.write().await;
        guard.nodes.insert(
            "net_revenue".to_string(),
            KnowledgeObject {
                id: "net_revenue".to_string(),
                label: "BusinessMetric".to_string(),
                description: "Net revenue in USD".to_string(),
                layer: 2,
                document_id: None,
                embedding: vec![0.0; 64],
            },
        );
        guard.node_meta.insert(
            "net_revenue".to_string(),
            OntologyElementMeta {
                epistemic_status: "verified".to_string(),
                confidence: 1.0,
                verified_by: Some("steward_alice".to_string()),
                properties: json!({ "formula": "SUM(amount_usd)" }),
            },
        );
        guard.edges.insert(GraphTuple {
            source: "net_revenue".to_string(),
            predicate: "COMPUTED_FROM".to_string(),
            target: "sales_orders".to_string(),
            document_id: None,
        });
        guard.nodes.insert(
            "comm_revenue".to_string(),
            KnowledgeObject {
                id: "comm_revenue".to_string(),
                label: "CommunitySummary".to_string(),
                description: "Revenue & Billing".to_string(),
                layer: 2,
                document_id: None,
                embedding: vec![0.0; 64],
            },
        );
        guard.edges.insert(GraphTuple {
            source: "comm_revenue".to_string(),
            predicate: "SUMMARIZES".to_string(),
            target: "sales_orders".to_string(),
            document_id: None,
        });
        guard.edges.insert(GraphTuple {
            source: "sales_orders_amount_usd".to_string(),
            predicate: "FORBIDDEN_JOIN".to_string(),
            target: "crm_customers_party_uuid".to_string(),
            document_id: None,
        });
        guard.edge_meta.insert(
            (
                "sales_orders_amount_usd".to_string(),
                "FORBIDDEN_JOIN".to_string(),
                "crm_customers_party_uuid".to_string(),
            ),
            OntologyElementMeta {
                epistemic_status: "rejected".to_string(),
                confidence: 1.0,
                verified_by: Some("steward_alice".to_string()),
                properties: json!({ "rejection_reason": "Incompatible domains" }),
            },
        );
    }

    #[tokio::test]
    async fn test_export_subgraph_ego_network_respects_hops_and_layers() {
        let store = GraphStore::default();
        let pool = mock_pool();
        seed_multilayer_ontology(&store, &pool).await;
        let cfg = OntologySubgraphConfig::default();

        // 1-hop from net_revenue reaches sales_orders (Layer 1) but not sales_orders_amount_usd (2 hops)
        let hop1 = store
            .export_ontology_subgraph(
                &pool,
                &cfg,
                OntologySubgraphRequest {
                    focus_node_id: Some("net_revenue".to_string()),
                    hops: Some(1),
                    ..Default::default()
                },
            )
            .await
            .expect("hop1");
        let hop1_ids: HashSet<&str> = hop1.nodes.iter().map(|n| n.id.as_str()).collect();
        assert!(hop1_ids.contains("net_revenue"));
        assert!(hop1_ids.contains("sales_orders"));
        assert!(!hop1_ids.contains("sales_orders_amount_usd"));
        assert_eq!(hop1.communities.len(), 1);
        assert_eq!(hop1.communities[0].id, "comm_revenue");

        // 2-hop with layers=[1, 2] excludes Layer 0 fields
        let hop2_l12 = store
            .export_ontology_subgraph(
                &pool,
                &cfg,
                OntologySubgraphRequest {
                    focus_node_id: Some("net_revenue".to_string()),
                    hops: Some(2),
                    layers: Some(vec![1, 2]),
                    ..Default::default()
                },
            )
            .await
            .expect("hop2 layers 1,2");
        assert!(hop2_l12.nodes.iter().all(|n| n.layer == 1 || n.layer == 2));
        assert!(hop2_l12.nodes.iter().any(|n| n.id == "net_revenue" && n.layer == 2));
        assert!(hop2_l12.nodes.iter().any(|n| n.id == "sales_orders" && n.layer == 1));
    }

    #[tokio::test]
    async fn test_export_subgraph_includes_forbidden_join_and_derived_confounders() {
        let store = GraphStore::default();
        let pool = mock_pool();
        seed_multilayer_ontology(&store, &pool).await;
        let cfg = OntologySubgraphConfig::default();

        let with_forbidden = store
            .export_ontology_subgraph(
                &pool,
                &cfg,
                OntologySubgraphRequest {
                    include_forbidden: Some(true),
                    ..Default::default()
                },
            )
            .await
            .expect("with forbidden");

        assert!(
            with_forbidden
                .edges
                .iter()
                .any(|e| e.predicate == "FORBIDDEN_JOIN"),
            "FORBIDDEN_JOIN should be present when include_forbidden=true"
        );
        assert!(
            with_forbidden.edges.iter().any(|e| {
                e.predicate == "CONFOUNDED_BY"
                    && e.derived
                    && e.source_id == "sales_orders_amount_usd"
                    && e.target_id == "crm_customers_segment"
            }),
            "derived CONFOUNDED_BY edge should be materialized from known_confounders"
        );

        let without_forbidden = store
            .export_ontology_subgraph(
                &pool,
                &cfg,
                OntologySubgraphRequest {
                    include_forbidden: Some(false),
                    ..Default::default()
                },
            )
            .await
            .expect("without forbidden");
        assert!(
            !without_forbidden
                .edges
                .iter()
                .any(|e| e.predicate == "FORBIDDEN_JOIN"),
            "FORBIDDEN_JOIN must be excluded when include_forbidden=false"
        );
    }

    #[tokio::test]
    async fn test_export_subgraph_enforces_max_nodes_truncation_without_dangling_edges() {
        let store = GraphStore::default();
        let pool = mock_pool();
        seed_multilayer_ontology(&store, &pool).await;
        let cfg = OntologySubgraphConfig::default();

        let resp = store
            .export_ontology_subgraph(
                &pool,
                &cfg,
                OntologySubgraphRequest {
                    max_nodes: Some(3),
                    ..Default::default()
                },
            )
            .await
            .expect("truncated export");

        assert!(resp.truncated);
        assert_eq!(resp.nodes.len(), 3);
        assert!(resp.total_matching_nodes > 3);

        let kept: HashSet<&str> = resp.nodes.iter().map(|n| n.id.as_str()).collect();
        for edge in &resp.edges {
            assert!(
                kept.contains(edge.source_id.as_str()) && kept.contains(edge.target_id.as_str()),
                "dangling edge detected after truncation: {:?}",
                edge
            );
        }
    }

    #[tokio::test]
    async fn test_export_subgraph_returns_not_found_for_unknown_focus_node() {
        let store = GraphStore::default();
        let pool = mock_pool();
        let cfg = OntologySubgraphConfig::default();

        let err = store
            .export_ontology_subgraph(
                &pool,
                &cfg,
                OntologySubgraphRequest {
                    focus_node_id: Some("does_not_exist".to_string()),
                    ..Default::default()
                },
            )
            .await
            .expect_err("unknown focus node must fail");

        assert!(matches!(err, GraphRagError::NodeNotFound(_)));
    }

    #[test]
    fn test_steward_action_deserialization_and_unknown_rejection() {
        for valid_verify in [r#""verify""#, r#""approve""#, r#""verified""#] {
            let parsed: StewardAction = serde_json::from_str(valid_verify).unwrap();
            assert_eq!(parsed, StewardAction::Verify);
        }
        for valid_reject in [r#""reject""#, r#""rejected""#, r#""forbidden_join""#] {
            let parsed: StewardAction = serde_json::from_str(valid_reject).unwrap();
            assert_eq!(parsed, StewardAction::Reject);
        }
        let invalid: Result<StewardAction, _> = serde_json::from_str(r#""unknown_action""#);
        assert!(invalid.is_err());
    }
}
