use crate::db::DatabaseConfig;
use figment::{
    Figment,
    providers::{Env, Format, Yaml},
};
use hams::hams::config::HamsConfig;
use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("llm.provider must not be empty")]
    EmptyLlmProvider,
    #[error("llm.base_url must not be empty")]
    EmptyLlmBaseUrl,
    #[error("llm.model must not be empty")]
    EmptyLlmModel,
    #[error("llm.timeout_seconds must be greater than 0")]
    ZeroLlmTimeout,
    #[error("server_port must be a valid port number")]
    ZeroServerPort,
    #[error(transparent)]
    Database(#[from] crate::db::DatabaseError),
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
pub struct LlmConfig {
    pub provider: String,
    pub base_url: String,
    pub api_key: Option<String>,
    pub model: String,
    pub timeout_seconds: u64,
}

impl LlmConfig {
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.provider.trim().is_empty() {
            return Err(ConfigError::EmptyLlmProvider);
        }
        if self.base_url.trim().is_empty() {
            return Err(ConfigError::EmptyLlmBaseUrl);
        }
        if self.model.trim().is_empty() {
            return Err(ConfigError::EmptyLlmModel);
        }
        if self.timeout_seconds == 0 {
            return Err(ConfigError::ZeroLlmTimeout);
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub struct AppConfig {
    pub database: DatabaseConfig,
    pub server_host: String,
    pub server_port: u16,
    #[serde(default)]
    pub hams: HamsConfig,
    pub llm: LlmConfig,
}

impl AppConfig {
    #[allow(clippy::result_large_err)]
    pub fn load(config_path: Option<&str>) -> Result<Self, figment::Error> {
        let mut figment = Figment::new();

        if let Some(path) = config_path {
            figment = figment.merge(Yaml::file(path));
        }

        figment = figment.merge(Env::prefixed("GRAPHRAG_BE__").split("__"));

        figment.extract()
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        self.database.validate()?;
        self.llm.validate()?;

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

    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    fn clear_env() {
        unsafe {
            env::remove_var("GRAPHRAG_BE__DATABASE__URL");
            env::remove_var("GRAPHRAG_BE__DATABASE__POOL_SIZE");
            env::remove_var("GRAPHRAG_BE__DATABASE__TIMEOUT_SECONDS");
            env::remove_var("GRAPHRAG_BE__SERVER_PORT");
            env::remove_var("GRAPHRAG_BE__HAMS__PORT");
        }
    }

    #[test]
    fn test_valid_config_from_yaml() {
        let _lock = ENV_MUTEX.lock().unwrap();
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            "database:\n  url: postgres://yaml\n  pool_size: 10\n  timeout_seconds: 30\nserver_host: 127.0.0.1\nserver_port: 8081\nllm:\n  provider: ollama\n  base_url: http://localhost:11434/v1\n  model: llama3\n  timeout_seconds: 60"
        )
        .unwrap();

        clear_env();

        let config = AppConfig::load(Some(file.path().to_str().unwrap())).unwrap();
        assert_eq!(config.database.url, "postgres://yaml");
        assert_eq!(config.database.pool_size, 10);
        assert_eq!(config.database.timeout_seconds, 30);
        assert_eq!(config.server_port, 8081);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_override_from_env() {
        let _lock = ENV_MUTEX.lock().unwrap();
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            "database:\n  url: postgres://yaml\n  pool_size: 10\n  timeout_seconds: 30\nserver_host: 127.0.0.1\nserver_port: 8081\nllm:\n  provider: ollama\n  base_url: http://localhost:11434/v1\n  model: llama3\n  timeout_seconds: 60"
        )
        .unwrap();

        clear_env();

        unsafe {
            env::set_var("GRAPHRAG_BE__DATABASE__URL", "postgres://env");
            env::set_var("GRAPHRAG_BE__DATABASE__POOL_SIZE", "20");
            env::set_var("GRAPHRAG_BE__DATABASE__TIMEOUT_SECONDS", "60");
            env::set_var("GRAPHRAG_BE__SERVER_PORT", "9090");
        }

        let config = AppConfig::load(Some(file.path().to_str().unwrap())).unwrap();

        clear_env();

        assert_eq!(config.database.url, "postgres://env");
        assert_eq!(config.database.pool_size, 20);
        assert_eq!(config.database.timeout_seconds, 60);
        assert_eq!(config.server_port, 9090);
    }

    #[test]
    fn test_validate_empty_database_url() {
        let config = AppConfig {
            database: DatabaseConfig {
                url: "   ".to_string(),
                pool_size: 10,
                timeout_seconds: 30,
            },
            server_host: "127.0.0.1".to_string(),
            server_port: 8080,
            hams: HamsConfig::default(),
            llm: LlmConfig {
                provider: "ollama".to_string(),
                base_url: "http://localhost:11434/v1".to_string(),
                api_key: None,
                model: "llama3".to_string(),
                timeout_seconds: 60,
            },
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_zero_server_port() {
        let config = AppConfig {
            database: DatabaseConfig {
                url: "postgres://test".to_string(),
                pool_size: 10,
                timeout_seconds: 30,
            },
            server_host: "127.0.0.1".to_string(),
            server_port: 0,
            hams: HamsConfig::default(),
            llm: LlmConfig {
                provider: "ollama".to_string(),
                base_url: "http://localhost:11434/v1".to_string(),
                api_key: None,
                model: "llama3".to_string(),
                timeout_seconds: 60,
            },
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_zero_pool_size() {
        let config = AppConfig {
            database: DatabaseConfig {
                url: "postgres://test".to_string(),
                pool_size: 0,
                timeout_seconds: 30,
            },
            server_host: "127.0.0.1".to_string(),
            server_port: 8080,
            hams: HamsConfig::default(),
            llm: LlmConfig {
                provider: "ollama".to_string(),
                base_url: "http://localhost:11434/v1".to_string(),
                api_key: None,
                model: "llama3".to_string(),
                timeout_seconds: 60,
            },
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_zero_timeout() {
        let config = AppConfig {
            database: DatabaseConfig {
                url: "postgres://test".to_string(),
                pool_size: 10,
                timeout_seconds: 0,
            },
            server_host: "127.0.0.1".to_string(),
            server_port: 8080,
            hams: HamsConfig::default(),
            llm: LlmConfig {
                provider: "ollama".to_string(),
                base_url: "http://localhost:11434/v1".to_string(),
                api_key: None,
                model: "llama3".to_string(),
                timeout_seconds: 60,
            },
        };
        assert!(config.validate().is_err());
    }
}
