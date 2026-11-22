use rig_core::client::CompletionClient;
use rig_core::completion::{AssistantContent, CompletionError, CompletionModel};
use rig_core::providers::openai::Client;
use serde::Deserialize;
use std::time::Duration;
use thiserror::Error;
use url::Url;

#[derive(Debug, Error)]
pub enum LlmError {
    #[error("llm.provider must not be empty")]
    EmptyProvider,
    #[error("llm.base_url must be an HTTP(S) URL with a host: {0}")]
    InvalidBaseUrl(String),
    #[error("llm.model must not be empty")]
    EmptyModel,
    #[error("llm.timeout_seconds must be greater than 0")]
    ZeroTimeout,
    #[error("LLM request timed out after {0}s")]
    RequestTimeout(u64),
    #[error(transparent)]
    ClientInit(#[from] rig_core::http_client::Error),
    #[error(transparent)]
    CompletionFailed(#[from] CompletionError),
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
pub struct LlmConfig {
    pub provider: String,
    pub base_url: Url,
    pub api_key: Option<String>,
    pub model: String,
    pub timeout_seconds: u64,
}

impl LlmConfig {
    pub fn validate(&self) -> Result<(), LlmError> {
        if self.provider.trim().is_empty() {
            return Err(LlmError::EmptyProvider);
        }
        if !matches!(self.base_url.scheme(), "http" | "https") || self.base_url.host_str().is_none()
        {
            return Err(LlmError::InvalidBaseUrl(self.base_url.to_string()));
        }
        if self.model.trim().is_empty() {
            return Err(LlmError::EmptyModel);
        }
        if self.timeout_seconds == 0 {
            return Err(LlmError::ZeroTimeout);
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct LlmClient {
    pub client: Client,
    pub model_name: String,
    pub timeout: Duration,
}

impl LlmClient {
    pub fn new(config: &LlmConfig) -> Result<Self, LlmError> {
        config.validate()?;

        let api_key = config.api_key.as_deref().unwrap_or("sk-no-key-required");
        let rig_client = rig_core::providers::openai::Client::builder()
            .api_key(api_key)
            .base_url(config.base_url.as_str())
            .build()?;

        Ok(Self {
            client: rig_client,
            model_name: config.model.clone(),
            timeout: Duration::from_secs(config.timeout_seconds),
        })
    }

    pub async fn prompt(&self, preamble: &str, user_prompt: &str) -> Result<String, LlmError> {
        let model = self.client.completion_model(&self.model_name);
        let request = model
            .completion_request(user_prompt)
            .preamble(preamble.to_string())
            .build();
        let timeout_secs = self.timeout.as_secs();
        let response = tokio::time::timeout(self.timeout, model.completion(request))
            .await
            .map_err(|_| LlmError::RequestTimeout(timeout_secs))??;
        let text = response
            .choice
            .iter()
            .filter_map(|item| match item {
                AssistantContent::Text(t) => Some(t.text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn valid_llm_config() -> LlmConfig {
        LlmConfig {
            provider: "ollama".to_string(),
            base_url: Url::parse("http://localhost:11434/v1").unwrap(),
            api_key: Some("test-key".to_string()),
            model: "llama3".to_string(),
            timeout_seconds: 60,
        }
    }

    #[test]
    fn test_llm_client_initialization() {
        let mut config = valid_llm_config();
        config.api_key = None;

        let client = LlmClient::new(&config);
        assert!(client.is_ok());
        let client = client.unwrap();
        assert_eq!(client.model_name, "llama3");
        assert_eq!(client.timeout, Duration::from_secs(60));
    }

    #[test]
    fn test_llm_config_validate_success() {
        let config = valid_llm_config();
        assert!(config.validate().is_ok());
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
        assert!(matches!(result, Err(LlmError::EmptyProvider)));
        assert_eq!(
            result.unwrap_err().to_string(),
            "llm.provider must not be empty"
        );
    }

    #[test]
    fn test_llm_config_validate_invalid_base_url_scheme() {
        let mut config = valid_llm_config();
        config.base_url = Url::parse("ftp://localhost:11434/v1").unwrap();

        let result = config.validate();
        assert!(matches!(result, Err(LlmError::InvalidBaseUrl(_))));
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
        assert!(matches!(result, Err(LlmError::EmptyModel)));
        assert_eq!(
            result.unwrap_err().to_string(),
            "llm.model must not be empty"
        );
    }

    #[test]
    fn test_llm_config_validate_zero_timeout() {
        let mut config = valid_llm_config();
        config.timeout_seconds = 0;

        let result = config.validate();
        assert!(matches!(result, Err(LlmError::ZeroTimeout)));
        assert_eq!(
            result.unwrap_err().to_string(),
            "llm.timeout_seconds must be greater than 0"
        );
    }
}



