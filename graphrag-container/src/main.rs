mod cli;
mod config;
mod db;
pub mod graph_rag;
pub mod hams_tools;
pub mod logging;
mod llm_tools;
pub mod mcp;
pub mod ontology;
mod routes;
mod state;

use crate::graph_rag::GraphStore;
use crate::hams_tools::HamsHarness;
use clap::Parser;
use cli::Cli;
use config::AppConfig;
use hams::hams::Hams;
use state::AppState;
use std::process;
use std::sync::Arc;
use thiserror::Error;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

#[derive(Debug, Error)]
enum BootError {
    #[error("Failed to load configuration: {0}")]
    ConfigLoad(#[from] Box<figment::Error>),
    #[error("Configuration validation failed: {0}")]
    ConfigValidation(#[from] config::ConfigError),
    #[error("Logging initialization failed: {0}")]
    Logging(#[from] logging::LoggingError),
    #[error("Database initialization failed: {0}")]
    Database(#[from] db::DatabaseError),
    #[error("Failed to initialize LlmClient: {0}")]
    Llm(#[from] llm_tools::LlmError),
    #[error("Failed to bind to address {address}: {source}")]
    Bind {
        address: String,
        #[source]
        source: std::io::Error,
    },
    #[error("Failed to initialize HaMS: {0}")]
    Hams(#[from] hams_tools::HamsError),
    #[error("Server error: {0}")]
    Serve(#[source] std::io::Error),
}

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        error!("{err}");
        eprintln!("{err}");
        process::exit(1);
    }
}

async fn run() -> Result<(), BootError> {
    let cli = Cli::parse();
    let ct = CancellationToken::new();

    let config =
        AppConfig::load(cli.config.as_deref(), cli.secrets_dir.as_deref()).map_err(Box::new)?;
    config.validate()?;

    let filter_directive = config.logging.filter_directive();
    let filter = config.logging.build_env_filter()?;

    tracing_subscriber::fmt().with_env_filter(filter).init();

    info!(
        "Configuration loaded successfully. Active log filter: {}",
        filter_directive
    );

    let pool = db::init_db(&config.database).await?;
    info!("Database connection pool initialized and verified successfully.");

    let llm_client = crate::llm_tools::LlmClient::new(&config.llm)?;
    info!("LLM client initialized successfully.");

    let address = format!("{}:{}", config.server_host, config.server_port);
    let hams_config = config.hams.clone();

    let app_state = AppState {
        pool,
        config: Arc::new(config),
        llm: Arc::new(llm_client),
        graph_store: GraphStore::default(),
        mcp_server: Arc::new(crate::mcp::server::GraphRagMcpServer::new()),
    };

    let app = routes::create_router_with_config(&app_state.config).with_state(app_state);

    let listener = tokio::net::TcpListener::bind(&address)
        .await
        .map_err(|source| BootError::Bind {
            address: address.clone(),
            source,
        })?;

    let hams = Hams::new(hams_config);
    let mut hams_harness = HamsHarness::init(hams, ct.clone()).await?;
    hams_harness.ready_signal.enable();
    info!("HaMS sidecar initialized and marked ready");

    info!("Application started successfully. Listening on {}", address);

    let server = axum::serve(listener, app).with_graceful_shutdown(async move {
        ct.cancelled().await;
        info!("Graceful shutdown signal received, stopping axum server");
    });

    let serve_result = server.await.map_err(BootError::Serve);

    if let Err(e) = hams_harness.hams.stop() {
        error!("Failed to cleanly stop HaMS: {}", e);
    }

    serve_result
}
