use crate::config::AppConfig;
use crate::graph_rag::GraphStore;
use crate::llm_tools::LlmClient;
use crate::mcp::server::GraphRagMcpServer;
use sqlx::PgPool;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct AppState {
    pub pool: PgPool,
    pub config: Arc<AppConfig>,
    pub llm: Arc<LlmClient>,
    pub graph_store: GraphStore,
    pub mcp_server: Arc<GraphRagMcpServer>,
}

impl axum::extract::FromRef<AppState> for PgPool {
    fn from_ref(state: &AppState) -> Self {
        state.pool.clone()
    }
}

impl axum::extract::FromRef<AppState> for Arc<AppConfig> {
    fn from_ref(state: &AppState) -> Self {
        state.config.clone()
    }
}

impl axum::extract::FromRef<AppState> for Arc<LlmClient> {
    fn from_ref(state: &AppState) -> Self {
        state.llm.clone()
    }
}

impl axum::extract::FromRef<AppState> for GraphStore {
    fn from_ref(state: &AppState) -> Self {
        state.graph_store.clone()
    }
}
