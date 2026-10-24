use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LoggingError {
    #[error("Invalid log level '{0}': must be one of trace, debug, info, warn, error")]
    InvalidLogLevel(String),
    #[error("Log filter cannot be empty if specified")]
    EmptyFilter,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct LoggingConfig {
    #[serde(default = "default_log_level")]
    pub level: String,
    pub filter: Option<String>,
}

fn default_log_level() -> String {
    "info".to_string()
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            filter: None,
        }
    }
}

impl LoggingConfig {
    pub fn validate(&self) -> Result<(), LoggingError> {
        let level_lower = self.level.trim().to_lowercase();
        match level_lower.as_str() {
            "trace" | "debug" | "info" | "warn" | "error" => {}
            _ => return Err(LoggingError::InvalidLogLevel(self.level.clone())),
        }

        if let Some(ref filter) = self.filter {
            if filter.trim().is_empty() {
                return Err(LoggingError::EmptyFilter);
            }
        }

        Ok(())
    }

    pub fn filter_directive(&self) -> String {
        if let Some(ref filter) = self.filter {
            filter.clone()
        } else {
            let lvl = self.level.trim().to_lowercase();
            format!("{},app={lvl},tower_http={lvl}", lvl)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_default_logging_config() {
        let cfg = LoggingConfig::default();
        assert!(cfg.validate().is_ok());
        assert_eq!(cfg.level, "info");
        assert_eq!(cfg.filter_directive(), "info,app=info,tower_http=info");
    }

    #[test]
    fn test_valid_custom_log_levels() {
        for lvl in &["trace", "debug", "info", "warn", "error", "DEBUG", "Info"] {
            let cfg = LoggingConfig {
                level: lvl.to_string(),
                filter: None,
            };
            assert!(cfg.validate().is_ok());
        }
    }

    #[test]
    fn test_invalid_log_level() {
        let cfg = LoggingConfig {
            level: "invalid_level".to_string(),
            filter: None,
        };
        assert_eq!(
            cfg.validate(),
            Err(LoggingError::InvalidLogLevel("invalid_level".to_string()))
        );
    }

    #[test]
    fn test_empty_filter_error() {
        let cfg = LoggingConfig {
            level: "debug".to_string(),
            filter: Some("   ".to_string()),
        };
        assert_eq!(cfg.validate(), Err(LoggingError::EmptyFilter));
    }

    #[test]
    fn test_custom_filter_directive() {
        let cfg = LoggingConfig {
            level: "debug".to_string(),
            filter: Some("debug,app=trace".to_string()),
        };
        assert!(cfg.validate().is_ok());
        assert_eq!(cfg.filter_directive(), "debug,app=trace");
    }
}
