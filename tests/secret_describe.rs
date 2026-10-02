//! `onekey secret describe` sends a PATCH with only the description, never a value.

use axum::{
  Json, Router,
  extract::State,
  routing::{get, patch},
};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
use tempfile::TempDir;
use tokio::{net::TcpListener, process::Command};

const TOKEN: &str = "dpa_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";

type Bodies = Arc<Mutex<Vec<Value>>>;

async fn describe(
  State(bodies): State<Bodies>,
  Json(body): Json<Value>,
) -> Json<Value> {
  bodies.lock().unwrap().push(body.clone());
  Json(
    json!({"data":{"key":"API_KEY","description":body["description"],"version":3,"createdAt":"","updatedAt":""}}),
  )
}

async fn run(
  address: std::net::SocketAddr,
  args: &[&str],
) -> std::process::Output {
  let directory = TempDir::new().unwrap();
  Command::new(env!("CARGO_BIN_EXE_onekey"))
    .args(["--data-dir", directory.path().to_str().unwrap()])
    .args(["--server", &format!("http://{address}")])
    .args(["secret", "describe", "alpha/dev", "API_KEY"])
    .args(args)
    .env("ONEKEY_TOKEN", TOKEN)
    .output()
    .await
    .unwrap()
}

#[tokio::test]
async fn describe_sets_and_clears_only_the_description() {
  let bodies: Bodies = Arc::default();
  let router = Router::new()
    .route(
      "/api/v1/environments/resolve",
      get(|| async { Json(json!({"data":{"id":"env_alpha","name":"dev"}})) }),
    )
    .route(
      "/api/v1/environments/env_alpha/secrets/API_KEY",
      patch(describe),
    )
    .with_state(bodies.clone());
  let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
  let address = listener.local_addr().unwrap();
  let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });

  let set = run(address, &["Billing API bearer token."]).await;
  assert!(set.status.success(), "{set:?}");
  assert!(String::from_utf8_lossy(&set.stdout).contains("Updated the description of API_KEY"));
  let cleared = run(address, &["--clear"]).await;
  assert!(cleared.status.success(), "{cleared:?}");
  assert!(String::from_utf8_lossy(&cleared.stdout).contains("Cleared the description of API_KEY"));
  let missing = run(address, &[]).await;
  assert!(!missing.status.success());

  assert_eq!(
    *bodies.lock().unwrap(),
    [
      json!({"description":"Billing API bearer token."}),
      json!({"description":null})
    ]
  );
  server.abort();
}
