use serde_json::{json, Value};
use crate::state::AppState;
use tracing::{debug, error, instrument, trace};

#[instrument(skip(state, raw_request), fields(method = tracing::field::Empty, req_id = tracing::field::Empty))]
pub async fn handle_json_rpc(state: &AppState, raw_request: Value) -> Option<Value> {
    let id = raw_request.get("id").cloned();
    let current_span = tracing::Span::current();
    if let Some(ref id_val) = id {
        current_span.record("req_id", tracing::field::display(id_val));
    }

    let method = match raw_request.get("method").and_then(|m| m.as_str()) {
        Some(m) => {
            current_span.record("method", m);
            m
        }
        None => {
            debug!("MCP JSON-RPC request missing method");
            return id.map(|id_val| {
                json!({
                    "jsonrpc": "2.0",
                    "id": id_val,
                    "error": {
                        "code": -32600,
                        "message": "Invalid Request: missing method"
                    }
                })
            });
        }
    };

    trace!("Processing MCP JSON-RPC method '{}'", method);
    let params = raw_request.get("params").cloned().unwrap_or(Value::Null);

    let result = match method {
        "initialize" => {
            debug!("MCP client initialize received");
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {}
                },
                "serverInfo": {
                    "name": state.config.mcp.server_name,
                    "version": env!("CARGO_PKG_VERSION")
                },
                "instructions": state.config.mcp.instructions
            })
        }
        "notifications/initialized" => {
            debug!("MCP client initialized notification received");
            if id.is_none() {
                return None;
            }
            json!({})
        }
        "ping" => {
            trace!("MCP ping received");
            json!({})
        }
        "tools/list" => {
            let tools = state.mcp_server.list_tools();
            debug!("MCP tools/list returning {} tools", tools.len());
            json!({
                "tools": tools
            })
        }
        "tools/call" => {
            let tool_name = match params.get("name").and_then(|n| n.as_str()) {
                Some(n) => n,
                None => {
                    debug!("MCP tools/call missing 'name' parameter");
                    return id.map(|id_val| {
                        json!({
                            "jsonrpc": "2.0",
                            "id": id_val,
                            "error": {
                                "code": -32602,
                                "message": "Invalid params: 'name' is required"
                            }
                        })
                    });
                }
            };

            let arguments = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
            debug!(tool = tool_name, args = %arguments, "MCP tools/call invoked");

            let start = std::time::Instant::now();
            let res = state.mcp_server.call_tool(state, tool_name, arguments).await;
            let elapsed = start.elapsed();

            match res {
                Ok(val) => {
                    let is_error = val.get("isError").and_then(|e| e.as_bool()).unwrap_or(false);
                    if is_error {
                        debug!(tool = tool_name, duration_ms = elapsed.as_millis(), "MCP tool returned error payload: {:?}", val);
                    } else {
                        debug!(tool = tool_name, duration_ms = elapsed.as_millis(), "MCP tool execution succeeded");
                    }
                    val
                }
                Err(err) => {
                    error!(tool = tool_name, duration_ms = elapsed.as_millis(), error = %err, "MCP tool execution failed");
                    json!({
                        "content": [
                            {
                                "type": "text",
                                "text": err.to_string()
                            }
                        ],
                        "isError": true
                    })
                }
            }
        }
        _ => {
            debug!("MCP JSON-RPC method '{}' not found", method);
            return id.map(|id_val| {
                json!({
                    "jsonrpc": "2.0",
                    "id": id_val,
                    "error": {
                        "code": -32601,
                        "message": format!("Method '{}' not found", method)
                    }
                })
            });
        }
    };

    Some(match id {
        Some(id_val) => json!({
            "jsonrpc": "2.0",
            "id": id_val,
            "result": result
        }),
        None => result,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;
    use crate::db::DatabaseConfig;
    use crate::graph_rag::GraphStore;
    use crate::llm_tools::LlmConfig;
    use crate::mcp::McpConfig;
    use sqlx::postgres::PgPoolOptions;
    use std::sync::Arc;

    fn mock_app_state() -> AppState {
        let pool = PgPoolOptions::new()
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
                base_url: "http://127.0.0.1:1/v1".to_string(),
                api_key: None,
                model: "llama3".to_string(),
                timeout_seconds: 1,
            },
            mcp: McpConfig::default(),
            logging: crate::logging::LoggingConfig::default(),
        });

        AppState {
            pool,
            config: config.clone(),
            llm: Arc::new(crate::llm_tools::LlmClient::new(&config.llm).unwrap()),
            graph_store: GraphStore::default(),
            mcp_server: Arc::new(crate::mcp::server::GraphRagMcpServer::new()),
        }
    }

    #[tokio::test]
    async fn test_rpc_initialize() {
        let state = mock_app_state();
        let req = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {}
        });

        let resp = handle_json_rpc(&state, req).await.unwrap();
        assert_eq!(resp["id"], 1);
        assert_eq!(resp["result"]["protocolVersion"], "2024-11-05");
        assert_eq!(resp["result"]["serverInfo"]["name"], "graphrag-mcp");
    }

    #[tokio::test]
    async fn test_rpc_notifications_initialized() {
        let state = mock_app_state();
        let notification = json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        });
        let resp = handle_json_rpc(&state, notification).await;
        assert!(resp.is_none());

        let req_with_id = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "notifications/initialized"
        });
        let resp2 = handle_json_rpc(&state, req_with_id).await.unwrap();
        assert_eq!(resp2["id"], 2);
    }

    #[tokio::test]
    async fn test_rpc_ping() {
        let state = mock_app_state();
        let req = json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "ping"
        });
        let resp = handle_json_rpc(&state, req).await.unwrap();
        assert_eq!(resp["id"], 3);
        assert_eq!(resp["result"], json!({}));
    }

    #[tokio::test]
    async fn test_rpc_tools_list() {
        let state = mock_app_state();
        let req = json!({
            "jsonrpc": "2.0",
            "id": 4,
            "method": "tools/list"
        });
        let resp = handle_json_rpc(&state, req).await.unwrap();
        assert_eq!(resp["id"], 4);
        let tools = resp["result"]["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 23);

        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"hello"));
        assert!(names.contains(&"graphrag_ingest"));
        assert!(names.contains(&"graphrag_query"));
        assert!(names.contains(&"document_browse"));
        assert!(names.contains(&"node_browse"));
        assert!(names.contains(&"edge_browse"));
        assert!(names.contains(&"community_browse"));
    }

    #[tokio::test]
    async fn test_rpc_tool_call_hello() {
        let state = mock_app_state();
        let req = json!({
            "jsonrpc": "2.0",
            "id": 5,
            "method": "tools/call",
            "params": {
                "name": "hello",
                "arguments": {
                    "name": "  Antigravity  "
                }
            }
        });
        let resp = handle_json_rpc(&state, req).await.unwrap();
        assert_eq!(resp["id"], 5);
        assert_eq!(resp["result"]["isError"], false);
        assert_eq!(
            resp["result"]["content"][0]["text"],
            "Hello, Antigravity!"
        );

        let req_empty = json!({
            "jsonrpc": "2.0",
            "id": 6,
            "method": "tools/call",
            "params": {
                "name": "hello",
                "arguments": {
                    "name": "   "
                }
            }
        });
        let resp_empty = handle_json_rpc(&state, req_empty).await.unwrap();
        assert_eq!(resp_empty["id"], 6);
        assert_eq!(resp_empty["result"]["isError"], true);
    }

    #[tokio::test]
    async fn test_rpc_errors() {
        let state = mock_app_state();

        // Missing method -> -32600
        let req_no_method = json!({
            "jsonrpc": "2.0",
            "id": 10
        });
        let resp = handle_json_rpc(&state, req_no_method).await.unwrap();
        assert_eq!(resp["error"]["code"], -32600);

        // Unknown method -> -32601
        let req_unknown = json!({
            "jsonrpc": "2.0",
            "id": 11,
            "method": "unknown_rpc_method"
        });
        let resp2 = handle_json_rpc(&state, req_unknown).await.unwrap();
        assert_eq!(resp2["error"]["code"], -32601);

        // Missing params.name -> -32602
        let req_no_tool_name = json!({
            "jsonrpc": "2.0",
            "id": 12,
            "method": "tools/call",
            "params": {}
        });
        let resp3 = handle_json_rpc(&state, req_no_tool_name).await.unwrap();
        assert_eq!(resp3["error"]["code"], -32602);
    }
}
