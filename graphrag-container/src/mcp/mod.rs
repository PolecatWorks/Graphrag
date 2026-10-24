use crate::graph_rag::GraphRagError;
use serde::{Deserialize, Serialize};

pub mod rpc;
pub mod server;

#[derive(thiserror::Error, Debug)]
pub enum McpError {
    #[error("Invalid MCP api_prefix '{0}': must start with '/' and not end with '/'")]
    InvalidApiPrefix(String),
    #[error("MCP server_name cannot be empty")]
    EmptyServerName,
    #[error("MCP instructions cannot be empty")]
    EmptyInstructions,
    #[error("Tool not found: '{0}'")]
    ToolNotFound(String),
    #[error("Invalid arguments for tool '{0}': {1}")]
    InvalidArguments(String, String),
    #[error("Greeting name cannot be empty")]
    EmptyGreetingName,
    #[error("Graph RAG error: {0}")]
    GraphRag(#[from] GraphRagError),
    #[error("JSON serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct McpConfig {
    pub enabled: bool,
    pub api_prefix: String,
    pub server_name: String,
    pub instructions: String,
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            api_prefix: "/api".to_string(),
            server_name: "graphrag-mcp".to_string(),
            instructions: "GraphRAG-RS Model Context Protocol Server for knowledge graph ingestion, hybrid RAG querying, and BREAD data management.".to_string(),
        }
    }
}

impl McpConfig {
    pub fn validate(&self) -> Result<(), McpError> {
        if self.enabled {
            if self.api_prefix.trim().is_empty()
                || !self.api_prefix.starts_with('/')
                || (self.api_prefix.len() > 1 && self.api_prefix.ends_with('/'))
            {
                return Err(McpError::InvalidApiPrefix(self.api_prefix.clone()));
            }

            if self.server_name.trim().is_empty() {
                return Err(McpError::EmptyServerName);
            }

            if self.instructions.trim().is_empty() {
                return Err(McpError::EmptyInstructions);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_mcp_config() {
        let config = McpConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_invalid_api_prefix_empty_or_no_slash() {
        let mut config = McpConfig::default();
        config.api_prefix = "".to_string();
        assert!(matches!(config.validate(), Err(McpError::InvalidApiPrefix(_))));

        config.api_prefix = "api".to_string();
        assert!(matches!(config.validate(), Err(McpError::InvalidApiPrefix(_))));

        config.api_prefix = "/api/".to_string();
        assert!(matches!(config.validate(), Err(McpError::InvalidApiPrefix(_))));
    }

    #[test]
    fn test_empty_server_name() {
        let mut config = McpConfig::default();
        config.server_name = "   ".to_string();
        assert!(matches!(config.validate(), Err(McpError::EmptyServerName)));
    }

    #[test]
    fn test_empty_instructions() {
        let mut config = McpConfig::default();
        config.instructions = "   ".to_string();
        assert!(matches!(config.validate(), Err(McpError::EmptyInstructions)));
    }

    #[test]
    fn test_disabled_skips_validation() {
        let mut config = McpConfig::default();
        config.enabled = false;
        config.server_name = "".to_string();
        config.instructions = "".to_string();
        config.api_prefix = "invalid".to_string();
        assert!(config.validate().is_ok());
    }
}
