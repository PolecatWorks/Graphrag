use crate::state::AppState;
use axum::{
    Json, Router,
    extract::{Path, State, Query},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use crate::graph_rag::{GraphRagError, GraphTuple, KnowledgeObject};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct IngestRequest {
    pub filename: String,
    pub content: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DocumentEditRequest {
    pub content: Option<String>,
    pub filename: Option<String>,
    #[serde(default = "default_true")]
    pub re_extract: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize)]
pub struct BrowseQuery {
    pub q: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default = "default_offset")]
    pub offset: usize,
}

fn default_limit() -> usize {
    50
}

fn default_offset() -> usize {
    0
}

#[derive(Debug, Deserialize)]
pub struct BrowseNodeQuery {
    pub label: Option<String>,
    pub layer: Option<i32>,
    pub document_id: Option<String>,
    pub q: Option<String>,
    pub semantic_query: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default = "default_offset")]
    pub offset: usize,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct NodeEditRequest {
    pub label: Option<String>,
    pub description: Option<String>,
    pub layer: Option<i32>,
    pub document_id: Option<Option<String>>,
}

#[derive(Debug, Deserialize)]
pub struct BrowseEdgeQuery {
    pub source: Option<String>,
    pub predicate: Option<String>,
    pub target: Option<String>,
    pub document_id: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default = "default_offset")]
    pub offset: usize,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct EdgeEditRequest {
    pub predicate: Option<String>,
    pub target: Option<String>,
    pub document_id: Option<Option<String>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AddEdgeRequest {
    #[serde(flatten)]
    pub edge: GraphTuple,
    #[serde(default)]
    pub auto_create_nodes: bool,
}

#[derive(Debug, Deserialize)]
pub struct BrowseCommunityQuery {
    pub q: Option<String>,
    pub member: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default = "default_offset")]
    pub offset: usize,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CommunityEditRequest {
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct IngestResponse {
    pub document_id: String,
    pub chunks_processed: usize,
    pub nodes_extracted: usize,
    pub edges_extracted: usize,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct QueryRequest {
    pub question: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct QueryResponse {
    pub answer: String,
    pub contexts: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

pub fn create_router() -> Router<AppState> {
    Router::new()
        .route("/hello", get(hello))
        .route("/ingest", post(ingest_document))
        .route("/query", post(query_graphrag))
        .route("/documents", get(browse_documents).post(add_document))
        .route("/documents/{id}", get(read_document).put(edit_document).delete(delete_document))
        .route("/nodes", get(browse_nodes).post(add_node))
        .route("/nodes/{id}", get(read_node).put(edit_node).delete(delete_node))
        .route("/edges", get(browse_edges).post(add_edge))
        .route("/edges/{source}/{predicate}/{target}", get(read_edge).put(edit_edge).delete(delete_edge))
        .route("/communities", get(browse_communities))
        .route("/communities/rebuild", post(rebuild_communities))
        .route("/communities/{id}", get(read_community).put(edit_community).delete(delete_community))
}
async fn hello() -> &'static str {
    "Hello, World!"
}

async fn ingest_document(
    State(state): State<AppState>,
    Json(payload): Json<IngestRequest>,
) -> impl IntoResponse {
    match state
        .graph_store
        .ingest_markdown(&state.pool, &state.llm, &payload.filename, &payload.content)
        .await
    {
        Ok((document_id, chunks_processed, nodes_extracted, edges_extracted)) => (
            StatusCode::OK,
            Json(IngestResponse {
                document_id,
                chunks_processed,
                nodes_extracted,
                edges_extracted,
            }),
        )
            .into_response(),
        Err(err) => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: err.to_string(),
            }),
        )
            .into_response(),
    }
}

async fn query_graphrag(
    State(state): State<AppState>,
    Json(payload): Json<QueryRequest>,
) -> impl IntoResponse {
    match state
        .graph_store
        .answer_question(&state.pool, &state.llm, &payload.question)
        .await
    {
        Ok((answer, contexts)) => {
            (StatusCode::OK, Json(QueryResponse { answer, contexts })).into_response()
        }
        Err(err) => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: err.to_string(),
            }),
        )
            .into_response(),
    }
}

async fn delete_document(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    state.graph_store.delete_document(&state.pool, &id).await;
    StatusCode::NO_CONTENT
}
fn map_graph_error_to_status(err: &GraphRagError) -> StatusCode {
    match err {
        GraphRagError::DocumentNotFound(_) | GraphRagError::NodeNotFound(_) | GraphRagError::EdgeNotFound { .. } | GraphRagError::CommunityNotFound(_) => StatusCode::NOT_FOUND,
        GraphRagError::EmptyMarkdownContent | GraphRagError::EmptyFilename | GraphRagError::EmptyQuestion | GraphRagError::ZeroMaxChunkChars | GraphRagError::EmptyNodeLabel | GraphRagError::EmptyNodeId | GraphRagError::EmptyPredicate | GraphRagError::InvalidExtractionJson(_) => StatusCode::BAD_REQUEST,
        GraphRagError::Llm(_) | GraphRagError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

async fn browse_documents(State(state): State<AppState>, Query(query): Query<BrowseQuery>) -> impl IntoResponse {
    match state.graph_store.browse_documents(&state.pool, query.q.as_deref(), query.limit, query.offset).await {
        Ok(resp) => (StatusCode::OK, Json(resp)).into_response(),
        Err(err) => { let status = map_graph_error_to_status(&err); (status, Json(ErrorResponse { error: err.to_string() })).into_response() }
    }
}

async fn read_document(State(state): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    match state.graph_store.read_document(&state.pool, &id).await {
        Ok(doc) => (StatusCode::OK, Json(doc)).into_response(),
        Err(err) => { let status = map_graph_error_to_status(&err); (status, Json(ErrorResponse { error: err.to_string() })).into_response() }
    }
}

async fn edit_document(State(state): State<AppState>, Path(id): Path<String>, Json(payload): Json<DocumentEditRequest>) -> impl IntoResponse {
    match state.graph_store.edit_document(&state.pool, &state.llm, &id, payload.content.as_deref(), payload.filename.as_deref(), payload.re_extract).await {
        Ok(_) => StatusCode::OK.into_response(),
        Err(err) => { let status = map_graph_error_to_status(&err); (status, Json(ErrorResponse { error: err.to_string() })).into_response() }
    }
}

async fn add_document(State(state): State<AppState>, Json(payload): Json<IngestRequest>) -> impl IntoResponse {
    match state.graph_store.ingest_markdown(&state.pool, &state.llm, &payload.filename, &payload.content).await {
        Ok((document_id, chunks_processed, nodes_extracted, edges_extracted)) => (StatusCode::CREATED, Json(IngestResponse { document_id, chunks_processed, nodes_extracted, edges_extracted })).into_response(),
        Err(err) => { let status = map_graph_error_to_status(&err); (status, Json(ErrorResponse { error: err.to_string() })).into_response() }
    }
}

async fn browse_nodes(State(state): State<AppState>, Query(query): Query<BrowseNodeQuery>) -> impl IntoResponse {
    match state.graph_store.browse_nodes(&state.pool, query.label.as_deref(), query.layer, query.document_id.as_deref(), query.q.as_deref(), query.semantic_query.as_deref(), query.limit, query.offset).await {
        Ok(resp) => (StatusCode::OK, Json(resp)).into_response(),
        Err(err) => { let status = map_graph_error_to_status(&err); (status, Json(ErrorResponse { error: err.to_string() })).into_response() }
    }
}

async fn read_node(State(state): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    match state.graph_store.read_node(&state.pool, &id).await {
        Ok(val) => (StatusCode::OK, Json(val)).into_response(),
        Err(err) => { let status = map_graph_error_to_status(&err); (status, Json(ErrorResponse { error: err.to_string() })).into_response() }
    }
}

async fn edit_node(State(state): State<AppState>, Path(id): Path<String>, Json(payload): Json<NodeEditRequest>) -> impl IntoResponse {
    match state.graph_store.edit_node(&state.pool, &id, payload.label.as_deref(), payload.description.as_deref(), payload.layer, payload.document_id.as_ref().map(|o| o.as_deref())).await {
        Ok(_) => StatusCode::OK.into_response(),
        Err(err) => { let status = map_graph_error_to_status(&err); (status, Json(ErrorResponse { error: err.to_string() })).into_response() }
    }
}

async fn add_node(State(state): State<AppState>, Json(payload): Json<KnowledgeObject>) -> impl IntoResponse {
    match state.graph_store.add_node(&state.pool, payload).await {
        Ok(_) => StatusCode::CREATED.into_response(),
        Err(err) => { let status = map_graph_error_to_status(&err); (status, Json(ErrorResponse { error: err.to_string() })).into_response() }
    }
}

async fn delete_node(State(state): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    match state.graph_store.delete_node(&state.pool, &id).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => { let status = map_graph_error_to_status(&err); (status, Json(ErrorResponse { error: err.to_string() })).into_response() }
    }
}

async fn browse_edges(State(state): State<AppState>, Query(query): Query<BrowseEdgeQuery>) -> impl IntoResponse {
    match state.graph_store.browse_edges(&state.pool, query.source.as_deref(), query.predicate.as_deref(), query.target.as_deref(), query.document_id.as_deref(), query.limit, query.offset).await {
        Ok(resp) => (StatusCode::OK, Json(resp)).into_response(),
        Err(err) => { let status = map_graph_error_to_status(&err); (status, Json(ErrorResponse { error: err.to_string() })).into_response() }
    }
}

async fn read_edge(State(state): State<AppState>, Path((source, predicate, target)): Path<(String, String, String)>) -> impl IntoResponse {
    match state.graph_store.read_edge(&state.pool, &source, &predicate, &target).await {
        Ok(val) => (StatusCode::OK, Json(val)).into_response(),
        Err(err) => { let status = map_graph_error_to_status(&err); (status, Json(ErrorResponse { error: err.to_string() })).into_response() }
    }
}

async fn edit_edge(State(state): State<AppState>, Path((source, predicate, target)): Path<(String, String, String)>, Json(payload): Json<EdgeEditRequest>) -> impl IntoResponse {
    match state.graph_store.edit_edge(&state.pool, &source, &predicate, &target, payload.predicate.as_deref(), payload.target.as_deref(), payload.document_id.as_ref().map(|o| o.as_deref())).await {
        Ok(_) => StatusCode::OK.into_response(),
        Err(err) => { let status = map_graph_error_to_status(&err); (status, Json(ErrorResponse { error: err.to_string() })).into_response() }
    }
}

async fn add_edge(State(state): State<AppState>, Json(payload): Json<AddEdgeRequest>) -> impl IntoResponse {
    match state.graph_store.add_edge(&state.pool, payload.edge, payload.auto_create_nodes).await {
        Ok(_) => StatusCode::CREATED.into_response(),
        Err(err) => { let status = map_graph_error_to_status(&err); (status, Json(ErrorResponse { error: err.to_string() })).into_response() }
    }
}

async fn delete_edge(State(state): State<AppState>, Path((source, predicate, target)): Path<(String, String, String)>) -> impl IntoResponse {
    match state.graph_store.delete_edge(&state.pool, &source, &predicate, &target).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => { let status = map_graph_error_to_status(&err); (status, Json(ErrorResponse { error: err.to_string() })).into_response() }
    }
}

async fn browse_communities(State(state): State<AppState>, Query(query): Query<BrowseCommunityQuery>) -> impl IntoResponse {
    match state.graph_store.browse_communities(&state.pool, query.q.as_deref(), query.member.as_deref(), query.limit, query.offset).await {
        Ok(resp) => (StatusCode::OK, Json(resp)).into_response(),
        Err(err) => { let status = map_graph_error_to_status(&err); (status, Json(ErrorResponse { error: err.to_string() })).into_response() }
    }
}

async fn read_community(State(state): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    match state.graph_store.read_community(&state.pool, &id).await {
        Ok(val) => (StatusCode::OK, Json(val)).into_response(),
        Err(err) => { let status = map_graph_error_to_status(&err); (status, Json(ErrorResponse { error: err.to_string() })).into_response() }
    }
}

async fn edit_community(State(state): State<AppState>, Path(id): Path<String>, Json(payload): Json<CommunityEditRequest>) -> impl IntoResponse {
    match state.graph_store.edit_community(&state.pool, &id, payload.description.as_deref()).await {
        Ok(_) => StatusCode::OK.into_response(),
        Err(err) => { let status = map_graph_error_to_status(&err); (status, Json(ErrorResponse { error: err.to_string() })).into_response() }
    }
}

async fn rebuild_communities(State(state): State<AppState>) -> impl IntoResponse {
    match state.graph_store.rebuild_communities(&state.pool).await {
        Ok(summaries) => (StatusCode::OK, Json(summaries)).into_response(),
        Err(err) => { let status = map_graph_error_to_status(&err); (status, Json(ErrorResponse { error: err.to_string() })).into_response() }
    }
}

async fn delete_community(State(state): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    match state.graph_store.delete_community(&state.pool, &id).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => { let status = map_graph_error_to_status(&err); (status, Json(ErrorResponse { error: err.to_string() })).into_response() }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;
    use crate::db::DatabaseConfig;
    use crate::graph_rag::GraphStore;
    use crate::llm_tools::LlmConfig;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt; // for `collect`
    use std::sync::Arc;
    use tower::ServiceExt; // for `oneshot`

    fn mock_app_state() -> AppState {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://mock:mock@localhost/mock")
            .unwrap();
        let config = Arc::new(AppConfig {
            database: DatabaseConfig {
                url: "postgres://mock".to_string(),
                pool_size: 1,
                timeout_seconds: 1,
            },
            server_host: "127.0.0.1".to_string(),
            server_port: 8080,
            hams: hams::hams::config::HamsConfig::default(),
            llm: LlmConfig {
                provider: "ollama".to_string(),
                base_url: "http://127.0.0.1:1/v1".to_string(),
                api_key: None,
                model: "llama3".to_string(),
                timeout_seconds: 1,
            },
        });

        AppState {
            pool,
            config: config.clone(),
            llm: Arc::new(crate::llm_tools::LlmClient::new(&config.llm).unwrap()),
            graph_store: GraphStore::default(),
        }
    }

    #[tokio::test]
    async fn test_hello() {
        let app = create_router().with_state(mock_app_state());

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/hello")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(&body[..], b"Hello, World!");
    }

    #[tokio::test]
    async fn test_ingest_and_query_and_delete_endpoints() {
        let state = mock_app_state();
        let app = create_router().with_state(state.clone());

        let ingest_body = serde_json::to_vec(&IngestRequest {
            filename: "04_michael_collins.md".to_string(),
            content: "# Michael Collins\n\nMichael Collins flew the Apollo 11 Command Module Columbia in lunar orbit while Neil Armstrong and Buzz Aldrin landed on the Moon.".to_string(),
        })
        .unwrap();

        let ingest_resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/ingest")
                    .header("content-type", "application/json")
                    .body(Body::from(ingest_body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(ingest_resp.status(), StatusCode::OK);
        let ingest_bytes = ingest_resp.into_body().collect().await.unwrap().to_bytes();
        let ingest_json: IngestResponse = serde_json::from_slice(&ingest_bytes).unwrap();
        assert_eq!(ingest_json.document_id, "04_michael_collins.md");
        assert!(ingest_json.nodes_extracted >= 1);

        let app_query = create_router().with_state(state.clone());
        let query_body = serde_json::to_vec(&QueryRequest {
            question: "Who flew the Apollo 11 Command Module Columbia in lunar orbit?".to_string(),
        })
        .unwrap();

        let query_resp = app_query
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/query")
                    .header("content-type", "application/json")
                    .body(Body::from(query_body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(query_resp.status(), StatusCode::OK);
        let query_bytes = query_resp.into_body().collect().await.unwrap().to_bytes();
        let query_json: QueryResponse = serde_json::from_slice(&query_bytes).unwrap();
        assert!(query_json.answer.contains("Michael Collins"));
        assert!(!query_json.contexts.is_empty());

        let app_delete = create_router().with_state(state);
        let del_resp = app_delete
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/documents/04_michael_collins.md")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(del_resp.status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn test_baseline_markdown_dataset_multi_hop_queries() {
        let state = mock_app_state();
        let baseline_files = [
            "01_apollo_program.md",
            "02_neil_armstrong.md",
            "03_buzz_aldrin.md",
            "04_michael_collins.md",
            "05_john_f_kennedy.md",
        ];

        for filename in baseline_files {
            let path = format!("tests/data/graph_rag_baseline/{}", filename);
            let content = std::fs::read_to_string(&path).expect("baseline markdown file should exist");
            let app = create_router().with_state(state.clone());
            let req_body = serde_json::to_vec(&IngestRequest {
                filename: filename.to_string(),
                content,
            })
            .unwrap();

            let resp = app
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/ingest")
                        .header("content-type", "application/json")
                        .body(Body::from(req_body))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(resp.status(), StatusCode::OK);
        }

        // Question 1: Single-hop Command Module Pilot
        let app_q1 = create_router().with_state(state.clone());
        let q1_body = serde_json::to_vec(&QueryRequest {
            question: "Who flew the Apollo 11 command module Columbia in lunar orbit while Neil Armstrong and Buzz Aldrin landed on the Moon?".to_string(),
        })
        .unwrap();
        let r1 = app_q1
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/query")
                    .header("content-type", "application/json")
                    .body(Body::from(q1_body))
                    .unwrap(),
            )
            .await
            .unwrap();
        let j1: QueryResponse =
            serde_json::from_slice(&r1.into_body().collect().await.unwrap().to_bytes()).unwrap();
        assert!(
            j1.answer.contains("Michael Collins"),
            "Expected Michael Collins in answer, got: {}",
            j1.answer
        );

        // Question 2: Multi-hop JFK + Buzz Aldrin MIT doctorate
        let app_q2 = create_router().with_state(state.clone());
        let q2_body = serde_json::to_vec(&QueryRequest {
            question: "Which US President proposed the national goal in May 1961 that was accomplished on July 20, 1969 by Neil Armstrong and Buzz Aldrin, and what university did the Lunar Module Eagle pilot earn his doctorate from?".to_string(),
        })
        .unwrap();
        let r2 = app_q2
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/query")
                    .header("content-type", "application/json")
                    .body(Body::from(q2_body))
                    .unwrap(),
            )
            .await
            .unwrap();
        let j2: QueryResponse =
            serde_json::from_slice(&r2.into_body().collect().await.unwrap().to_bytes()).unwrap();
        assert!(
            j2.answer.contains("Kennedy"),
            "Expected Kennedy in answer, got: {}",
            j2.answer
        );
        assert!(
            j2.answer.contains("Massachusetts Institute of Technology") || j2.answer.contains("MIT"),
            "Expected MIT in answer, got: {}",
            j2.answer
        );

        // Question 3: Multi-hop West Point graduates (Buzz Aldrin 1951 & Michael Collins 1952)
        let app_q3 = create_router().with_state(state);
        let q3_body = serde_json::to_vec(&QueryRequest {
            question: "Which Apollo 11 astronauts graduated from the United States Military Academy at West Point, and in what years did they graduate?".to_string(),
        })
        .unwrap();
        let r3 = app_q3
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/query")
                    .header("content-type", "application/json")
                    .body(Body::from(q3_body))
                    .unwrap(),
            )
            .await
            .unwrap();
        let j3: QueryResponse =
            serde_json::from_slice(&r3.into_body().collect().await.unwrap().to_bytes()).unwrap();
        assert!(
            j3.answer.contains("1951") && j3.answer.contains("1952"),
            "Expected 1951 and 1952 in answer, got: {}",
            j3.answer
        );
    }
}
