use serde::{Deserialize, Deserializer, Serialize};
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::time::Duration;
use url::Url;

#[derive(Debug, thiserror::Error)]
pub enum DatabaseError {
    #[error("database.url must not be empty")]
    EmptyUrl,
    #[error("database.url is not a valid URL: {0}")]
    InvalidUrl(#[from] url::ParseError),
    #[error("failed to set database username on database.url")]
    SetUsernameFailed,
    #[error("failed to set database password on database.url")]
    SetPasswordFailed,
    #[error("database.pool_size must be greater than 0")]
    ZeroPoolSize,
    #[error("database.timeout_seconds must be greater than 0")]
    ZeroTimeout,
    #[error("Failed to connect to database: {0}")]
    ConnectionFailed(#[source] sqlx::Error),
    #[error("Failed to query pg_extension: {0}")]
    ExtensionQueryFailed(#[source] sqlx::Error),
    #[error(
        "pgvector extension is not enabled in the database. Please ensure 'CREATE EXTENSION vector;' has been run."
    )]
    PgVectorNotEnabled,
    #[error("Failed to run database migrations: {0}")]
    MigrationFailed(#[from] sqlx::migrate::MigrateError),
}

/// A structured database URL configuration supporting file-based secret resolution.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct DatabaseUrlConfig {
    pub url: String,
    pub username: Option<String>,
    pub password: Option<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum DatabaseUrlHelper {
    Structured {
        url: String,
        username: Option<String>,
        password: Option<String>,
    },
    Simple(String),
}

impl<'de> Deserialize<'de> for DatabaseUrlConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let helper = DatabaseUrlHelper::deserialize(deserializer)?;
        match helper {
            DatabaseUrlHelper::Structured {
                url,
                username,
                password,
            } => Ok(Self {
                url,
                username,
                password,
            }),
            DatabaseUrlHelper::Simple(url) => Ok(Self {
                url,
                username: None,
                password: None,
            }),
        }
    }
}

impl DatabaseUrlConfig {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            username: None,
            password: None,
        }
    }

    /// Assembles and parses the final connection URL, incorporating username and password if set.
    pub fn connection_url(&self) -> Result<Url, DatabaseError> {
        let trimmed = self.url.trim();
        if trimmed.is_empty() {
            return Err(DatabaseError::EmptyUrl);
        }

        let mut parsed = Url::parse(trimmed)?;
        if let Some(user) = &self.username {
            parsed
                .set_username(user)
                .map_err(|()| DatabaseError::SetUsernameFailed)?;
        }
        if let Some(pass) = &self.password {
            parsed
                .set_password(Some(pass))
                .map_err(|()| DatabaseError::SetPasswordFailed)?;
        }
        Ok(parsed)
    }
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
pub struct DatabaseConfig {
    pub url: DatabaseUrlConfig,
    pub pool_size: u32,
    pub timeout_seconds: u64,
}

impl DatabaseConfig {
    pub fn validate(&self) -> Result<(), DatabaseError> {
        self.url.connection_url()?;
        if self.pool_size == 0 {
            return Err(DatabaseError::ZeroPoolSize);
        }
        if self.timeout_seconds == 0 {
            return Err(DatabaseError::ZeroTimeout);
        }
        Ok(())
    }
}

pub async fn init_db(config: &DatabaseConfig) -> Result<PgPool, DatabaseError> {
    let conn_url = config.url.connection_url()?;
    let pool_options = PgPoolOptions::new()
        .max_connections(config.pool_size)
        .acquire_timeout(Duration::from_secs(config.timeout_seconds));

    let pool = pool_options
        .connect(conn_url.as_str())
        .await
        .map_err(DatabaseError::ConnectionFailed)?;

    // Verify pgvector is enabled
    let pgvector_check: Option<(i32,)> =
        sqlx::query_as("SELECT 1 FROM pg_extension WHERE extname = 'vector'")
            .fetch_optional(&pool)
            .await
            .map_err(DatabaseError::ExtensionQueryFailed)?;

    if pgvector_check.is_none() {
        return Err(DatabaseError::PgVectorNotEnabled);
    }

    // Run migrations
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .map_err(DatabaseError::MigrationFailed)?;

    Ok(pool)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn valid_database_config() -> DatabaseConfig {
        DatabaseConfig {
            url: DatabaseUrlConfig::new("postgres://localhost:5432/test"),
            pool_size: 10,
            timeout_seconds: 30,
        }
    }

    #[test]
    fn test_database_config_validate_success() {
        let config = valid_database_config();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_database_url_config_connection_url_simple() {
        let url_config = DatabaseUrlConfig::new("postgres://localhost:5432/test");
        assert_eq!(
            url_config.connection_url().unwrap().as_str(),
            "postgres://localhost:5432/test"
        );
    }

    #[test]
    fn test_database_url_config_connection_url_with_credentials() {
        let url_config = DatabaseUrlConfig {
            url: "postgres://localhost:5432/test".to_string(),
            username: Some("myuser".to_string()),
            password: Some("mypass".to_string()),
        };
        assert_eq!(
            url_config.connection_url().unwrap().as_str(),
            "postgres://myuser:mypass@localhost:5432/test"
        );
    }

    #[test]
    fn test_database_url_config_rejects_invalid_url_and_cannot_be_a_base_credentials() {
        let bad = DatabaseUrlConfig::new("not a url");
        assert!(matches!(
            bad.connection_url(),
            Err(DatabaseError::InvalidUrl(_))
        ));

        let cannot_set_creds = DatabaseUrlConfig {
            url: "mailto:someone@example.com".to_string(),
            username: Some("user".to_string()),
            password: Some("pass".to_string()),
        };
        assert!(matches!(
            cannot_set_creds.connection_url(),
            Err(DatabaseError::SetUsernameFailed)
        ));
    }

    #[rstest]
    #[case("")]
    #[case(" ")]
    #[case("  ")]
    #[case("\t")]
    #[case("\n")]
    #[case(" \t\n ")]
    fn test_database_config_validate_empty_url(#[case] url: &str) {
        let mut config = valid_database_config();
        config.url = DatabaseUrlConfig::new(url);

        let result = config.validate();
        assert!(matches!(result, Err(DatabaseError::EmptyUrl)));
        assert_eq!(
            result.unwrap_err().to_string(),
            "database.url must not be empty"
        );
    }

    #[test]
    fn test_database_config_validate_zero_pool_size() {
        let mut config = valid_database_config();
        config.pool_size = 0;

        let result = config.validate();
        assert!(matches!(result, Err(DatabaseError::ZeroPoolSize)));
        assert_eq!(
            result.unwrap_err().to_string(),
            "database.pool_size must be greater than 0"
        );
    }

    #[test]
    fn test_database_config_validate_zero_timeout() {
        let mut config = valid_database_config();
        config.timeout_seconds = 0;

        let result = config.validate();
        assert!(matches!(result, Err(DatabaseError::ZeroTimeout)));
        assert_eq!(
            result.unwrap_err().to_string(),
            "database.timeout_seconds must be greater than 0"
        );
    }
}


