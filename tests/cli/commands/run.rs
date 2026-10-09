use axum::{Json, Router, extract::State, http::StatusCode, response::IntoResponse, routing::get};
use onekey_cli::cli::{
  client::ApiClient,
  local_config::{ClientConfig, ResolvedServer, ServerSource},
  runtime_cache::{self, RuntimeSource},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::sync::{
  Arc,
  atomic::{AtomicU8, Ordering},
};
use std::{fs, path::PathBuf};
use tempfile::TempDir;

#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::{
  io::Write,
  process::{Command, Stdio},
  thread,
  time::{Duration, Instant},
};

const TOKEN: &str = "dbt_cache_credential_marker";
const SECRET: &str = "cached-secret-marker";

fn make_server(
  directory: &TempDir,
  url: &str,
) -> ResolvedServer {
  ResolvedServer {
    url: url.into(),
    source: ServerSource::Argument,
    config_path: directory.path().join("config.toml"),
    config: ClientConfig {
      version: 1,
      server_url: None,
      default_environment: None,
    },
  }
}

fn cache_paths(server: &ResolvedServer) -> (PathBuf, PathBuf, PathBuf) {
  let root = server.config_path.parent().unwrap();
  let directory = root.join("run-cache");
  let name = format!("{:x}", Sha256::digest(server.url.as_bytes()));
  (
    root.join("run-cache-key"),
    directory.join(&name),
    directory.join(format!("{name}.lock")),
  )
}

/// Stand-in for `GET /api/v1/projects/{ref}/secrets/runtime`; `mode` selects failures.
async fn runtime_handler(State(mode): State<Arc<AtomicU8>>) -> impl IntoResponse {
  let ok = |entries: Value| {
    (
      StatusCode::OK,
      Json(json!({
        "data": {
          "project": "payment-service",
          "projectId": "prj_01CACHE",
          "groups": ["development"],
          "cacheTtlSeconds": if mode.load(Ordering::SeqCst) == 8 { 0 } else { 3600 },
          "entries": entries
        }
      })),
    )
  };
  match mode.load(Ordering::SeqCst) {
    1 => (
      StatusCode::SERVICE_UNAVAILABLE,
      Json(json!({"error":{"SERVER_UNAVAILABLE":"try later"}})),
    ),
    2 => (
      StatusCode::UNAUTHORIZED,
      Json(json!({"error":{"AUTHENTICATION_REQUIRED":"sign in"}})),
    ),
    7 => (
      StatusCode::FORBIDDEN,
      Json(json!({"error":{"AUTHORIZATION_DENIED":"revoked"}})),
    ),
    3 => (StatusCode::OK, Json(json!({"data":{"invalid":true}}))),
    4 => (
      StatusCode::NOT_FOUND,
      Json(json!({"error":{"PROJECT_NOT_FOUND":"missing"}})),
    ),
    5 => ok(json!([{"key":"","value":"invalid"}])),
    6 => {
      let oversized = "x".repeat(4 * 1024 * 1024 + 1);
      (StatusCode::OK, Json(json!({"data":{"padding":oversized}})))
    }
    _ => ok(json!([{"key":"API_TOKEN","value":SECRET}])),
  }
}

#[tokio::test]
async fn disabled_offline_fallback_removes_the_cached_copy() {
  let (mode, url, task) = start_server().await;
  let directory = TempDir::new().unwrap();
  let server = make_server(&directory, &url);
  let api = ApiClient::new(&server, Some(TOKEN.into())).unwrap();
  runtime_cache::load(&server, &api, "payment-service")
    .await
    .unwrap();
  let cache_path = cache_paths(&server).1;
  assert!(cache_path.exists());

  mode.store(8, Ordering::SeqCst);
  let live = runtime_cache::load(&server, &api, "payment-service")
    .await
    .unwrap();
  assert!(matches!(
    live.source,
    RuntimeSource::Live {
      cache_warning: None
    }
  ));
  assert_eq!(live.entries[0].value, SECRET);
  assert!(!cache_path.exists());
  task.abort();
}

async fn start_server() -> (Arc<AtomicU8>, String, tokio::task::JoinHandle<()>) {
  let mode = Arc::new(AtomicU8::new(0));
  let router = Router::new()
    .route(
      "/api/v1/projects/{project_ref}/secrets/runtime",
      get(runtime_handler),
    )
    .with_state(mode.clone());
  let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
  let address = listener.local_addr().unwrap();
  let task = tokio::spawn(async move {
    axum::serve(listener, router).await.unwrap();
  });
  (mode, format!("http://{address}"), task)
}

#[tokio::test]
async fn live_fetch_refreshes_encrypted_cache_and_falls_back_only_on_availability_errors() {
  let (mode, url, task) = start_server().await;
  let directory = TempDir::new().unwrap();
  let server = make_server(&directory, &url);
  let api = ApiClient::new(&server, Some(TOKEN.into())).unwrap();

  let live = runtime_cache::load(&server, &api, "payment-service")
    .await
    .unwrap();
  assert!(matches!(live.source, RuntimeSource::Live { .. }));

  #[cfg_attr(not(unix), allow(unused_variables))]
  let (key_path, cache_path, lock_path) = cache_paths(&server);
  let stored = fs::read(&cache_path).unwrap();
  for marker in [
    SECRET,
    "API_TOKEN",
    "payment-service",
    "development",
    &url,
    TOKEN,
  ] {
    assert!(
      !stored
        .windows(marker.len())
        .any(|window| window == marker.as_bytes())
    );
  }

  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(
      fs::metadata(directory.path()).unwrap().permissions().mode() & 0o777,
      0o700
    );
    assert_eq!(
      fs::metadata(cache_path.parent().unwrap())
        .unwrap()
        .permissions()
        .mode()
        & 0o777,
      0o700
    );
    for path in [&key_path, &cache_path, &lock_path] {
      assert_eq!(
        fs::metadata(path).unwrap().permissions().mode() & 0o777,
        0o600
      );
    }
    for path in [&key_path, &cache_path, &lock_path] {
      fs::set_permissions(path, std::fs::Permissions::from_mode(0o644)).unwrap();
    }
  }

  mode.store(1, Ordering::SeqCst);
  let cached = runtime_cache::load(&server, &api, "payment-service")
    .await
    .unwrap();
  assert!(matches!(cached.source, RuntimeSource::Cache { .. }));
  assert_eq!(cached.entries[0].value, SECRET);

  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    for path in [&key_path, &cache_path, &lock_path] {
      assert_eq!(
        fs::metadata(path).unwrap().permissions().mode() & 0o777,
        0o600
      );
    }
  }

  mode.store(2, Ordering::SeqCst);
  let unauthorized = runtime_cache::load(&server, &api, "payment-service")
    .await
    .unwrap_err()
    .to_string();
  assert!(unauthorized.contains("AUTHENTICATION_REQUIRED"));

  mode.store(7, Ordering::SeqCst);
  let forbidden = runtime_cache::load(&server, &api, "payment-service")
    .await
    .unwrap_err()
    .to_string();
  assert!(forbidden.contains("AUTHORIZATION_DENIED"));

  mode.store(4, Ordering::SeqCst);
  let not_found = runtime_cache::load(&server, &api, "payment-service")
    .await
    .unwrap_err()
    .to_string();
  assert!(not_found.contains("PROJECT_NOT_FOUND"));

  mode.store(3, Ordering::SeqCst);
  let invalid = runtime_cache::load(&server, &api, "payment-service")
    .await
    .unwrap_err()
    .to_string();
  assert!(invalid.contains("runtime response was invalid"));

  mode.store(5, Ordering::SeqCst);
  let invalid_entry = runtime_cache::load(&server, &api, "payment-service")
    .await
    .unwrap_err()
    .to_string();
  assert!(invalid_entry.contains("invalid environment variable name"));

  mode.store(6, Ordering::SeqCst);
  let oversized = runtime_cache::load(&server, &api, "payment-service")
    .await
    .unwrap_err()
    .to_string();
  assert!(oversized.contains("maximum allowed size"));

  mode.store(0, Ordering::SeqCst);
  let original_key = fs::read(&key_path).unwrap();
  fs::write(&key_path, b"invalid").unwrap();
  let live_with_warning = runtime_cache::load(&server, &api, "payment-service")
    .await
    .unwrap();
  assert!(matches!(
    live_with_warning.source,
    RuntimeSource::Live {
      cache_warning: Some(_)
    }
  ));

  fs::write(&key_path, original_key).unwrap();
  task.abort();
  let offline = runtime_cache::load(&server, &api, "payment-service")
    .await
    .unwrap();
  assert!(matches!(offline.source, RuntimeSource::Cache { .. }));

  let mut tampered = fs::read(&cache_path).unwrap();
  let last = tampered.len() - 1;
  tampered[last] ^= 1;
  fs::write(&cache_path, tampered).unwrap();
  assert!(
    runtime_cache::load(&server, &api, "payment-service")
      .await
      .is_err()
  );

  let wrong_api = ApiClient::new(&server, Some("different-token".into())).unwrap();
  assert!(
    runtime_cache::load(&server, &wrong_api, "payment-service")
      .await
      .is_err()
  );
}

#[tokio::test]
async fn unavailable_server_without_cache_does_not_provide_runtime_values() {
  let (mode, url, task) = start_server().await;
  let directory = TempDir::new().unwrap();
  let server = make_server(&directory, &url);
  let api = ApiClient::new(&server, Some(TOKEN.into())).unwrap();
  mode.store(1, Ordering::SeqCst);
  task.abort();

  let error = runtime_cache::load(&server, &api, "payment-service")
    .await
    .unwrap_err()
    .to_string();
  assert!(error.contains("Environment variables were not injected"));
  assert!(error.contains("child was not started"));
}

// Reuse this integration-test executable as a portable, trusted stdin consumer.
// This fixture uses synthetic values only and never prints the rendered input.
#[test]
fn template_child_receiver() {
  let Some(marker) = std::env::var_os("ONEKEY_TEST_TEMPLATE_CHILD") else {
    return;
  };
  use std::io::Read;
  let mut text = String::new();
  std::io::stdin().read_to_string(&mut text).unwrap();
  let doc: Value = serde_saphyr::from_str(&text).unwrap();
  assert_eq!(doc["stringData"]["password"], SECRET);
  assert_eq!(doc["stringData"]["literal"], "${API_TOKEN}");
  assert_eq!(std::env::var("API_TOKEN").unwrap(), SECRET);
  assert!(std::env::var_os("ONEKEY_TOKEN").is_none());
  fs::write(marker, "received").unwrap();
  std::process::exit(23);
}

#[tokio::test]
async fn template_passes_yaml_to_child_stdin_without_modifying_source_or_logging_values() {
  let (_mode, url, task) = start_server().await;
  let directory = TempDir::new().unwrap();
  let template = directory.path().join("manifest.yaml");
  let marker = directory.path().join("child.started");
  let source =
    "kind: Secret\nstringData:\n  password: #{{API_TOKEN}}#\n  literal: '${API_TOKEN}'\n";
  fs::write(&template, source).unwrap();
  let output = tokio::process::Command::new(env!("CARGO_BIN_EXE_onekey"))
    .args(["--server", &url, "--data-dir"])
    .arg(directory.path())
    .args(["run", "payment-service", "--template"])
    .arg(&template)
    .arg("--")
    .arg(std::env::current_exe().unwrap())
    .args([
      "--exact",
      "commands::run::template_child_receiver",
      "--nocapture",
    ])
    .env("ONEKEY_TOKEN", TOKEN)
    .env("ONEKEY_TEST_TEMPLATE_CHILD", &marker)
    .output()
    .await
    .unwrap();
  assert_eq!(
    output.status.code(),
    Some(23),
    "{}",
    String::from_utf8_lossy(&output.stderr)
  );
  assert_eq!(fs::read_to_string(marker).unwrap(), "received");
  assert_eq!(fs::read_to_string(template).unwrap(), source);
  assert!(!String::from_utf8_lossy(&output.stdout).contains(SECRET));
  assert!(!String::from_utf8_lossy(&output.stderr).contains(SECRET));
  task.abort();
}

#[tokio::test]
async fn missing_or_invalid_template_does_not_start_the_child() {
  let (_mode, url, task) = start_server().await;
  let directory = TempDir::new().unwrap();
  let template = directory.path().join("manifest.yaml");
  for source in [
    "password: #{{MISSING}}#",
    "password: [",
    "password: !custom literal-secret-marker",
  ] {
    fs::write(&template, source).unwrap();
    let output = tokio::process::Command::new(env!("CARGO_BIN_EXE_onekey"))
      .args(["--server", &url, "--data-dir"])
      .arg(directory.path())
      .args(["run", "payment-service", "--template"])
      .arg(&template)
      .args(["--", env!("CARGO_BIN_EXE_onekey"), "--version"])
      .env("ONEKEY_TOKEN", TOKEN)
      .output()
      .await
      .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty(), "child was started");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains(SECRET));
    assert!(!stderr.contains("literal-secret-marker"));
  }
  task.abort();
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn interactive_child_reads_from_the_terminal_and_returns_its_exit_status() {
  let (_mode, url, task) = start_server().await;
  let directory = TempDir::new().unwrap();
  let child_pid_path = directory.path().join("child.pid");
  let launcher_path = directory.path().join("interactive-child.sh");
  fs::write(
    &launcher_path,
    "exec \"$ONEKEY_BIN\" --server \"$ONEKEY_SERVER\" --data-dir \"$ONEKEY_DATA_DIR\" run payment-service -- sh -c 'echo \"$$\" > \"$CHILD_PID_PATH\"; IFS= read -r line; [ \"$line\" = ping ] && [ \"$API_TOKEN\" = \"$EXPECTED_SECRET\" ]; exit 23'\n",
  )
  .unwrap();
  let mut command = Command::new("script");
  #[cfg(target_os = "macos")]
  command
    .args(["-q", "-e", "/dev/null", "sh"])
    .arg(&launcher_path);
  #[cfg(not(target_os = "macos"))]
  command.args([
    "-q",
    "-e",
    "-f",
    "-c",
    "sh \"$ONEKEY_TEST_SCRIPT\"",
    "/dev/null",
  ]);
  command
    .env("ONEKEY_BIN", env!("CARGO_BIN_EXE_onekey"))
    .env("ONEKEY_SERVER", &url)
    .env("ONEKEY_DATA_DIR", directory.path())
    .env("ONEKEY_TEST_SCRIPT", &launcher_path)
    .env("ONEKEY_TOKEN", TOKEN)
    .env("CHILD_PID_PATH", &child_pid_path)
    .env("EXPECTED_SECRET", SECRET)
    .stdin(Stdio::piped())
    .stdout(Stdio::null())
    .stderr(Stdio::null());

  let mut child = command.spawn().unwrap();
  child.stdin.take().unwrap().write_all(b"ping\n").unwrap();

  let deadline = Instant::now() + Duration::from_secs(3);
  let status = loop {
    if let Some(status) = child.try_wait().unwrap() {
      break status;
    }
    if Instant::now() >= deadline {
      if let Ok(pid) = fs::read_to_string(&child_pid_path)
        && let Ok(pid) = pid.trim().parse::<i32>()
      {
        use nix::{
          sys::signal::{Signal, killpg},
          unistd::Pid,
        };

        let _ = killpg(Pid::from_raw(pid), Signal::SIGCONT);
        let _ = killpg(Pid::from_raw(pid), Signal::SIGKILL);
      }
      let _ = child.kill();
      let _ = child.wait();
      panic!("interactive child did not finish after receiving terminal input");
    }
    thread::sleep(Duration::from_millis(10));
  };

  task.abort();
  assert_eq!(status.code(), Some(23));
}

#[tokio::test]
async fn expired_or_unbounded_cache_does_not_start_run_or_curl() {
  use onekey_cli::cli::commands::cache::store::{self, CachedRuntime};
  use onekey_cli::models::SecretInput;
  use tokio::process::Command;
  let (mode, url, task) = start_server().await;
  mode.store(1, Ordering::SeqCst);
  let directory = TempDir::new().unwrap();
  let server = make_server(&directory, &url);
  for (ttl, age, message) in [
    (Some(3600), 3601, "offline cache has expired"),
    (Some(0), 0, "disabled by the server"),
    (None, 0, "no server-issued TTL"),
  ] {
    let cached = CachedRuntime {
      project: "payment-service".into(),
      environment: "development".into(),
      environment_id: "prj_01CACHE".into(),
      aliases: vec![],
      fetched_at: chrono::Utc::now() - chrono::Duration::seconds(age),
      cache_ttl_seconds: ttl,
      entries: vec![SecretInput {
        key: "API_TOKEN".into(),
        value: SECRET.into(),
      }],
    };
    store::save(&server, TOKEN, "payment-service", &cached).unwrap();
    let original = fs::read(cache_paths(&server).1).unwrap();
    for arguments in [
      vec![
        "run",
        "payment-service",
        "--",
        env!("CARGO_BIN_EXE_onekey"),
        "--version",
      ],
      vec![
        "curl",
        "-p",
        "payment-service",
        "--bearer",
        "API_TOKEN",
        "https://example.test",
      ],
    ] {
      let output = Command::new(env!("CARGO_BIN_EXE_onekey"))
        .args(["--server", &url, "--data-dir"])
        .arg(directory.path())
        .args(arguments)
        .env("ONEKEY_TOKEN", TOKEN)
        .output()
        .await
        .unwrap();
      assert!(!output.status.success());
      assert!(output.stdout.is_empty(), "child ran unexpectedly");
      let stderr = String::from_utf8_lossy(&output.stderr);
      assert!(stderr.contains(message), "{stderr}");
      assert!(!stderr.contains(SECRET));
    }
    assert_eq!(
      fs::read(cache_paths(&server).1).unwrap(),
      original,
      "offline reads must not refresh the cache"
    );
  }
  task.abort();
}
