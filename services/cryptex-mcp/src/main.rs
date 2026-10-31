use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use cryptex_projection::{ProjectionDb, SearchQuery, search_events};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Serialize, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize, Deserialize)]
struct JsonRpcError {
    code: i64,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

fn resolve_runtime_root() -> PathBuf {
    if let Ok(val) = std::env::var("CRYPTEX_RUNTIME_ROOT") {
        return PathBuf::from(val);
    }
    cryptexd::paths::default_p0_2_runtime_root()
        .unwrap_or_else(|_| std::env::temp_dir().join("Cryptex-Dev").join("p0-2"))
}

fn handle_tools_list() -> Value {
    json!({
        "tools": [
            {
                "name": "cryptex_search",
                "description": "Full-text search (FTS5) over personal memory, notes, and events in Cryptex.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "text": {
                            "type": "string",
                            "description": "Search keyword or FTS5 query string"
                        },
                        "classification": {
                            "type": "string",
                            "description": "Filter by classification: public, internal, private",
                            "enum": ["public", "internal", "private"]
                        },
                        "limit": {
                            "type": "number",
                            "description": "Max results to return (default: 10, max: 50)"
                        }
                    },
                    "required": ["text"]
                }
            },
            {
                "name": "cryptex_status",
                "description": "Check Cryptex engine and runtime health status.",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            }
        ]
    })
}

fn handle_tool_call(name: &str, arguments: &Value) -> Result<Value, String> {
    match name {
        "cryptex_search" => {
            let text = arguments
                .get("text")
                .and_then(|t| t.as_str())
                .ok_or_else(|| "Missing required parameter 'text'".to_string())?;
            let classification = arguments
                .get("classification")
                .and_then(|c| c.as_str())
                .map(|s| s.to_string());
            let limit = arguments
                .get("limit")
                .and_then(|l| l.as_u64())
                .map(|n| n.min(50) as u32);

            let root = resolve_runtime_root();
            let (db, _open_kind) = ProjectionDb::open_replacing_unusable(&root).map_err(|e| {
                format!(
                    "Failed to open Cryptex projection DB at {}: {:?}",
                    root.display(),
                    e
                )
            })?;

            let query = SearchQuery {
                text: Some(text.to_string()),
                classification,
                event_type: None,
                device_id: None,
                brain_id: None,
                created_at_from: None,
                created_at_to: None,
                limit,
            };

            let hits = search_events(db.connection(), &query)
                .map_err(|e| format!("Search failed: {:?}", e))?;

            let mut out = String::new();
            if hits.is_empty() {
                out.push_str("No memories found matching the query.\n");
            } else {
                out.push_str(&format!("Found {} matching memories:\n\n", hits.len()));
                for (idx, hit) in hits.iter().enumerate() {
                    out.push_str(&format!(
                        "### {}. Event `{}` ({})\n- **Created:** {}\n- **Classification:** {}\n\n```\n{}\n```\n\n",
                        idx + 1,
                        hit.event_id,
                        hit.event_type,
                        hit.created_at,
                        hit.classification,
                        hit.text
                    ));
                }
            }

            Ok(json!({
                "content": [
                    {
                        "type": "text",
                        "text": out
                    }
                ]
            }))
        }
        "cryptex_status" => {
            let root = resolve_runtime_root();
            let exists = root.exists();
            let db_path = root
                .join("derived")
                .join("projection")
                .join("projection.db");
            let db_exists = db_path.exists();

            let status_text = format!(
                "**Cryptex Engine Status**\n\
                 - **Runtime Root:** `{}` (Exists: {})\n\
                 - **Projection SQLite DB:** `{}` (Exists: {})\n\
                 - **Mode:** Local-First Embedded / Supervised IPC\n\
                 - **Status:** Healthy\n",
                root.display(),
                exists,
                db_path.display(),
                db_exists
            );

            Ok(json!({
                "content": [
                    {
                        "type": "text",
                        "text": status_text
                    }
                ]
            }))
        }
        _ => Err(format!("Unknown tool: {}", name)),
    }
}

fn handle_request(request: &JsonRpcRequest) -> Option<JsonRpcResponse> {
    let req_id = request.id.clone().unwrap_or(Value::Null);

    match request.method.as_str() {
        "initialize" => Some(JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id: req_id,
            result: Some(json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {}
                },
                "serverInfo": {
                    "name": "cryptex-mcp",
                    "version": "0.1.0"
                }
            })),
            error: None,
        }),
        "notifications/initialized" => {
            // MCP client notification, no response required
            None
        }
        "ping" => Some(JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id: req_id,
            result: Some(json!({})),
            error: None,
        }),
        "tools/list" => Some(JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id: req_id,
            result: Some(handle_tools_list()),
            error: None,
        }),
        "tools/call" => {
            let params = request.params.clone().unwrap_or(Value::Null);
            let tool_name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let arguments = params.get("arguments").unwrap_or(&Value::Null);

            match handle_tool_call(tool_name, arguments) {
                Ok(result) => Some(JsonRpcResponse {
                    jsonrpc: "2.0".into(),
                    id: req_id,
                    result: Some(result),
                    error: None,
                }),
                Err(err_msg) => Some(JsonRpcResponse {
                    jsonrpc: "2.0".into(),
                    id: req_id,
                    result: Some(json!({
                        "isError": true,
                        "content": [
                            {
                                "type": "text",
                                "text": format!("Error: {}", err_msg)
                            }
                        ]
                    })),
                    error: None,
                }),
            }
        }
        _ => Some(JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id: req_id,
            result: None,
            error: Some(JsonRpcError {
                code: -32601,
                message: format!("Method not found: {}", request.method),
                data: None,
            }),
        }),
    }
}

fn handle_line(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    let request: JsonRpcRequest = match serde_json::from_str(trimmed) {
        Ok(req) => req,
        Err(e) => {
            let err_resp = json!({
                "jsonrpc": "2.0",
                "id": Value::Null,
                "error": {
                    "code": -32700,
                    "message": format!("Parse error: {}", e)
                }
            });
            return Some(err_resp.to_string());
        }
    };

    let resp = handle_request(&request)?;
    serde_json::to_string(&resp).ok()
}

fn main() -> io::Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        if let Some(out) = handle_line(&line) {
            writeln!(stdout, "{}", out)?;
            stdout.flush()?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Feed one synthetic JSON-RPC line through the same parse + dispatch
    /// path `main` uses and return the parsed response.
    fn dispatch(line: &str) -> Value {
        let out = handle_line(line).expect("request line should produce a response");
        serde_json::from_str(&out).expect("response should be valid JSON")
    }

    #[test]
    fn unknown_method_returns_method_not_found() {
        let resp = dispatch(r#"{"jsonrpc":"2.0","id":1,"method":"no/suchMethod"}"#);
        assert_eq!(resp["id"], json!(1));
        assert!(resp.get("result").is_none());
        assert_eq!(resp["error"]["code"], json!(-32601));
        assert!(
            resp["error"]["message"]
                .as_str()
                .expect("error message should be a string")
                .contains("no/suchMethod")
        );
    }

    #[test]
    fn unknown_tool_returns_structured_is_error() {
        let resp = dispatch(
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"cryptex_nope","arguments":{}}}"#,
        );
        assert_eq!(resp["id"], json!(2));
        assert!(resp.get("error").is_none());
        let result = &resp["result"];
        assert_eq!(result["isError"], json!(true));
        let text = result["content"][0]["text"]
            .as_str()
            .expect("error content should carry text");
        assert!(text.contains("Unknown tool: cryptex_nope"), "got: {text}");
    }

    #[test]
    fn search_without_text_argument_is_a_clean_error() {
        // The argument check runs before any database is opened, so these
        // calls fail cleanly without touching a real runtime root.
        for params in [
            r#"{"name":"cryptex_search","arguments":{}}"#,
            r#"{"name":"cryptex_search","arguments":{"limit":5}}"#,
            r#"{"name":"cryptex_search"}"#,
        ] {
            let line =
                format!(r#"{{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{params}}}"#);
            let resp = dispatch(&line);
            assert_eq!(resp["id"], json!(3));
            let result = &resp["result"];
            assert_eq!(result["isError"], json!(true), "params: {params}");
            let text = result["content"][0]["text"]
                .as_str()
                .expect("error content should carry text");
            assert!(
                text.contains("Missing required parameter 'text'"),
                "params: {params}, got: {text}"
            );
        }
    }

    #[test]
    fn initialize_returns_server_info() {
        let resp = dispatch(
            r#"{"jsonrpc":"2.0","id":4,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"0"}}}"#,
        );
        assert_eq!(resp["id"], json!(4));
        assert!(resp.get("error").is_none());
        assert_eq!(resp["result"]["serverInfo"]["name"], json!("cryptex-mcp"));
        assert_eq!(resp["result"]["protocolVersion"], json!("2024-11-05"));
    }

    #[test]
    fn tools_list_advertises_search_and_status() {
        let resp = dispatch(r#"{"jsonrpc":"2.0","id":5,"method":"tools/list"}"#);
        assert_eq!(resp["id"], json!(5));
        let tools = resp["result"]["tools"]
            .as_array()
            .expect("tools should be an array");
        let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
        assert!(names.contains(&"cryptex_search"), "got: {names:?}");
        assert!(names.contains(&"cryptex_status"), "got: {names:?}");
        let search = tools
            .iter()
            .find(|t| t["name"] == json!("cryptex_search"))
            .expect("cryptex_search should be listed");
        assert_eq!(search["inputSchema"]["required"], json!(["text"]));
    }

    #[test]
    fn malformed_json_returns_parse_error() {
        let resp = dispatch(r#"{"jsonrpc":"2.0","id":6,"method":"#);
        assert_eq!(resp["id"], Value::Null);
        assert_eq!(resp["error"]["code"], json!(-32700));
    }

    #[test]
    fn notifications_and_blank_lines_get_no_response() {
        assert!(handle_line(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#).is_none());
        assert!(handle_line("   ").is_none());
    }
}
