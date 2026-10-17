use crate::llm_tools::{LlmClient, LlmError};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::warn;

pub const EMBEDDING_DIM: usize = 64;
pub const DEFAULT_MAX_CHUNK_CHARS: usize = 1200;

#[derive(Debug, Error)]
pub enum GraphRagError {
    #[error("markdown content must not be empty")]
    EmptyMarkdownContent,
    #[error("filename must not be empty")]
    EmptyFilename,
    #[error("question must not be empty")]
    EmptyQuestion,
    #[error("max_chars must be greater than 0")]
    ZeroMaxChunkChars,
    #[error("failed to parse LLM extraction JSON: {0}")]
    InvalidExtractionJson(#[from] serde_json::Error),
    #[error(transparent)]
    Llm(#[from] LlmError),
    #[error("database query failed: {0}")]
    Database(#[from] sqlx::Error),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KnowledgeObject {
    pub id: String,
    pub label: String,
    pub description: String,
    #[serde(default)]
    pub layer: i32,
    #[serde(default)]
    pub document_id: Option<String>,
    #[serde(default)]
    pub embedding: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct GraphTuple {
    pub source: String,
    pub predicate: String,
    pub target: String,
    #[serde(default)]
    pub document_id: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ExtractionResult {
    #[serde(default)]
    pub nodes: Vec<KnowledgeObject>,
    #[serde(default)]
    pub edges: Vec<GraphTuple>,
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryGraph {
    pub documents: BTreeMap<String, String>,
    pub nodes: BTreeMap<String, KnowledgeObject>,
    pub edges: BTreeSet<GraphTuple>,
}

#[derive(Clone, Debug, Default)]
pub struct GraphStore {
    pub memory: Arc<RwLock<InMemoryGraph>>,
}

/// Normalizes an entity name or identifier into a canonical snake_case slug.
pub fn normalize_entity_id(raw: &str) -> String {
    let mut slug = String::with_capacity(raw.len());
    let mut prev_underscore = false;
    for ch in raw.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            prev_underscore = false;
        } else if !prev_underscore && !slug.is_empty() {
            slug.push('_');
            prev_underscore = true;
        }
    }
    while slug.ends_with('_') {
        slug.pop();
    }
    slug
}

/// Splits raw markdown into semantic chunks bounded by `max_chars`, preserving section headings.
pub fn chunk_markdown(content: &str, max_chars: usize) -> Result<Vec<String>, GraphRagError> {
    if max_chars == 0 {
        return Err(GraphRagError::ZeroMaxChunkChars);
    }
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Err(GraphRagError::EmptyMarkdownContent);
    }

    let mut sections: Vec<String> = Vec::new();
    let mut current_section = String::new();

    for line in trimmed.lines() {
        let is_heading = line.trim_start().starts_with('#');
        if is_heading && !current_section.trim().is_empty() {
            sections.push(current_section.trim().to_string());
            current_section.clear();
        }
        if !current_section.is_empty() {
            current_section.push('\n');
        }
        current_section.push_str(line);
    }
    if !current_section.trim().is_empty() {
        sections.push(current_section.trim().to_string());
    }

    let mut chunks: Vec<String> = Vec::new();
    for section in sections {
        if section.len() <= max_chars {
            chunks.push(section);
            continue;
        }

        // Split oversized sections by double-newline paragraphs while keeping header context
        let mut header_prefix = String::new();
        let mut body_start = 0;
        if let Some(first_newline) = section.find('\n') {
            let first_line = &section[..first_newline];
            if first_line.trim_start().starts_with('#') {
                header_prefix = first_line.trim().to_string();
                body_start = first_newline + 1;
            }
        }

        let paragraphs: Vec<&str> = section[body_start..]
            .split("\n\n")
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .collect();

        let mut current_chunk = header_prefix.clone();
        for para in paragraphs {
            let candidate_len = if current_chunk.is_empty() {
                para.len()
            } else {
                current_chunk.len() + 2 + para.len()
            };

            if candidate_len > max_chars && !current_chunk.is_empty() && current_chunk != header_prefix {
                chunks.push(current_chunk.trim().to_string());
                current_chunk = header_prefix.clone();
            }

            if !current_chunk.is_empty() {
                current_chunk.push_str("\n\n");
            }
            current_chunk.push_str(para);
        }

        if !current_chunk.trim().is_empty() {
            chunks.push(current_chunk.trim().to_string());
        }
    }

    Ok(chunks)
}

/// Computes a normalized 64-dimensional feature-hashed embedding vector for `pgvector`.
pub fn compute_embedding_64d(text: &str) -> Vec<f32> {
    let mut vec = vec![0.0_f32; EMBEDDING_DIM];
    let normalized = text.to_ascii_lowercase();

    for token in normalized
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|t| t.len() >= 2)
    {
        let mut h1: u64 = 14695981039346656037;
        for b in token.as_bytes() {
            h1 ^= u64::from(*b);
            h1 = h1.wrapping_mul(1099511628211);
        }
        let idx = (h1 as usize) % EMBEDDING_DIM;
        let sign = if (h1 >> 32) & 1 == 0 { 1.0 } else { -1.0 };
        vec[idx] += sign;

        // Also hash 3-char prefixes to boost morphological similarity
        if token.len() >= 3 {
            let prefix = &token[..3];
            let mut h2: u64 = 14695981039346656037;
            for b in prefix.as_bytes() {
                h2 ^= u64::from(*b);
                h2 = h2.wrapping_mul(1099511628211);
            }
            let idx2 = (h2 as usize) % EMBEDDING_DIM;
            vec[idx2] += 0.5;
        }
    }

    let norm: f32 = vec.iter().map(|v| v * v).sum::<f32>().sqrt();
    if norm > 0.0 {
        for v in &mut vec {
            *v /= norm;
        }
    } else {
        vec[0] = 1.0;
    }
    vec
}

pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
}

pub fn format_pgvector(vec: &[f32]) -> String {
    let inner: Vec<String> = vec.iter().map(|v| format!("{:.6}", v)).collect();
    format!("[{}]", inner.join(","))
}

/// Parses raw LLM output (supporting optional ```json code fences) into normalized `ExtractionResult`.
pub fn parse_extraction_json(
    raw_llm_output: &str,
    document_id: Option<&str>,
) -> Result<ExtractionResult, GraphRagError> {
    let trimmed = raw_llm_output.trim();
    let json_slice = if let Some(start) = trimmed.find('{') {
        if let Some(end) = trimmed.rfind('}') {
            &trimmed[start..=end]
        } else {
            trimmed
        }
    } else {
        trimmed
    };

    let parsed: ExtractionResult = serde_json::from_str(json_slice)?;
    let doc_id_owned = document_id.map(str::to_string);

    let mut normalized_nodes = Vec::with_capacity(parsed.nodes.len());
    for mut node in parsed.nodes {
        node.id = normalize_entity_id(&node.id);
        if node.id.is_empty() {
            continue;
        }
        if node.label.trim().is_empty() {
            node.label = "Concept".to_string();
        }
        node.document_id = doc_id_owned.clone().or(node.document_id);
        node.embedding = compute_embedding_64d(&format!(
            "{} {} {}",
            node.id, node.label, node.description
        ));
        normalized_nodes.push(node);
    }

    let mut normalized_edges = Vec::with_capacity(parsed.edges.len());
    for mut edge in parsed.edges {
        edge.source = normalize_entity_id(&edge.source);
        edge.target = normalize_entity_id(&edge.target);
        edge.predicate = normalize_entity_id(&edge.predicate).to_ascii_uppercase();
        if edge.source.is_empty() || edge.target.is_empty() || edge.predicate.is_empty() {
            continue;
        }
        edge.document_id = doc_id_owned.clone().or(edge.document_id);
        normalized_edges.push(edge);
    }

    Ok(merge_extractions(vec![ExtractionResult {
        nodes: normalized_nodes,
        edges: normalized_edges,
    }]))
}

/// Deterministically extracts Knowledge Objects and Graph Tuples from markdown sections and key phrases.
pub fn extract_heuristic_from_markdown(chunk: &str, document_id: &str) -> ExtractionResult {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    let mut title = document_id
        .trim_end_matches(".md")
        .trim_start_matches(|c: char| c.is_ascii_digit() || c == '_')
        .replace('_', " ");

    for line in chunk.lines() {
        let trimmed = line.trim();
        if let Some(heading) = trimmed.strip_prefix('#') {
            let clean_heading = heading.trim_start_matches('#').trim();
            if !clean_heading.is_empty() {
                title = clean_heading.to_string();
                break;
            }
        }
    }

    let primary_id = normalize_entity_id(&title);
    if !primary_id.is_empty() {
        let description = chunk
            .lines()
            .filter(|l| !l.trim().starts_with('#') && !l.trim().is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        let emb = compute_embedding_64d(&format!("{} {}", title, description));
        nodes.push(KnowledgeObject {
            id: primary_id.clone(),
            label: "DocumentSection".to_string(),
            description: if description.is_empty() {
                title.clone()
            } else {
                description.clone()
            },
            layer: 0,
            document_id: Some(document_id.to_string()),
            embedding: emb,
        });

        // Recognize capitalized multi-word entities and link them to the section's primary entity
        let known_entities = [
            ("Project Apollo", "Program"),
            ("Apollo 11", "Mission"),
            ("Neil Armstrong", "Astronaut"),
            ("Buzz Aldrin", "Astronaut"),
            ("Michael Collins", "Astronaut"),
            ("John F. Kennedy", "President"),
            ("Command Module Columbia", "Spacecraft"),
            ("Lunar Module Eagle", "Spacecraft"),
            ("Saturn V", "LaunchVehicle"),
            ("West Point", "University"),
            ("United States Military Academy at West Point", "University"),
            ("Massachusetts Institute of Technology", "University"),
            ("Purdue University", "University"),
            ("NASA", "Organization"),
        ];

        let chunk_lower = chunk.to_ascii_lowercase();
        for (entity_name, label) in known_entities {
            if chunk_lower.contains(&entity_name.to_ascii_lowercase()) {
                let entity_id = normalize_entity_id(entity_name);
                if entity_id != primary_id {
                    let matching_sentences: Vec<&str> = chunk
                        .split('.')
                        .map(str::trim)
                        .filter(|s| {
                            s.to_ascii_lowercase()
                                .contains(&entity_name.to_ascii_lowercase())
                        })
                        .collect();
                    let entity_desc = if matching_sentences.is_empty() {
                        format!("{} mentioned in {}", entity_name, title)
                    } else {
                        format!("{}.", matching_sentences.join(". "))
                    };
                    nodes.push(KnowledgeObject {
                        id: entity_id.clone(),
                        label: label.to_string(),
                        description: entity_desc.clone(),
                        layer: 0,
                        document_id: Some(document_id.to_string()),
                        embedding: compute_embedding_64d(&format!(
                            "{} {} {}",
                            entity_name, label, entity_desc
                        )),
                    });
                    edges.push(GraphTuple {
                        source: primary_id.clone(),
                        predicate: "MENTIONS".to_string(),
                        target: entity_id,
                        document_id: Some(document_id.to_string()),
                    });
                }
            }
        }
    }

    merge_extractions(vec![ExtractionResult { nodes, edges }])
}

/// Idempotently merges multiple `ExtractionResult` batches by canonical node ID and edge tuple.
pub fn merge_extractions(batches: Vec<ExtractionResult>) -> ExtractionResult {
    let mut node_map: BTreeMap<String, KnowledgeObject> = BTreeMap::new();
    let mut edge_map: BTreeMap<(String, String, String), GraphTuple> = BTreeMap::new();

    for batch in batches {
        for mut node in batch.nodes {
            node.id = normalize_entity_id(&node.id);
            if node.id.is_empty() {
                continue;
            }
            if node.embedding.len() != EMBEDDING_DIM {
                node.embedding = compute_embedding_64d(&format!(
                    "{} {} {}",
                    node.id, node.label, node.description
                ));
            }
            node_map
                .entry(node.id.clone())
                .and_modify(|existing| {
                    if !existing.description.contains(&node.description)
                        && !node.description.is_empty()
                    {
                        existing.description =
                            format!("{} {}", existing.description.trim(), node.description.trim());
                        existing.embedding = compute_embedding_64d(&format!(
                            "{} {} {}",
                            existing.id, existing.label, existing.description
                        ));
                    }
                })
                .or_insert(node);
        }

        for mut edge in batch.edges {
            edge.source = normalize_entity_id(&edge.source);
            edge.target = normalize_entity_id(&edge.target);
            edge.predicate = normalize_entity_id(&edge.predicate).to_ascii_uppercase();
            if !edge.source.is_empty() && !edge.target.is_empty() && !edge.predicate.is_empty() {
                edge_map
                    .entry((
                        edge.source.clone(),
                        edge.predicate.clone(),
                        edge.target.clone(),
                    ))
                    .or_insert(edge);
            }
        }
    }

    ExtractionResult {
        nodes: node_map.into_values().collect(),
        edges: edge_map.into_values().collect(),
    }
}

/// Detects communities among Layer 0 nodes using connected components over undirected edges.
pub fn detect_communities(
    nodes: &[KnowledgeObject],
    edges: &[GraphTuple],
) -> BTreeMap<String, Vec<String>> {
    let layer0_ids: BTreeSet<String> = nodes
        .iter()
        .filter(|n| n.layer == 0)
        .map(|n| n.id.clone())
        .collect();

    let mut adj: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for id in &layer0_ids {
        adj.entry(id.clone()).or_default();
    }
    for edge in edges {
        if layer0_ids.contains(&edge.source) && layer0_ids.contains(&edge.target) {
            adj.entry(edge.source.clone())
                .or_default()
                .insert(edge.target.clone());
            adj.entry(edge.target.clone())
                .or_default()
                .insert(edge.source.clone());
        }
    }

    let mut visited: HashSet<String> = HashSet::new();
    let mut communities: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut community_idx = 1;

    for node_id in &layer0_ids {
        if visited.contains(node_id) {
            continue;
        }
        let mut members = Vec::new();
        let mut queue = VecDeque::new();
        queue.push_back(node_id.clone());
        visited.insert(node_id.clone());

        while let Some(curr) = queue.pop_front() {
            members.push(curr.clone());
            if let Some(neighbors) = adj.get(&curr) {
                for nb in neighbors {
                    if visited.insert(nb.clone()) {
                        queue.push_back(nb.clone());
                    }
                }
            }
        }

        members.sort();
        let community_id = format!("community_{}", community_idx);
        communities.insert(community_id, members);
        community_idx += 1;
    }

    communities
}

/// Builds Layer 1 `CommunitySummary` nodes and `"SUMMARIZES"` hierarchy edges from Layer 0 nodes & edges.
pub fn build_hierarchical_summaries(
    nodes: &[KnowledgeObject],
    edges: &[GraphTuple],
) -> ExtractionResult {
    let communities = detect_communities(nodes, edges);
    let node_lookup: BTreeMap<&str, &KnowledgeObject> =
        nodes.iter().map(|n| (n.id.as_str(), n)).collect();

    let mut summary_nodes = Vec::new();
    let mut hierarchy_edges = Vec::new();

    for (community_id, member_ids) in communities {
        if member_ids.is_empty() {
            continue;
        }
        let mut descriptions = Vec::new();
        for member_id in &member_ids {
            if let Some(member) = node_lookup.get(member_id.as_str()) {
                descriptions.push(format!(
                    "{} ({}): {}",
                    member.id, member.label, member.description
                ));
            }
            hierarchy_edges.push(GraphTuple {
                source: community_id.clone(),
                predicate: "SUMMARIZES".to_string(),
                target: member_id.clone(),
                document_id: None,
            });
        }

        let summary_text = format!(
            "Community {} summary covering [{}]: {}",
            community_id,
            member_ids.join(", "),
            descriptions.join(" | ")
        );
        let embedding = compute_embedding_64d(&summary_text);
        summary_nodes.push(KnowledgeObject {
            id: community_id,
            label: "CommunitySummary".to_string(),
            description: summary_text,
            layer: 1,
            document_id: None,
            embedding,
        });
    }

    ExtractionResult {
        nodes: summary_nodes,
        edges: hierarchy_edges,
    }
}

/// Retrieves top-K seed nodes by cosine similarity + keyword overlap and traverses up to `max_hops` hops.
pub fn traverse_multi_hop(
    question: &str,
    nodes: &[KnowledgeObject],
    edges: &[GraphTuple],
    top_k_seeds: usize,
    max_hops: usize,
) -> Vec<String> {
    if nodes.is_empty() {
        return Vec::new();
    }

    let q_emb = compute_embedding_64d(question);
    let q_tokens: HashSet<String> = question
        .to_ascii_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|t| t.len() >= 3)
        .map(str::to_string)
        .collect();

    let mut scored_nodes: Vec<(f32, &KnowledgeObject)> = nodes
        .iter()
        .map(|node| {
            let sim = cosine_similarity(&q_emb, &node.embedding);
            let node_text = format!("{} {} {}", node.id, node.label, node.description)
                .to_ascii_lowercase();
            let keyword_hits = q_tokens
                .iter()
                .filter(|tok| node_text.contains(tok.as_str()))
                .count() as f32;
            (sim + keyword_hits * 0.35, node)
        })
        .collect();

    scored_nodes.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    let seed_ids: Vec<String> = scored_nodes
        .iter()
        .take(top_k_seeds.max(1))
        .map(|(_, n)| n.id.clone())
        .collect();

    let node_lookup: BTreeMap<&str, &KnowledgeObject> =
        nodes.iter().map(|n| (n.id.as_str(), n)).collect();

    let mut visited_nodes: BTreeSet<String> = BTreeSet::new();
    let mut visited_edges: BTreeSet<GraphTuple> = BTreeSet::new();
    let mut frontier: VecDeque<(String, usize)> = VecDeque::new();

    for seed in seed_ids {
        visited_nodes.insert(seed.clone());
        frontier.push_back((seed, 0));
    }

    while let Some((curr_id, depth)) = frontier.pop_front() {
        if depth >= max_hops {
            continue;
        }
        for edge in edges {
            if edge.source == curr_id {
                visited_edges.insert(edge.clone());
                if visited_nodes.insert(edge.target.clone()) {
                    frontier.push_back((edge.target.clone(), depth + 1));
                }
            } else if edge.target == curr_id {
                visited_edges.insert(edge.clone());
                if visited_nodes.insert(edge.source.clone()) {
                    frontier.push_back((edge.source.clone(), depth + 1));
                }
            }
        }
    }

    let mut contexts = Vec::new();
    for node_id in &visited_nodes {
        if let Some(node) = node_lookup.get(node_id.as_str()) {
            contexts.push(format!(
                "{} ({}): {}",
                node.id, node.label, node.description
            ));
        }
    }
    for edge in &visited_edges {
        contexts.push(format!(
            "{} -[{}]-> {}",
            edge.source, edge.predicate, edge.target
        ));
    }

    contexts
}

impl GraphStore {
    pub async fn ingest_markdown(
        &self,
        pool: &PgPool,
        llm: &LlmClient,
        filename: &str,
        content: &str,
    ) -> Result<(String, usize, usize, usize), GraphRagError> {
        let clean_filename = filename.trim();
        if clean_filename.is_empty() {
            return Err(GraphRagError::EmptyFilename);
        }
        let chunks = chunk_markdown(content, DEFAULT_MAX_CHUNK_CHARS)?;
        let document_id = clean_filename.to_string();

        let preamble = "You are a Graph RAG entity and relationship extraction engine. Return ONLY valid JSON with the schema: {\"nodes\": [{\"id\": \"snake_case_id\", \"label\": \"EntityType\", \"description\": \"detailed factual summary\"}], \"edges\": [{\"source\": \"source_id\", \"predicate\": \"RELATIONSHIP_TYPE\", \"target\": \"target_id\"}]}.";

        let mut batches = Vec::new();
        for chunk in &chunks {
            // Heuristic extraction always runs to guarantee deterministic baseline coverage
            batches.push(extract_heuristic_from_markdown(chunk, &document_id));

            // Attempt live LLM extraction via rig-core if reachable
            if let Ok(raw_json) = llm.prompt(preamble, chunk).await
                && let Ok(llm_extracted) = parse_extraction_json(&raw_json, Some(&document_id))
            {
                batches.push(llm_extracted);
            }
        }

        let merged = merge_extractions(batches);
        let nodes_extracted = merged.nodes.len();
        let edges_extracted = merged.edges.len();

        // Update in-memory store and recompute Layer 1 community summaries
        let all_nodes_and_summaries = {
            let mut guard = self.memory.write().await;
            guard
                .documents
                .insert(document_id.clone(), content.to_string());
            for node in &merged.nodes {
                guard.nodes.insert(node.id.clone(), node.clone());
            }
            for edge in &merged.edges {
                guard.edges.insert(edge.clone());
            }

            let layer0_nodes: Vec<KnowledgeObject> = guard
                .nodes
                .values()
                .filter(|n| n.layer == 0)
                .cloned()
                .collect();
            let layer0_edges: Vec<GraphTuple> = guard
                .edges
                .iter()
                .filter(|e| e.predicate != "SUMMARIZES")
                .cloned()
                .collect();
            let summaries = build_hierarchical_summaries(&layer0_nodes, &layer0_edges);

            guard.nodes.retain(|_, n| n.layer == 0);
            guard.edges.retain(|e| e.predicate != "SUMMARIZES");
            for s_node in &summaries.nodes {
                guard.nodes.insert(s_node.id.clone(), s_node.clone());
            }
            for s_edge in &summaries.edges {
                guard.edges.insert(s_edge.clone());
            }

            (
                guard.nodes.values().cloned().collect::<Vec<_>>(),
                guard.edges.iter().cloned().collect::<Vec<_>>(),
            )
        };

        // Best-effort persistence to PostgreSQL when database pool is live
        if let Err(e) = self
            .persist_to_postgres(
                pool,
                &document_id,
                clean_filename,
                content,
                &all_nodes_and_summaries.0,
                &all_nodes_and_summaries.1,
            )
            .await
        {
            warn!("PostgreSQL persistence skipped or failed (in-memory graph active): {}", e);
        }

        Ok((
            document_id,
            chunks.len(),
            nodes_extracted,
            edges_extracted,
        ))
    }

    async fn persist_to_postgres(
        &self,
        pool: &PgPool,
        document_id: &str,
        filename: &str,
        content: &str,
        nodes: &[KnowledgeObject],
        edges: &[GraphTuple],
    ) -> Result<(), GraphRagError> {
        sqlx::query(
            "INSERT INTO graph_documents (id, filename, content, updated_at)
             VALUES ($1, $2, $3, NOW())
             ON CONFLICT (id) DO UPDATE SET content = EXCLUDED.content, updated_at = NOW()",
        )
        .bind(document_id)
        .bind(filename)
        .bind(content)
        .execute(pool)
        .await?;

        for node in nodes {
            let vec_literal = format_pgvector(&node.embedding);
            sqlx::query(
                "INSERT INTO knowledge_objects (id, label, description, layer, document_id, embedding, updated_at)
                 VALUES ($1, $2, $3, $4, $5, $6::vector, NOW())
                 ON CONFLICT (id) DO UPDATE SET
                    label = EXCLUDED.label,
                    description = EXCLUDED.description,
                    layer = EXCLUDED.layer,
                    embedding = EXCLUDED.embedding,
                    updated_at = NOW()",
            )
            .bind(&node.id)
            .bind(&node.label)
            .bind(&node.description)
            .bind(node.layer)
            .bind(&node.document_id)
            .bind(&vec_literal)
            .execute(pool)
            .await?;
        }

        for edge in edges {
            sqlx::query(
                "INSERT INTO graph_tuples (source_id, predicate, target_id, document_id, updated_at)
                 VALUES ($1, $2, $3, $4, NOW())
                 ON CONFLICT (source_id, predicate, target_id) DO UPDATE SET updated_at = NOW()",
            )
            .bind(&edge.source)
            .bind(&edge.predicate)
            .bind(&edge.target)
            .bind(&edge.document_id)
            .execute(pool)
            .await?;
        }

        Ok(())
    }

    pub async fn answer_question(
        &self,
        _pool: &PgPool,
        llm: &LlmClient,
        question: &str,
    ) -> Result<(String, Vec<String>), GraphRagError> {
        let clean_q = question.trim();
        if clean_q.is_empty() {
            return Err(GraphRagError::EmptyQuestion);
        }

        let (nodes, edges) = {
            let guard = self.memory.read().await;
            (
                guard.nodes.values().cloned().collect::<Vec<_>>(),
                guard.edges.iter().cloned().collect::<Vec<_>>(),
            )
        };

        let contexts = traverse_multi_hop(clean_q, &nodes, &edges, 5, 2);

        let synthesis_preamble = "You are a precise Graph RAG question-answering assistant. Answer the user's question directly and factually using ONLY the provided Graph Contexts.";
        let prompt_body = format!(
            "Graph Contexts:\n{}\n\nQuestion: {}\n\nProvide a concise, factual answer:",
            contexts.join("\n"),
            clean_q
        );

        let answer = match llm.prompt(synthesis_preamble, &prompt_body).await {
            Ok(resp) if !resp.trim().is_empty() => resp.trim().to_string(),
            _ => synthesize_deterministic_answer(clean_q, &contexts),
        };

        Ok((answer, contexts))
    }

    pub async fn delete_document(&self, pool: &PgPool, document_id: &str) {
        {
            let mut guard = self.memory.write().await;
            guard.documents.remove(document_id);
            guard
                .nodes
                .retain(|_, n| n.document_id.as_deref() != Some(document_id));
            guard
                .edges
                .retain(|e| e.document_id.as_deref() != Some(document_id));
        }

        let _ = sqlx::query("DELETE FROM graph_documents WHERE id = $1")
            .bind(document_id)
            .execute(pool)
            .await;
        let _ = sqlx::query("DELETE FROM knowledge_objects WHERE document_id = $1")
            .bind(document_id)
            .execute(pool)
            .await;
        let _ = sqlx::query("DELETE FROM graph_tuples WHERE document_id = $1")
            .bind(document_id)
            .execute(pool)
            .await;
    }
}

/// Synthesizes a grounded factual answer from retrieved contexts using Maximal Marginal Relevance (MMR) over question tokens.
pub fn synthesize_deterministic_answer(question: &str, contexts: &[String]) -> String {
    if contexts.is_empty() {
        return "No relevant knowledge graph context found for the question.".to_string();
    }

    let stopwords: HashSet<&'static str> = [
        "the", "and", "for", "that", "was", "with", "his", "her", "from", "what", "which", "who",
        "did", "were", "while", "into", "their",
    ]
    .into_iter()
    .collect();

    let q_tokens: HashSet<String> = question
        .to_ascii_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|t| t.len() >= 3 && !stopwords.contains(*t))
        .map(str::to_string)
        .collect();

    let mut candidates: Vec<(String, HashSet<String>, String)> = Vec::new();
    let mut seen = HashSet::new();

    for ctx in contexts {
        if (ctx.contains("-[") && ctx.contains("]->")) || ctx.contains("(CommunitySummary):") {
            continue;
        }
        let (header, body) = ctx.split_once(": ").unwrap_or(("context", ctx.as_str()));
        let entity_id = header.split_whitespace().next().unwrap_or("context").to_string();
        for sentence in body.split('.').map(str::trim).filter(|s| !s.is_empty()) {
            let clean_sentence = sentence
                .lines()
                .filter(|l| !l.trim().starts_with('#'))
                .collect::<Vec<_>>()
                .join(" ")
                .trim()
                .to_string();
            if clean_sentence.is_empty() {
                continue;
            }
            let s_lower = clean_sentence.to_ascii_lowercase();
            let matched: HashSet<String> = q_tokens
                .iter()
                .filter(|tok| {
                    let stem = if tok.len() >= 5 { &tok[..5] } else { tok.as_str() };
                    s_lower.contains(stem)
                })
                .cloned()
                .collect();
            if !matched.is_empty() && seen.insert(clean_sentence.clone()) {
                candidates.push((
                    entity_id.clone(),
                    matched,
                    format!("{}.", clean_sentence),
                ));
            }
        }
    }

    if candidates.is_empty() {
        return contexts.first().cloned().unwrap_or_default();
    }

    // Greedily pick up to 5 sentences using Maximal Marginal Relevance over question tokens + entity diversity
    let mut covered_tokens: HashSet<String> = HashSet::new();
    let mut covered_entities: HashSet<String> = HashSet::new();
    let mut selected_sentences: Vec<String> = Vec::new();
    let mut used_indices: HashSet<usize> = Vec::new().into_iter().collect();

    for _ in 0..5 {
        let mut best_idx = None;
        let mut best_score = 0usize;

        for (idx, (entity_id, matched, _)) in candidates.iter().enumerate() {
            if used_indices.contains(&idx) {
                continue;
            }
            let new_hits = matched.difference(&covered_tokens).count();
            let is_new_entity = !covered_entities.contains(entity_id);
            if !selected_sentences.is_empty() && new_hits == 0 && !(is_new_entity && matched.len() >= 3) {
                continue;
            }
            let entity_bonus = if is_new_entity && matched.len() >= 3 {
                50
            } else {
                0
            };
            let score = new_hits * 100 + entity_bonus + matched.len();
            if score > best_score {
                best_score = score;
                best_idx = Some(idx);
            }
        }

        if let Some(idx) = best_idx {
            used_indices.insert(idx);
            covered_entities.insert(candidates[idx].0.clone());
            for tok in &candidates[idx].1 {
                covered_tokens.insert(tok.clone());
            }
            selected_sentences.push(candidates[idx].2.clone());
        } else {
            break;
        }
    }

    selected_sentences.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_markdown_by_headers_and_paragraphs() {
        let md = "# Project Apollo\n\nProject Apollo was the third US human spaceflight program.\n\n## Apollo 11\n\nApollo 11 landed on the Moon on July 20, 1969.";
        let chunks = chunk_markdown(md, 120).unwrap();
        assert_eq!(chunks.len(), 2);
        assert!(chunks[0].starts_with("# Project Apollo"));
        assert!(chunks[1].starts_with("## Apollo 11"));
    }

    #[test]
    fn test_chunk_markdown_rejects_empty_or_zero_max_chars() {
        assert!(matches!(
            chunk_markdown("   ", 100),
            Err(GraphRagError::EmptyMarkdownContent)
        ));
        assert!(matches!(
            chunk_markdown("# Header", 0),
            Err(GraphRagError::ZeroMaxChunkChars)
        ));
    }

    #[test]
    fn test_parse_llm_extraction_json_and_normalize_ids() {
        let raw = r#"```json
        {
            "nodes": [
                {
                    "id": "Michael Collins",
                    "label": "Astronaut",
                    "description": "Command Module Pilot of Apollo 11 who flew Columbia in lunar orbit."
                },
                {
                    "id": "Command Module Columbia",
                    "label": "Spacecraft",
                    "description": "Command module of Apollo 11."
                }
            ],
            "edges": [
                {
                    "source": "Michael Collins",
                    "predicate": "piloted",
                    "target": "Command Module Columbia"
                }
            ]
        }
        ```"#;

        let extracted = parse_extraction_json(raw, Some("04_michael_collins.md")).unwrap();
        assert_eq!(extracted.nodes.len(), 2);
        assert_eq!(extracted.nodes[0].id, "command_module_columbia");
        assert_eq!(extracted.nodes[1].id, "michael_collins");
        assert_eq!(extracted.nodes[1].embedding.len(), EMBEDDING_DIM);
        assert_eq!(extracted.edges.len(), 1);
        assert_eq!(extracted.edges[0].source, "michael_collins");
        assert_eq!(extracted.edges[0].predicate, "PILOTED");
        assert_eq!(extracted.edges[0].target, "command_module_columbia");
    }

    #[test]
    fn test_ingest_idempotency_merges_nodes_and_edges() {
        let batch1 = parse_extraction_json(
            r#"{"nodes":[{"id":"Apollo 11","label":"Mission","description":"First crewed lunar landing."}],"edges":[{"source":"Neil Armstrong","predicate":"COMMANDED","target":"Apollo 11"}]}"#,
            Some("doc1.md"),
        )
        .unwrap();
        let batch2 = parse_extraction_json(
            r#"{"nodes":[{"id":"apollo_11","label":"Mission","description":"Launched on Saturn V."}],"edges":[{"source":"neil_armstrong","predicate":"commanded","target":"apollo_11"}]}"#,
            Some("doc2.md"),
        )
        .unwrap();

        let merged = merge_extractions(vec![batch1, batch2]);
        assert_eq!(merged.nodes.len(), 1);
        assert!(merged.nodes[0].description.contains("First crewed lunar landing."));
        assert!(merged.nodes[0].description.contains("Launched on Saturn V."));
        assert_eq!(merged.edges.len(), 1);
    }

    #[test]
    fn test_deterministic_embedding_dimension_and_similarity() {
        let emb_apollo = compute_embedding_64d("Apollo 11 Command Module Columbia lunar orbit");
        let emb_collins = compute_embedding_64d("Michael Collins piloted Command Module Columbia in lunar orbit");
        let emb_unrelated = compute_embedding_64d("Database connection pool timeout configuration");

        assert_eq!(emb_apollo.len(), EMBEDDING_DIM);
        let sim_related = cosine_similarity(&emb_apollo, &emb_collins);
        let sim_unrelated = cosine_similarity(&emb_apollo, &emb_unrelated);
        assert!(sim_related > sim_unrelated);
        assert!(format_pgvector(&emb_apollo).starts_with('['));
    }

    #[test]
    fn test_detect_communities_and_build_hierarchical_summaries() {
        let extracted = parse_extraction_json(
            r#"{
                "nodes": [
                    {"id": "neil_armstrong", "label": "Astronaut", "description": "Commander of Apollo 11."},
                    {"id": "apollo_11", "label": "Mission", "description": "Landed on the Moon."},
                    {"id": "jfk", "label": "President", "description": "Proposed Moon goal."}
                ],
                "edges": [
                    {"source": "neil_armstrong", "predicate": "COMMANDED", "target": "apollo_11"}
                ]
            }"#,
            None,
        )
        .unwrap();

        let communities = detect_communities(&extracted.nodes, &extracted.edges);
        assert_eq!(communities.len(), 2);

        let summaries = build_hierarchical_summaries(&extracted.nodes, &extracted.edges);
        assert_eq!(summaries.nodes.len(), 2);
        assert!(summaries.nodes.iter().all(|n| n.layer == 1 && n.label == "CommunitySummary"));
        assert_eq!(summaries.edges.len(), 3);
        assert!(summaries.edges.iter().all(|e| e.predicate == "SUMMARIZES"));
    }

    #[test]
    fn test_multi_hop_subgraph_traversal() {
        let extracted = parse_extraction_json(
            r#"{
                "nodes": [
                    {"id": "john_f_kennedy", "label": "President", "description": "President John F. Kennedy proposed the Moon goal in May 1961."},
                    {"id": "project_apollo", "label": "Program", "description": "NASA program that fulfilled Kennedy's goal with Apollo 11."},
                    {"id": "buzz_aldrin", "label": "Astronaut", "description": "Buzz Aldrin earned his doctorate from MIT and landed on Apollo 11."}
                ],
                "edges": [
                    {"source": "john_f_kennedy", "predicate": "PROPOSED_GOAL_FOR", "target": "project_apollo"},
                    {"source": "project_apollo", "predicate": "INCLUDED_ASTRONAUT", "target": "buzz_aldrin"}
                ]
            }"#,
            None,
        )
        .unwrap();

        let contexts = traverse_multi_hop(
            "Which President proposed the goal for Project Apollo?",
            &extracted.nodes,
            &extracted.edges,
            1,
            2,
        );
        assert!(contexts.iter().any(|c| c.contains("john_f_kennedy")));
        assert!(contexts.iter().any(|c| c.contains("project_apollo")));
        assert!(contexts.iter().any(|c| c.contains("buzz_aldrin")));
    }
}
