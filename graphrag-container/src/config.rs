use crate::db::DatabaseConfig;
use crate::llm_tools::LlmConfig;
use crate::logging::{LoggingConfig, LoggingError};
use crate::mcp::{McpConfig, McpError};
use crate::ontology::{OntologyConfig, OntologyError};
use figment::{
    Figment,
    providers::{Env, Format, Yaml},
};
use figment_file_provider_adapter::FileAdapter;
use hams::hams::config::HamsConfig;
use serde::Deserialize;
use std::net::IpAddr;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("server_host must be a valid IP address: {0}")]
    InvalidServerHost(String),
    #[error("server_port must be a valid port number")]
    ZeroServerPort,
    #[error("api_prefix must start with '/' and not end with '/': {0}")]
    InvalidApiPrefix(String),
    #[error(transparent)]
    Database(#[from] crate::db::DatabaseError),
    #[error(transparent)]
    Llm(#[from] crate::llm_tools::LlmError),
    #[error(transparent)]
    Mcp(#[from] McpError),
    #[error(transparent)]
    Logging(#[from] LoggingError),
    #[error(transparent)]
    Ontology(#[from] OntologyError),
}

#[derive(Debug, Deserialize)]
pub struct AppConfig {
    pub api_prefix: String,
    pub database: DatabaseConfig,
    pub server_host: String,
    pub server_port: u16,
    pub hams: HamsConfig,
    pub llm: LlmConfig,
    pub mcp: McpConfig,
    pub logging: LoggingConfig,
    pub ontology: OntologyConfig,
}

impl AppConfig {
    #[allow(clippy::result_large_err)]
    pub fn load(config_path: &Path, secrets_dir: Option<&Path>) -> Result<Self, figment::Error> {
        let mut figment = Figment::new();

        if let Some(s_dir) = secrets_dir {
            let adapter = FileAdapter::wrap(Yaml::file(config_path)).relative_to_dir(s_dir);
            figment = figment.merge(adapter);
        } else {
            figment = figment.merge(Yaml::file(config_path));
        }

        figment = figment.merge(Env::prefixed("GRAPHRAG_BE__").split("__"));

        figment.extract()
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        if !self.api_prefix.starts_with('/')
            || (self.api_prefix.len() > 1 && self.api_prefix.ends_with('/'))
        {
            return Err(ConfigError::InvalidApiPrefix(self.api_prefix.clone()));
        }

        if self.server_host.parse::<IpAddr>().is_err() {
            return Err(ConfigError::InvalidServerHost(self.server_host.clone()));
        }

        self.database.validate()?;
        self.llm.validate()?;
        self.mcp.validate()?;
        self.logging.validate()?;
        self.ontology.validate()?;

        if self.server_port == 0 {
            return Err(ConfigError::ZeroServerPort);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hams::hams::config::HamsConfig;
    use std::env;
    use std::io::Write;
    use std::sync::Mutex;
    use tempfile::NamedTempFile;
    use url::Url;

    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    const BASE_YAML_SECTIONS: &str = r#"api_prefix: "/api/graphrag/v1"
hams:
  address: "127.0.0.1:8079"
  name: "graphrag"
  version: "0.1.0"
  logging: true
mcp:
  enabled: true
  server_name: "graphrag-mcp"
  instructions: "GraphRAG-RS Model Context Protocol Server for knowledge graph ingestion, hybrid RAG querying, and BREAD data management."
logging:
  level: "info"
ontology:
  subgraph:
    default_hops: 2
    max_hops: 5
    default_max_nodes: 250
    default_max_edges: 500"#;

    fn clear_env() {
        unsafe {
            env::remove_var("GRAPHRAG_BE__API_PREFIX");
            env::remove_var("GRAPHRAG_BE__DATABASE__URL");
            env::remove_var("GRAPHRAG_BE__DATABASE__POOL_SIZE");
            env::remove_var("GRAPHRAG_BE__DATABASE__TIMEOUT_SECONDS");
            env::remove_var("GRAPHRAG_BE__SERVER_PORT");
            env::remove_var("GRAPHRAG_BE__HAMS__PORT");
            env::remove_var("GRAPHRAG_BE__LOGGING__LEVEL");
            env::remove_var("GRAPHRAG_BE__LOGGING__FILTER");
        }
    }

    fn valid_app_config() -> AppConfig {
        AppConfig {
            api_prefix: "/api/graphrag/v1".to_string(),
            database: DatabaseConfig {
                url: crate::db::DatabaseUrlConfig::new("postgres://test"),
                pool_size: 10,
                timeout_seconds: 30,
            },
            server_host: "127.0.0.1".to_string(),
            server_port: 8080,
            hams: HamsConfig::default(),
            llm: LlmConfig {
                provider: "ollama".to_string(),
                base_url: Url::parse("http://localhost:11434/v1").unwrap(),
                api_key: None,
                model: "llama3".to_string(),
                timeout_seconds: 60,
            },
            mcp: McpConfig::default(),
            logging: LoggingConfig::default(),
            ontology: OntologyConfig::default(),
        }
    }

    #[test]
    fn test_valid_config_from_yaml() {
        let _lock = ENV_MUTEX.lock().unwrap();
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            "database:\n  url: postgres://yaml\n  pool_size: 10\n  timeout_seconds: 30\nserver_host: 127.0.0.1\nserver_port: 8081\nllm:\n  provider: ollama\n  base_url: http://localhost:11434/v1\n  model: llama3\n  timeout_seconds: 60\n{BASE_YAML_SECTIONS}"
        )
        .unwrap();

        clear_env();

        let config = AppConfig::load(file.path(), None).unwrap();
        assert_eq!(
            config.database.url.connection_url().unwrap().as_str(),
            "postgres://yaml"
        );
        assert_eq!(config.database.pool_size, 10);
        assert_eq!(config.database.timeout_seconds, 30);
        assert_eq!(config.server_port, 8081);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_repository_config_files_load_and_validate() {
        let _lock = ENV_MUTEX.lock().unwrap();
        clear_env();

        for rel_path in ["config.yaml", "../charts/graphrag/configs/config.yaml"] {
            let path = Path::new(rel_path);
            let config = AppConfig::load(path, None)
                .unwrap_or_else(|e| panic!("Failed to load {rel_path}: {e}"));
            config
                .validate()
                .unwrap_or_else(|e| panic!("Failed to validate {rel_path}: {e}"));
        }
    }

    #[test]
    fn test_config_load_with_secrets_dir() {
        let _lock = ENV_MUTEX.lock().unwrap();
        let secrets_dir = tempfile::tempdir().unwrap();
        let conn_dir = secrets_dir.path().join("persistence/db/connection");
        std::fs::create_dir_all(&conn_dir).unwrap();

        std::fs::write(conn_dir.join("username"), "testuser").unwrap();
        std::fs::write(conn_dir.join("password"), "testpass").unwrap();

        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            "database:\n  url:\n    url: postgres://localhost:5432/graphrag\n    username_file: persistence/db/connection/username\n    password_file: persistence/db/connection/password\n  pool_size: 5\n  timeout_seconds: 10\nserver_host: 127.0.0.1\nserver_port: 8080\nllm:\n  provider: ollama\n  base_url: http://localhost:11434/v1\n  model: llama3\n  timeout_seconds: 60\n{BASE_YAML_SECTIONS}"
        )
        .unwrap();

        clear_env();

        let config = AppConfig::load(file.path(), Some(secrets_dir.path())).unwrap();

        assert_eq!(
            config.database.url.connection_url().unwrap().as_str(),
            "postgres://testuser:testpass@localhost:5432/graphrag"
        );
        assert_eq!(config.database.pool_size, 5);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_override_from_env() {
        let _lock = ENV_MUTEX.lock().unwrap();
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            "database:\n  url: postgres://yaml\n  pool_size: 10\n  timeout_seconds: 30\nserver_host: 127.0.0.1\nserver_port: 8081\nllm:\n  provider: ollama\n  base_url: http://localhost:11434/v1\n  model: llama3\n  timeout_seconds: 60\n{BASE_YAML_SECTIONS}"
        )
        .unwrap();

        clear_env();

        unsafe {
            env::set_var("GRAPHRAG_BE__DATABASE__URL", "postgres://env");
            env::set_var("GRAPHRAG_BE__DATABASE__POOL_SIZE", "20");
            env::set_var("GRAPHRAG_BE__DATABASE__TIMEOUT_SECONDS", "60");
            env::set_var("GRAPHRAG_BE__SERVER_PORT", "9090");
        }

        let config = AppConfig::load(file.path(), None).unwrap();

        clear_env();

        assert_eq!(
            config.database.url.connection_url().unwrap().as_str(),
            "postgres://env"
        );
        assert_eq!(config.database.pool_size, 20);
        assert_eq!(config.database.timeout_seconds, 60);
        assert_eq!(config.server_port, 9090);
    }

    #[test]
    fn test_config_logging_override_from_env() {
        let _lock = ENV_MUTEX.lock().unwrap();
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            "database:\n  url: postgres://yaml\n  pool_size: 10\n  timeout_seconds: 30\nserver_host: 127.0.0.1\nserver_port: 8081\nllm:\n  provider: ollama\n  base_url: http://localhost:11434/v1\n  model: llama3\n  timeout_seconds: 60\n{BASE_YAML_SECTIONS}"
        )
        .unwrap();

        clear_env();

        unsafe {
            env::set_var("GRAPHRAG_BE__LOGGING__LEVEL", "debug");
            env::set_var("GRAPHRAG_BE__LOGGING__FILTER", "debug,app=trace");
        }

        let config = AppConfig::load(file.path(), None).unwrap();

        clear_env();

        assert_eq!(config.logging.level, "debug");
        assert_eq!(config.logging.filter.as_deref(), Some("debug,app=trace"));
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_delegates_to_logging_config() {
        let mut config = valid_app_config();
        config.logging.level = "invalid_log_level".to_string();
        let res = config.validate();
        assert!(res.is_err());
        assert!(matches!(res.unwrap_err(), ConfigError::Logging(_)));
    }

    #[test]
    fn test_validate_delegates_to_mcp_config() {
        let mut config = valid_app_config();
        config.mcp.server_name = "".to_string();
        let res = config.validate();
        assert!(matches!(res, Err(ConfigError::Mcp(McpError::EmptyServerName))));
    }

    #[test]
    fn test_validate_invalid_server_host() {
        let mut config = valid_app_config();
        config.server_host = "not-an-ip".to_string();

        let result = config.validate();
        assert!(matches!(result, Err(ConfigError::InvalidServerHost(_))));
    }

    #[test]
    fn test_validate_zero_server_port() {
        let mut config = valid_app_config();
        config.server_port = 0;

        let result = config.validate();
        assert!(matches!(result, Err(ConfigError::ZeroServerPort)));
        assert_eq!(
            result.unwrap_err().to_string(),
            "server_port must be a valid port number"
        );
    }

    #[test]
    fn test_validate_delegates_to_database_config() {
        let mut config = valid_app_config();
        config.database.url = crate::db::DatabaseUrlConfig::new("   ");

        let result = config.validate();
        assert!(matches!(
            result,
            Err(ConfigError::Database(crate::db::DatabaseError::EmptyUrl))
        ));
        assert_eq!(
            result.unwrap_err().to_string(),
            "database.url must not be empty"
        );
    }

    #[test]
    fn test_validate_delegates_to_llm_config() {
        let mut config = valid_app_config();
        config.llm.provider = "   ".to_string();

        let result = config.validate();
        assert!(matches!(
            result,
            Err(ConfigError::Llm(crate::llm_tools::LlmError::EmptyProvider))
        ));
        assert_eq!(
            result.unwrap_err().to_string(),
            "llm.provider must not be empty"
        );
    }

    #[test]
    fn test_api_prefix_validation() {
        let mut config = valid_app_config();
        config.api_prefix = "api/graphrag/v1".to_string();
        let res = config.validate();
        assert!(matches!(res, Err(ConfigError::InvalidApiPrefix(_))));

        config.api_prefix = "/api/graphrag/v1/".to_string();
        let res = config.validate();
        assert!(matches!(res, Err(ConfigError::InvalidApiPrefix(_))));

        config.api_prefix = "/".to_string();
        assert!(config.validate().is_ok());

        config.api_prefix = "/custom/prefix".to_string();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_api_prefix_override_from_env() {
        let _lock = ENV_MUTEX.lock().unwrap();
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            "database:\n  url: postgres://yaml\n  pool_size: 10\n  timeout_seconds: 30\nserver_host: 127.0.0.1\nserver_port: 8081\nllm:\n  provider: ollama\n  base_url: http://localhost:11434/v1\n  model: llama3\n  timeout_seconds: 60\n{BASE_YAML_SECTIONS}"
        )
        .unwrap();

        clear_env();

        unsafe {
            env::set_var("GRAPHRAG_BE__API_PREFIX", "/api/custom/v1");
        }

        let config = AppConfig::load(file.path(), None).unwrap();

        clear_env();

        assert_eq!(config.api_prefix, "/api/custom/v1");
        assert!(config.validate().is_ok());
    }
}


