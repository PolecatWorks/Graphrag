mod cli;
mod config;
mod db;
pub mod graph_rag;
pub mod hams_tools;
mod llm_tools;
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
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    let ct = CancellationToken::new();

    let config = match AppConfig::load(cli.config.as_deref()) {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to load configuration: {}", e);
            process::exit(1);
        }
    };

    if let Err(e) = config.validate() {
        error!("Configuration validation failed: {}", e);
        process::exit(1);
    }

    info!("Configuration loaded successfully.");

    let hams_config = config.hams.clone();
    let hams = Hams::new(hams_config);
    let mut hams_harness = match HamsHarness::init(hams, ct.clone()).await {
        Ok(h) => {
            info!("HaMS sidecar initialized successfully");
            h
        }
        Err(e) => {
            error!("Failed to initialize HaMS: {}", e);
            process::exit(1);
        }
    };

    let pool = match db::init_db(&config.database).await {
        Ok(p) => {
            info!("Database connection pool initialized and verified successfully.");
            p
        }
        Err(e) => {
            error!("Database initialization failed: {}", e);
            process::exit(1);
        }
    };

    let host = config.server_host.clone();
    let port = config.server_port;

    let llm_client = match crate::llm_tools::LlmClient::new(&config.llm) {
        Ok(client) => {
            info!("LLM client initialized successfully.");
            client
        }
        Err(e) => {
            error!("Failed to initialize LlmClient: {}", e);
            process::exit(1);
        }
    };

    hams_harness.ready_signal.enable();

    let app_state = AppState {
        pool,
        config: Arc::new(config),
        llm: Arc::new(llm_client),
        graph_store: GraphStore::default(),
    };

    let app = routes::create_router().with_state(app_state);

    let address = format!("{}:{}", host, port);
    let listener = match tokio::net::TcpListener::bind(&address).await {
        Ok(l) => l,
        Err(e) => {
            error!("Failed to bind to address {}: {}", address, e);
            process::exit(1);
        }
    };

    info!("Application started successfully. Listening on {}", address);

    let server = axum::serve(listener, app).with_graceful_shutdown(async move {
        ct.cancelled().await;
        info!("Graceful shutdown signal received, stopping axum server");
    });

    if let Err(e) = server.await {
        error!("Server error: {}", e);
        process::exit(1);
    }

    if let Err(e) = hams_harness.hams.stop() {
        error!("Failed to cleanly stop HaMS: {}", e);
    }
}
