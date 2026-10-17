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
