use serde::Deserialize;
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum DatabaseError {
    #[error("database.url must not be empty")]
    EmptyUrl,
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

#[derive(Debug, Deserialize, PartialEq, Clone)]
pub struct DatabaseConfig {
    pub url: String,
    pub pool_size: u32,
    pub timeout_seconds: u64,
}

impl DatabaseConfig {
    pub fn validate(&self) -> Result<(), DatabaseError> {
        if self.url.trim().is_empty() {
            return Err(DatabaseError::EmptyUrl);
        }
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
    let pool_options = PgPoolOptions::new()
        .max_connections(config.pool_size)
        .acquire_timeout(Duration::from_secs(config.timeout_seconds));

    let pool = pool_options
        .connect(&config.url)
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
            url: "postgres://localhost:5432/test".to_string(),
            pool_size: 10,
            timeout_seconds: 30,
        }
    }

    #[test]
    fn test_database_config_validate_success() {
        let config = valid_database_config();
        assert!(config.validate().is_ok());
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
        config.url = url.to_string();

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


