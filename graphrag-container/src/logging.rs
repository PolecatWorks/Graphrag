use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LoggingError {
    #[error("Invalid log level '{0}': must be one of trace, debug, info, warn, error")]
    InvalidLogLevel(String),
    #[error("Log filter cannot be empty if specified")]
    EmptyFilter,
    #[error("Invalid log filter '{directive}': {reason}")]
    InvalidFilter { directive: String, reason: String },
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct LoggingConfig {
    pub level: String,
    pub filter: Option<String>,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
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

        self.build_env_filter()?;

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

    pub fn build_env_filter(&self) -> Result<EnvFilter, LoggingError> {
        let directive = self.filter_directive();
        EnvFilter::try_new(&directive).map_err(|err| LoggingError::InvalidFilter {
            directive,
            reason: err.to_string(),
        })
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

    #[test]
    fn test_invalid_env_filter_rejected() {
        for bad_filter in ["app[unclosed", "app=not_a_valid_level=extra", "[unclosed"] {
            let cfg = LoggingConfig {
                level: "info".to_string(),
                filter: Some(bad_filter.to_string()),
            };
            let err = cfg.validate().expect_err(&format!(
                "Expected invalid EnvFilter '{}' to fail validation",
                bad_filter
            ));
            assert!(
                matches!(err, LoggingError::InvalidFilter { ref directive, .. } if directive == bad_filter),
                "Expected LoggingError::InvalidFilter for '{}', got: {:?}",
                bad_filter,
                err
            );
        }
    }
}
