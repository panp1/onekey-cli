use axum::{
  Json, Router,
  http::{StatusCode, Uri},
  routing::get,
};
use serde_json::{Value, json};
use tempfile::TempDir;
use tokio::{
  io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
  net::TcpListener,
  process::{ChildStdin, ChildStdout, Command},
  time::{Duration, timeout},
};

const TOKEN: &str = "dpa_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";

async fn exchange(
  stdin: &mut ChildStdin,
  stdout: &mut BufReader<ChildStdout>,
  request: Value,
) -> Value {
  stdin
    .write_all(request.to_string().as_bytes())
    .await
    .unwrap();
  stdin.write_all(b"\n").await.unwrap();
  let mut line = String::new();
  timeout(Duration::from_secs(10), stdout.read_line(&mut line))
    .await
    .expect("MCP server response timed out")
    .unwrap();
  serde_json::from_str(&line).expect("MCP response must be JSON")
}

#[tokio::test]
async fn stdio_server_lists_only_scoped_metadata() {
  let router = Router::new()
    .route(
      "/api/v1/projects",
      get(|| async { Json(json!({"data":[{"id":"prj_alpha","name":"alpha"}]})) }),
    )
    .route(
      "/api/v1/environments",
      get(|uri:Uri| async move {
        if uri.query() == Some("project=error") {
          return (StatusCode::SERVICE_UNAVAILABLE,Json(json!({"error":"MUST_NOT_LEAK"})));
        }
        (StatusCode::OK,Json(json!({"data":[{"id":"env_alpha","projectId":"prj_alpha","projectName":"alpha","name":"dev"}]})))
      }),
    )
    .route(
      "/api/v1/environments/env_alpha/secrets",
      get(|| async {
        Json(json!({"data":[{"key":"API_KEY","description":"Use as a bearer token for the billing API.","version":2,"updatedAt":"2026-10-01T00:00:00Z","value":"MUST_NOT_LEAK"}]}))
      }),
    );
  let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
  let address = listener.local_addr().unwrap();
  let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
  let directory = TempDir::new().unwrap();
  let mut child = Command::new(env!("CARGO_BIN_EXE_onekey"))
    .args([
      "--data-dir",
      directory.path().to_str().unwrap(),
      "--server",
      &format!("http://{address}"),
      "mcp",
      "serve",
    ])
    .env("ONEKEY_TOKEN", TOKEN)
    .stdin(std::process::Stdio::piped())
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .kill_on_drop(true)
    .spawn()
    .unwrap();
  let mut stdin = child.stdin.take().unwrap();
  let mut stdout = BufReader::new(child.stdout.take().unwrap());

  let initialized = exchange(
    &mut stdin,
    &mut stdout,
    json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"test","version":"1"}}}),
  )
  .await;
  assert_eq!(initialized["id"], 1);
  assert_eq!(initialized["result"]["serverInfo"]["name"], "onekey");
  stdin
    .write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n")
    .await
    .unwrap();

  let listed = exchange(
    &mut stdin,
    &mut stdout,
    json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
  )
  .await;
  let tools = listed["result"]["tools"].as_array().unwrap();
  assert_eq!(tools.len(), 9);
  assert!(
    tools
      .iter()
      .filter(|tool| ![
        "onekey_fill_browser_connection",
        "onekey_request_browser_fill",
        "onekey_cancel_browser_fill"
      ]
      .iter()
      .any(|name| tool["name"] == *name))
      .all(|tool| tool["annotations"]["readOnlyHint"] == true)
  );
  assert!(!listed.to_string().contains("reveal"));

  let projects = exchange(
    &mut stdin,
    &mut stdout,
    json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"onekey_list_projects","arguments":{}}}),
  )
  .await;
  assert_eq!(
    projects["result"]["structuredContent"]["items"][0]["name"],
    "alpha"
  );

  let groups = exchange(
    &mut stdin,
    &mut stdout,
    json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"onekey_list_groups","arguments":{"project":"alpha"}}}),
  )
  .await;
  assert_eq!(
    groups["result"]["structuredContent"]["items"][0]["name"],
    "dev"
  );

  let secrets = exchange(
    &mut stdin,
    &mut stdout,
    json!({"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"onekey_list_secret_names","arguments":{"project":"alpha","group":"dev"}}}),
  )
  .await;
  assert_eq!(
    secrets["result"]["structuredContent"]["items"][0]["key"],
    "API_KEY"
  );
  assert_eq!(
    secrets["result"]["structuredContent"]["items"][0]["description"],
    "Use as a bearer token for the billing API."
  );
  assert!(!secrets.to_string().contains("MUST_NOT_LEAK"));

  let status = exchange(&mut stdin,&mut stdout,json!({"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"onekey_get_browser_status","arguments":{}}})).await;
  assert_eq!(
    status["result"]["structuredContent"]["extensionRecentlyConnected"],
    false
  );
  std::fs::write(directory.path().join("browser-logins.json"),serde_json::to_vec(&json!({"bindings":[{"name":"web","serverUrl":format!("http://{address}"),"origin":"https://accounts.example.com","project":"alpha","usernameKey":"USER","passwordKey":"PASSWORD","usernameSelector":null,"passwordSelector":null,"allowAi":true,"enabled":true}]})).unwrap()).unwrap();
  let connections=exchange(&mut stdin,&mut stdout,json!({"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"onekey_list_browser_connections","arguments":{}}})).await;
  assert_eq!(
    connections["result"]["structuredContent"]["connections"][0]["name"],
    "web"
  );
  let request=exchange(&mut stdin,&mut stdout,json!({"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"onekey_request_browser_fill","arguments":{"name":"web"}}})).await;
  let id = request["result"]["structuredContent"]["requestId"]
    .as_str()
    .expect("async fill returns a requestId");
  let pending=exchange(&mut stdin,&mut stdout,json!({"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"name":"onekey_get_browser_fill_result","arguments":{"request_id":id}}})).await;
  assert_eq!(pending["result"]["structuredContent"]["status"], "pending");
  let canceled=exchange(&mut stdin,&mut stdout,json!({"jsonrpc":"2.0","id":10,"method":"tools/call","params":{"name":"onekey_cancel_browser_fill","arguments":{"request_id":id}}})).await;
  assert_eq!(canceled["result"]["structuredContent"]["canceled"], true);
  let result=exchange(&mut stdin,&mut stdout,json!({"jsonrpc":"2.0","id":11,"method":"tools/call","params":{"name":"onekey_get_browser_fill_result","arguments":{"request_id":id}}})).await;
  assert_eq!(result["result"]["structuredContent"]["outcome"], "canceled");
  for output in [status, connections, request, pending, canceled, result] {
    assert!(!output.to_string().contains("MUST_NOT_LEAK"));
    assert!(!output.to_string().contains(TOKEN));
  }

  let failure=exchange(&mut stdin,&mut stdout,json!({"jsonrpc":"2.0","id":12,"method":"tools/call","params":{"name":"onekey_list_groups","arguments":{"project":"error"}}})).await;
  assert_eq!(failure["result"]["isError"], true);
  assert!(!failure.to_string().contains("MUST_NOT_LEAK"));
  drop(stdin);
  let status = timeout(Duration::from_secs(10), child.wait())
    .await
    .unwrap()
    .unwrap();
  assert!(status.success());
  server.abort();
}

#[tokio::test]
async fn stdio_server_refuses_human_login_token() {
  let directory = TempDir::new().unwrap();
  let output = Command::new(env!("CARGO_BIN_EXE_onekey"))
    .args([
      "--data-dir",
      directory.path().to_str().unwrap(),
      "mcp",
      "serve",
    ])
    .env("ONEKEY_TOKEN", "human-session-token")
    .output()
    .await
    .unwrap();
  assert!(!output.status.success());
  assert!(
    String::from_utf8_lossy(&output.stderr).contains("human login sessions are not supported")
  );
  assert!(output.stdout.is_empty());
}
