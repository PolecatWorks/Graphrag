use rmcp::{
    ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{ServerCapabilities, ServerConfig},
    schemars, tool, tool_handler, tool_router,
};
use serde::{Deserialize, Serialize};
use crate::mcp::McpError;
use crate::state::AppState;
use crate::graph_rag::{GraphTuple, KnowledgeObject};

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct HelloRequest {
    #[schemars(description = "The name of the user, persona, or entity to greet.")]
    pub name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct GraphRagIngestRequest {
    #[schemars(description = "Filename or identifier of the Markdown document.")]
    pub filename: String,
    #[schemars(description = "Full Markdown text content to ingest.")]
    pub content: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct GraphRagQueryRequest {
    #[schemars(description = "Question or query to ask against the Graph RAG index.")]
    pub question: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct DocumentBrowseRequest {
    #[schemars(description = "Substring search query for filename or content.")]
    pub q: Option<String>,
    #[schemars(description = "Maximum number of items to return.")]
    pub limit: Option<usize>,
    #[schemars(description = "Number of items to skip.")]
    pub offset: Option<usize>,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct DocumentReadRequest {
    #[schemars(description = "Document ID to read.")]
    pub id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct DocumentEditRequest {
    #[schemars(description = "Document ID to edit.")]
    pub id: String,
    #[schemars(description = "Updated Markdown content.")]
    pub content: String,
    #[schemars(description = "Updated filename (optional).")]
    pub filename: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct DocumentAddRequest {
    #[schemars(description = "Filename for the new Markdown document.")]
    pub filename: String,
    #[schemars(description = "Markdown text content.")]
    pub content: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct DocumentDeleteRequest {
    #[schemars(description = "Document ID to delete.")]
    pub id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct NodeBrowseRequest {
    #[schemars(description = "Filter by exact entity label.")]
    pub label: Option<String>,
    #[schemars(description = "Filter by entity layer (0 for base entities, >=1 for communities).")]
    pub layer: Option<i32>,
    #[schemars(description = "Filter by origin document ID.")]
    pub document_id: Option<String>,
    #[schemars(description = "Keyword search over id and description.")]
    pub q: Option<String>,
    #[schemars(description = "Semantic query text to search using cosine similarity.")]
    pub semantic_query: Option<String>,
    #[schemars(description = "Maximum number of nodes to return.")]
    pub limit: Option<usize>,
    #[schemars(description = "Number of nodes to skip.")]
    pub offset: Option<usize>,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct NodeReadRequest {
    #[schemars(description = "Entity ID (normalized snake_case) to retrieve.")]
    pub id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct NodeEditRequest {
    #[schemars(description = "Entity ID to update.")]
    pub id: String,
    #[schemars(description = "Updated entity label.")]
    pub label: Option<String>,
    #[schemars(description = "Updated entity description.")]
    pub description: Option<String>,
    #[schemars(description = "Updated entity layer.")]
    pub layer: Option<i32>,
    #[schemars(description = "Updated source document ID.")]
    pub document_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct NodeAddRequest {
    #[schemars(description = "Unique entity ID (will be normalized to snake_case).")]
    pub id: String,
    #[schemars(description = "Entity label (e.g., PERSON, ORGANIZATION, CONCEPT).")]
    pub label: String,
    #[schemars(description = "Entity description.")]
    pub description: String,
    #[schemars(description = "Entity layer (defaults to 0).")]
    pub layer: Option<i32>,
    #[schemars(description = "Origin document ID (optional).")]
    pub document_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct NodeDeleteRequest {
    #[schemars(description = "Entity ID to delete.")]
    pub id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct EdgeBrowseRequest {
    #[schemars(description = "Filter by source node ID.")]
    pub source_id: Option<String>,
    #[schemars(description = "Filter by predicate (e.g. RELATES_TO).")]
    pub predicate: Option<String>,
    #[schemars(description = "Filter by target node ID.")]
    pub target_id: Option<String>,
    #[schemars(description = "Filter by origin document ID.")]
    pub document_id: Option<String>,
    #[schemars(description = "Maximum number of edges to return.")]
    pub limit: Option<usize>,
    #[schemars(description = "Number of edges to skip.")]
    pub offset: Option<usize>,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct EdgeReadRequest {
    #[schemars(description = "Source node ID.")]
    pub source_id: String,
    #[schemars(description = "Relationship predicate.")]
    pub predicate: String,
    #[schemars(description = "Target node ID.")]
    pub target_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct EdgeEditRequest {
    #[schemars(description = "Source node ID of edge to edit.")]
    pub source_id: String,
    #[schemars(description = "Current predicate of edge.")]
    pub predicate: String,
    #[schemars(description = "Target node ID of edge.")]
    pub target_id: String,
    #[schemars(description = "New relationship predicate (UPPER_SNAKE_CASE).")]
    pub new_predicate: Option<String>,
    #[schemars(description = "Updated origin document ID.")]
    pub document_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct EdgeAddRequest {
    #[schemars(description = "Source node ID.")]
    pub source_id: String,
    #[schemars(description = "Relationship predicate.")]
    pub predicate: String,
    #[schemars(description = "Target node ID.")]
    pub target_id: String,
    #[schemars(description = "Origin document ID (optional).")]
    pub document_id: Option<String>,
    #[schemars(description = "Whether to auto-create missing endpoint nodes (default false).")]
    pub auto_create_nodes: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct EdgeDeleteRequest {
    #[schemars(description = "Source node ID.")]
    pub source_id: String,
    #[schemars(description = "Relationship predicate.")]
    pub predicate: String,
    #[schemars(description = "Target node ID.")]
    pub target_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct CommunityBrowseRequest {
    #[schemars(description = "Keyword search in community description.")]
    pub q: Option<String>,
    #[schemars(description = "Filter communities containing this member node ID.")]
    pub member_id: Option<String>,
    #[schemars(description = "Maximum number of communities to return.")]
    pub limit: Option<usize>,
    #[schemars(description = "Number of communities to skip.")]
    pub offset: Option<usize>,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct CommunityReadRequest {
    #[schemars(description = "Community summary ID to read.")]
    pub id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct CommunityEditRequest {
    #[schemars(description = "Community summary ID to edit.")]
    pub id: String,
    #[schemars(description = "Updated community description.")]
    pub description: String,
    #[schemars(description = "Updated community label (optional).")]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct CommunityRebuildRequest {}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct CommunityDeleteRequest {
    #[schemars(description = "Community summary ID to delete.")]
    pub id: String,
}

#[derive(Clone)]
pub struct GraphRagMcpServer {
    tool_router: ToolRouter<Self>,
}

impl std::fmt::Debug for GraphRagMcpServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GraphRagMcpServer").finish()
    }
}

impl GraphRagMcpServer {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    pub fn list_tools(&self) -> Vec<rmcp::model::Tool> {
        self.tool_router.list_all()
    }

    pub fn info(&self, instructions: &str) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_instructions(instructions)
    }

    pub async fn call_tool(
        &self,
        state: &AppState,
        name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value, McpError> {
        match name {
            "hello" => {
                let req: HelloRequest = serde_json::from_value(arguments)
                    .map_err(|e| McpError::InvalidArguments("hello".to_string(), e.to_string()))?;
                let trimmed = req.name.trim();
                if trimmed.is_empty() {
                    return Err(McpError::EmptyGreetingName);
                }
                Ok(serde_json::json!({
                    "content": [{"type": "text", "text": format!("Hello, {}!", trimmed)}],
                    "isError": false
                }))
            }
            "graphrag_ingest" => {
                let req: GraphRagIngestRequest = serde_json::from_value(arguments)
                    .map_err(|e| McpError::InvalidArguments("graphrag_ingest".to_string(), e.to_string()))?;
                let (document_id, chunks_processed, nodes_extracted, edges_extracted) = state
                    .graph_store
                    .ingest_markdown(&state.pool, &state.llm, &req.filename, &req.content)
                    .await?;
                let res = serde_json::json!({
                    "document_id": document_id,
                    "chunks_processed": chunks_processed,
                    "nodes_extracted": nodes_extracted,
                    "edges_extracted": edges_extracted
                });
                Ok(serde_json::json!({
                    "content": [{"type": "text", "text": serde_json::to_string_pretty(&res)?}],
                    "isError": false
                }))
            }
            "graphrag_query" => {
                let req: GraphRagQueryRequest = serde_json::from_value(arguments)
                    .map_err(|e| McpError::InvalidArguments("graphrag_query".to_string(), e.to_string()))?;
                let (answer, contexts) = state
                    .graph_store
                    .answer_question(&state.pool, &state.llm, &req.question)
                    .await?;
                let res = serde_json::json!({
                    "answer": answer,
                    "contexts": contexts
                });
                Ok(serde_json::json!({
                    "content": [{"type": "text", "text": serde_json::to_string_pretty(&res)?}],
                    "isError": false
                }))
            }
            "document_browse" => {
                let req: DocumentBrowseRequest = serde_json::from_value(arguments)
                    .map_err(|e| McpError::InvalidArguments("document_browse".to_string(), e.to_string()))?;
                let docs = state
                    .graph_store
                    .browse_documents(
                        &state.pool,
                        req.q.as_deref(),
                        req.limit.unwrap_or(50),
                        req.offset.unwrap_or(0),
                    )
                    .await?;
                Ok(serde_json::json!({
                    "content": [{"type": "text", "text": serde_json::to_string_pretty(&docs)?}],
                    "isError": false
                }))
            }
            "document_read" => {
                let req: DocumentReadRequest = serde_json::from_value(arguments)
                    .map_err(|e| McpError::InvalidArguments("document_read".to_string(), e.to_string()))?;
                let doc = state.graph_store.read_document(&state.pool, &req.id).await?;
                Ok(serde_json::json!({
                    "content": [{"type": "text", "text": serde_json::to_string_pretty(&doc)?}],
                    "isError": false
                }))
            }
            "document_edit" => {
                let req: DocumentEditRequest = serde_json::from_value(arguments)
                    .map_err(|e| McpError::InvalidArguments("document_edit".to_string(), e.to_string()))?;
                state
                    .graph_store
                    .edit_document(
                        &state.pool,
                        &state.llm,
                        &req.id,
                        Some(&req.content),
                        req.filename.as_deref(),
                        true,
                    )
                    .await?;
                let updated = state.graph_store.read_document(&state.pool, &req.id).await?;
                Ok(serde_json::json!({
                    "content": [{"type": "text", "text": serde_json::to_string_pretty(&updated)?}],
                    "isError": false
                }))
            }
            "document_add" => {
                let req: DocumentAddRequest = serde_json::from_value(arguments)
                    .map_err(|e| McpError::InvalidArguments("document_add".to_string(), e.to_string()))?;
                let (document_id, chunks_processed, nodes_extracted, edges_extracted) = state
                    .graph_store
                    .ingest_markdown(&state.pool, &state.llm, &req.filename, &req.content)
                    .await?;
                let res = serde_json::json!({
                    "document_id": document_id,
                    "chunks_processed": chunks_processed,
                    "nodes_extracted": nodes_extracted,
                    "edges_extracted": edges_extracted
                });
                Ok(serde_json::json!({
                    "content": [{"type": "text", "text": serde_json::to_string_pretty(&res)?}],
                    "isError": false
                }))
            }
            "document_delete" => {
                let req: DocumentDeleteRequest = serde_json::from_value(arguments)
                    .map_err(|e| McpError::InvalidArguments("document_delete".to_string(), e.to_string()))?;
                state.graph_store.delete_document(&state.pool, &req.id).await;
                Ok(serde_json::json!({
                    "content": [{"type": "text", "text": format!("Document '{}' deleted successfully.", req.id)}],
                    "isError": false
                }))
            }
            "node_browse" => {
                let req: NodeBrowseRequest = serde_json::from_value(arguments)
                    .map_err(|e| McpError::InvalidArguments("node_browse".to_string(), e.to_string()))?;
                let nodes = state
                    .graph_store
                    .browse_nodes(
                        &state.pool,
                        req.label.as_deref(),
                        req.layer,
                        req.document_id.as_deref(),
                        req.q.as_deref(),
                        req.semantic_query.as_deref(),
                        req.limit.unwrap_or(50),
                        req.offset.unwrap_or(0),
                    )
                    .await?;
                Ok(serde_json::json!({
                    "content": [{"type": "text", "text": serde_json::to_string_pretty(&nodes)?}],
                    "isError": false
                }))
            }
            "node_read" => {
                let req: NodeReadRequest = serde_json::from_value(arguments)
                    .map_err(|e| McpError::InvalidArguments("node_read".to_string(), e.to_string()))?;
                let node = state.graph_store.read_node(&state.pool, &req.id).await?;
                Ok(serde_json::json!({
                    "content": [{"type": "text", "text": serde_json::to_string_pretty(&node)?}],
                    "isError": false
                }))
            }
            "node_edit" => {
                let req: NodeEditRequest = serde_json::from_value(arguments)
                    .map_err(|e| McpError::InvalidArguments("node_edit".to_string(), e.to_string()))?;
                state
                    .graph_store
                    .edit_node(
                        &state.pool,
                        &req.id,
                        req.label.as_deref(),
                        req.description.as_deref(),
                        req.layer,
                        req.document_id.as_deref().map(Some),
                    )
                    .await?;
                let updated = state.graph_store.read_node(&state.pool, &req.id).await?;
                Ok(serde_json::json!({
                    "content": [{"type": "text", "text": serde_json::to_string_pretty(&updated)?}],
                    "isError": false
                }))
            }
            "node_add" => {
                let req: NodeAddRequest = serde_json::from_value(arguments)
                    .map_err(|e| McpError::InvalidArguments("node_add".to_string(), e.to_string()))?;
                let node = KnowledgeObject {
                    id: req.id,
                    label: req.label,
                    description: req.description,
                    layer: req.layer.unwrap_or(0),
                    document_id: req.document_id,
                    embedding: vec![],
                };
                let created = state.graph_store.add_node(&state.pool, node).await?;
                Ok(serde_json::json!({
                    "content": [{"type": "text", "text": serde_json::to_string_pretty(&created)?}],
                    "isError": false
                }))
            }
            "node_delete" => {
                let req: NodeDeleteRequest = serde_json::from_value(arguments)
                    .map_err(|e| McpError::InvalidArguments("node_delete".to_string(), e.to_string()))?;
                state.graph_store.delete_node(&state.pool, &req.id).await?;
                Ok(serde_json::json!({
                    "content": [{"type": "text", "text": format!("Node '{}' deleted successfully.", req.id)}],
                    "isError": false
                }))
            }
            "edge_browse" => {
                let req: EdgeBrowseRequest = serde_json::from_value(arguments)
                    .map_err(|e| McpError::InvalidArguments("edge_browse".to_string(), e.to_string()))?;
                let edges = state
                    .graph_store
                    .browse_edges(
                        &state.pool,
                        req.source_id.as_deref(),
                        req.predicate.as_deref(),
                        req.target_id.as_deref(),
                        req.document_id.as_deref(),
                        req.limit.unwrap_or(50),
                        req.offset.unwrap_or(0),
                    )
                    .await?;
                Ok(serde_json::json!({
                    "content": [{"type": "text", "text": serde_json::to_string_pretty(&edges)?}],
                    "isError": false
                }))
            }
            "edge_read" => {
                let req: EdgeReadRequest = serde_json::from_value(arguments)
                    .map_err(|e| McpError::InvalidArguments("edge_read".to_string(), e.to_string()))?;
                let edge = state
                    .graph_store
                    .read_edge(&state.pool, &req.source_id, &req.predicate, &req.target_id)
                    .await?;
                Ok(serde_json::json!({
                    "content": [{"type": "text", "text": serde_json::to_string_pretty(&edge)?}],
                    "isError": false
                }))
            }
            "edge_edit" => {
                let req: EdgeEditRequest = serde_json::from_value(arguments)
                    .map_err(|e| McpError::InvalidArguments("edge_edit".to_string(), e.to_string()))?;
                state
                    .graph_store
                    .edit_edge(
                        &state.pool,
                        &req.source_id,
                        &req.predicate,
                        &req.target_id,
                        req.new_predicate.as_deref(),
                        None,
                        req.document_id.as_deref().map(Some),
                    )
                    .await?;
                let pred = req.new_predicate.unwrap_or(req.predicate);
                let updated = state
                    .graph_store
                    .read_edge(&state.pool, &req.source_id, &pred, &req.target_id)
                    .await?;
                Ok(serde_json::json!({
                    "content": [{"type": "text", "text": serde_json::to_string_pretty(&updated)?}],
                    "isError": false
                }))
            }
            "edge_add" => {
                let req: EdgeAddRequest = serde_json::from_value(arguments)
                    .map_err(|e| McpError::InvalidArguments("edge_add".to_string(), e.to_string()))?;
                let edge = GraphTuple {
                    source: req.source_id,
                    predicate: req.predicate,
                    target: req.target_id,
                    document_id: req.document_id,
                };
                let created = state
                    .graph_store
                    .add_edge(&state.pool, edge, req.auto_create_nodes.unwrap_or(false))
                    .await?;
                Ok(serde_json::json!({
                    "content": [{"type": "text", "text": serde_json::to_string_pretty(&created)?}],
                    "isError": false
                }))
            }
            "edge_delete" => {
                let req: EdgeDeleteRequest = serde_json::from_value(arguments)
                    .map_err(|e| McpError::InvalidArguments("edge_delete".to_string(), e.to_string()))?;
                state
                    .graph_store
                    .delete_edge(&state.pool, &req.source_id, &req.predicate, &req.target_id)
                    .await?;
                Ok(serde_json::json!({
                    "content": [{"type": "text", "text": format!("Edge ({}, {}, {}) deleted successfully.", req.source_id, req.predicate, req.target_id)}],
                    "isError": false
                }))
            }
            "community_browse" => {
                let req: CommunityBrowseRequest = serde_json::from_value(arguments)
                    .map_err(|e| McpError::InvalidArguments("community_browse".to_string(), e.to_string()))?;
                let communities = state
                    .graph_store
                    .browse_communities(
                        &state.pool,
                        req.q.as_deref(),
                        req.member_id.as_deref(),
                        req.limit.unwrap_or(50),
                        req.offset.unwrap_or(0),
                    )
                    .await?;
                Ok(serde_json::json!({
                    "content": [{"type": "text", "text": serde_json::to_string_pretty(&communities)?}],
                    "isError": false
                }))
            }
            "community_read" => {
                let req: CommunityReadRequest = serde_json::from_value(arguments)
                    .map_err(|e| McpError::InvalidArguments("community_read".to_string(), e.to_string()))?;
                let comm = state.graph_store.read_community(&state.pool, &req.id).await?;
                Ok(serde_json::json!({
                    "content": [{"type": "text", "text": serde_json::to_string_pretty(&comm)?}],
                    "isError": false
                }))
            }
            "community_edit" => {
                let req: CommunityEditRequest = serde_json::from_value(arguments)
                    .map_err(|e| McpError::InvalidArguments("community_edit".to_string(), e.to_string()))?;
                state
                    .graph_store
                    .edit_community(&state.pool, &req.id, Some(&req.description))
                    .await?;
                let updated = state.graph_store.read_community(&state.pool, &req.id).await?;
                Ok(serde_json::json!({
                    "content": [{"type": "text", "text": serde_json::to_string_pretty(&updated)?}],
                    "isError": false
                }))
            }
            "community_rebuild" => {
                let _req: CommunityRebuildRequest = serde_json::from_value(arguments)
                    .map_err(|e| McpError::InvalidArguments("community_rebuild".to_string(), e.to_string()))?;
                let summaries = state.graph_store.rebuild_communities(&state.pool).await?;
                Ok(serde_json::json!({
                    "content": [{"type": "text", "text": serde_json::to_string_pretty(&summaries)?}],
                    "isError": false
                }))
            }
            "community_delete" => {
                let req: CommunityDeleteRequest = serde_json::from_value(arguments)
                    .map_err(|e| McpError::InvalidArguments("community_delete".to_string(), e.to_string()))?;
                state.graph_store.delete_community(&state.pool, &req.id).await?;
                Ok(serde_json::json!({
                    "content": [{"type": "text", "text": format!("Community '{}' deleted successfully.", req.id)}],
                    "isError": false
                }))
            }
            _ => Err(McpError::ToolNotFound(name.to_string())),
        }
    }
}

impl Default for GraphRagMcpServer {
    fn default() -> Self {
        Self::new()
    }
}

#[tool_router]
impl GraphRagMcpServer {
    #[tool(description = "Generates a friendly greeting response for a specified user or entity name.")]
    pub fn hello(&self, Parameters(HelloRequest { name }): Parameters<HelloRequest>) -> Result<String, String> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err("Greeting name cannot be empty".to_string());
        }
        Ok(format!("Hello, {}!", trimmed))
    }

    #[tool(description = "Ingests a raw Markdown document into the knowledge graph.")]
    pub fn graphrag_ingest(&self, _params: Parameters<GraphRagIngestRequest>) -> Result<String, String> {
        Ok("Dispatched via call_tool".to_string())
    }

    #[tool(description = "Executes a hybrid Graph RAG query combining semantic similarity, graph neighborhood expansion, and community summaries.")]
    pub fn graphrag_query(&self, _params: Parameters<GraphRagQueryRequest>) -> Result<String, String> {
        Ok("Dispatched via call_tool".to_string())
    }

    #[tool(description = "Lists ingested Markdown documents with pagination and optional query filtering.")]
    pub fn document_browse(&self, _params: Parameters<DocumentBrowseRequest>) -> Result<String, String> {
        Ok("Dispatched via call_tool".to_string())
    }

    #[tool(description = "Retrieves a document by ID with its extracted nodes and edges.")]
    pub fn document_read(&self, _params: Parameters<DocumentReadRequest>) -> Result<String, String> {
        Ok("Dispatched via call_tool".to_string())
    }

    #[tool(description = "Updates an existing Markdown document and refreshes its extracted graph elements.")]
    pub fn document_edit(&self, _params: Parameters<DocumentEditRequest>) -> Result<String, String> {
        Ok("Dispatched via call_tool".to_string())
    }

    #[tool(description = "Adds and ingests a new Markdown document into the knowledge graph.")]
    pub fn document_add(&self, _params: Parameters<DocumentAddRequest>) -> Result<String, String> {
        Ok("Dispatched via call_tool".to_string())
    }

    #[tool(description = "Deletes a document and its associated graph elements.")]
    pub fn document_delete(&self, _params: Parameters<DocumentDeleteRequest>) -> Result<String, String> {
        Ok("Dispatched via call_tool".to_string())
    }

    #[tool(description = "Browses or searches knowledge objects/nodes with optional semantic or label filtering.")]
    pub fn node_browse(&self, _params: Parameters<NodeBrowseRequest>) -> Result<String, String> {
        Ok("Dispatched via call_tool".to_string())
    }

    #[tool(description = "Retrieves a node by canonical ID along with incoming and outgoing edges.")]
    pub fn node_read(&self, _params: Parameters<NodeReadRequest>) -> Result<String, String> {
        Ok("Dispatched via call_tool".to_string())
    }

    #[tool(description = "Updates an existing node and refreshes its vector embedding.")]
    pub fn node_edit(&self, _params: Parameters<NodeEditRequest>) -> Result<String, String> {
        Ok("Dispatched via call_tool".to_string())
    }

    #[tool(description = "Creates or upserts a node, computes its embedding, and updates community summaries.")]
    pub fn node_add(&self, _params: Parameters<NodeAddRequest>) -> Result<String, String> {
        Ok("Dispatched via call_tool".to_string())
    }

    #[tool(description = "Deletes a node and cascades deletion to all incident edges.")]
    pub fn node_delete(&self, _params: Parameters<NodeDeleteRequest>) -> Result<String, String> {
        Ok("Dispatched via call_tool".to_string())
    }

    #[tool(description = "Browses directed graph edges with optional endpoint, predicate, or document filtering.")]
    pub fn edge_browse(&self, _params: Parameters<EdgeBrowseRequest>) -> Result<String, String> {
        Ok("Dispatched via call_tool".to_string())
    }

    #[tool(description = "Retrieves a directed edge by its source, predicate, and target IDs.")]
    pub fn edge_read(&self, _params: Parameters<EdgeReadRequest>) -> Result<String, String> {
        Ok("Dispatched via call_tool".to_string())
    }

    #[tool(description = "Updates a directed edge predicate or origin document ID.")]
    pub fn edge_edit(&self, _params: Parameters<EdgeEditRequest>) -> Result<String, String> {
        Ok("Dispatched via call_tool".to_string())
    }

    #[tool(description = "Adds a directed edge between nodes.")]
    pub fn edge_add(&self, _params: Parameters<EdgeAddRequest>) -> Result<String, String> {
        Ok("Dispatched via call_tool".to_string())
    }

    #[tool(description = "Deletes a directed edge by source, predicate, and target IDs.")]
    pub fn edge_delete(&self, _params: Parameters<EdgeDeleteRequest>) -> Result<String, String> {
        Ok("Dispatched via call_tool".to_string())
    }

    #[tool(description = "Browses hierarchical community summaries with member counts.")]
    pub fn community_browse(&self, _params: Parameters<CommunityBrowseRequest>) -> Result<String, String> {
        Ok("Dispatched via call_tool".to_string())
    }

    #[tool(description = "Retrieves a community summary by ID with member nodes and intra-community edges.")]
    pub fn community_read(&self, _params: Parameters<CommunityReadRequest>) -> Result<String, String> {
        Ok("Dispatched via call_tool".to_string())
    }

    #[tool(description = "Updates community description and recomputes embedding.")]
    pub fn community_edit(&self, _params: Parameters<CommunityEditRequest>) -> Result<String, String> {
        Ok("Dispatched via call_tool".to_string())
    }

    #[tool(description = "Triggers deterministic connected-components clustering and rebuilds community summaries.")]
    pub fn community_rebuild(&self, _params: Parameters<CommunityRebuildRequest>) -> Result<String, String> {
        Ok("Dispatched via call_tool".to_string())
    }

    #[tool(description = "Deletes a community summary node and associated hierarchy edges.")]
    pub fn community_delete(&self, _params: Parameters<CommunityDeleteRequest>) -> Result<String, String> {
        Ok("Dispatched via call_tool".to_string())
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for GraphRagMcpServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_instructions("GraphRAG-RS Model Context Protocol Server")
    }
}
