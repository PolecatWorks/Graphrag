use crate::state::AppState;
use axum::{Router, routing::get};

pub fn create_router() -> Router<AppState> {
    Router::new().route("/hello", get(hello))
}

async fn hello() -> &'static str {
    "Hello, World!"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;
    use crate::db::DatabaseConfig;
    use crate::llm_tools::LlmConfig;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt; // for `collect`
    use std::sync::Arc;
    use tower::ServiceExt; // for `oneshot`

    #[tokio::test]
    async fn test_hello() {
        // Create mock AppState
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://mock:mock@localhost/mock")
            .unwrap();
        let config = Arc::new(AppConfig {
            database: DatabaseConfig {
                url: "postgres://mock".to_string(),
                pool_size: 1,
                timeout_seconds: 1,
            },
            server_host: "127.0.0.1".to_string(),
            server_port: 8080,
            hams: hams::hams::config::HamsConfig::default(),
            llm: LlmConfig {
                provider: "ollama".to_string(),
                base_url: "http://localhost:11434/v1".to_string(),
                api_key: None,
                model: "llama3".to_string(),
                timeout_seconds: 60,
            },
        });

        let state = AppState {
            pool,
            config: config.clone(),
            llm: std::sync::Arc::new(crate::llm_tools::LlmClient::new(&config.llm).unwrap()),
        };

        let app = create_router().with_state(state);

        // `Router` implements `tower::Service<Request<Body>>` so we can
        // call it like any tower service, no need to run an HTTP server.
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/hello")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(&body[..], b"Hello, World!");
    }
}
