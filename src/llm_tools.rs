use rig_core::providers::openai::Client;
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

#[derive(Clone, Debug)]
pub struct LlmClient {
    pub client: Client,
    pub model_name: String,
}

impl LlmClient {
    pub fn new(config: &LlmConfig) -> Result<Self, String> {
        let api_key = config.api_key.as_deref().unwrap_or("sk-no-key-required");
        let mut client_builder = rig_core::providers::openai::Client::builder().api_key(api_key);

        let url = config.base_url.trim();
        if !url.is_empty() {
            client_builder = client_builder.base_url(url);
        }

        let rig_client = client_builder.build().map_err(|e| e.to_string())?;

        Ok(Self {
            client: rig_client,
            model_name: config.model.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

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
    fn test_llm_client_initialization() {
        let mut config = valid_llm_config();
        config.api_key = None;

        let client = LlmClient::new(&config);
        assert!(client.is_ok());
        let client = client.unwrap();
        assert_eq!(client.model_name, "llama3");
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

