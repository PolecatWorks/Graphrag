use crate::db::DatabaseConfig;
use figment::{
    Figment,
    providers::{Env, Format, Yaml},
};
use hams::hams::config::HamsConfig;
use serde::Deserialize;

#[derive(Debug, Deserialize, PartialEq, Clone)]
pub struct LlmConfig {
    pub provider: String,
    pub base_url: String,
    pub api_key: Option<String>,
    pub model: String,
    pub timeout_seconds: u64,
}

impl LlmConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.provider.trim().is_empty() {
            return Err("llm.provider must not be empty".to_string());
        }
        if self.base_url.trim().is_empty() {
            return Err("llm.base_url must not be empty".to_string());
        }
        if self.model.trim().is_empty() {
            return Err("llm.model must not be empty".to_string());
        }
        if self.timeout_seconds == 0 {
            return Err("llm.timeout_seconds must be greater than 0".to_string());
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

    pub fn validate(&self) -> Result<(), String> {
        self.database.validate()?;
        self.llm.validate()?;

        if self.server_port == 0 {
            return Err("server_port must be a valid port number".to_string());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hams::hams::config::HamsConfig;
    use rstest::rstest;
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

    fn valid_llm_config() -> LlmConfig {
        LlmConfig {
            provider: "ollama".to_string(),
            base_url: "http://localhost:11434/v1".to_string(),
            api_key: Some("test-key".to_string()),
            model: "llama3".to_string(),
            timeout_seconds: 60,
        }
    }

    #[test]
    fn test_llm_config_validate_success() {
        let config = valid_llm_config();
        assert_eq!(config.validate(), Ok(()));
    }

    #[rstest]
    #[case("")]
    #[case(" ")]
    #[case("  ")]
    #[case("\t")]
    #[case("\n")]
    #[case(" \t\n ")]
    fn test_llm_config_validate_empty_provider(#[case] provider: &str) {
        let mut config = valid_llm_config();
        config.provider = provider.to_string();

        let result = config.validate();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "llm.provider must not be empty");
    }

    #[rstest]
    #[case("")]
    #[case(" ")]
    #[case("  ")]
    #[case("\t")]
    #[case("\n")]
    #[case(" \t\n ")]
    fn test_llm_config_validate_empty_base_url(#[case] base_url: &str) {
        let mut config = valid_llm_config();
        config.base_url = base_url.to_string();

        let result = config.validate();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "llm.base_url must not be empty");
    }

    #[rstest]
    #[case("")]
    #[case(" ")]
    #[case("  ")]
    #[case("\t")]
    #[case("\n")]
    #[case(" \t\n ")]
    fn test_llm_config_validate_empty_model(#[case] model: &str) {
        let mut config = valid_llm_config();
        config.model = model.to_string();

        let result = config.validate();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "llm.model must not be empty");
    }

    #[test]
    fn test_llm_config_validate_zero_timeout() {
        let mut config = valid_llm_config();
        config.timeout_seconds = 0;

        let result = config.validate();
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "llm.timeout_seconds must be greater than 0"
        );
    }
}
