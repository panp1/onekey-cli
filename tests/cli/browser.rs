#[cfg(unix)]
use axum::{Json, Router, extract::State, http::StatusCode, routing::get};
#[cfg(unix)]
use onekey_cli::cli::{local_config, session};
use serde_json::{Value, json};
use std::process::Stdio;
#[cfg(unix)]
use std::sync::{
  Arc,
  atomic::{AtomicUsize, Ordering},
};
use tempfile::TempDir;
use tokio::{io::AsyncWriteExt, process::Command};

const ID: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
fn frame(value: Value) -> Vec<u8> {
  let bytes = serde_json::to_vec(&value).unwrap();
  let mut result = (bytes.len() as u32).to_ne_bytes().to_vec();
  result.extend(bytes);
  result
}
async fn command(
  dir: &TempDir,
  server: &str,
  args: &[&str],
) -> std::process::Output {
  Command::new(env!("CARGO_BIN_EXE_onekey"))
    .args([
      "--data-dir",
      dir.path().to_str().unwrap(),
      "--server",
      server,
    ])
    .args(args)
    .env_remove("ONEKEY_TOKEN")
    .output()
    .await
    .unwrap()
}
#[cfg(unix)]
async fn native(
  dir: &TempDir,
  server: &str,
  caller: &str,
  value: Value,
) -> std::process::Output {
  let mut child = Command::new(env!("CARGO_BIN_EXE_onekey"))
    .args([
      "--data-dir",
      dir.path().to_str().unwrap(),
      "--server",
      server,
      "browser",
      "host",
      caller,
    ])
    .env_remove("ONEKEY_TOKEN")
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .unwrap();
  let mut stdin = child.stdin.take().unwrap();
  stdin.write_all(&frame(value)).await.unwrap();
  drop(stdin);
  child.wait_with_output().await.unwrap()
}
fn response(output: &std::process::Output) -> Value {
  assert!(
    output.status.success(),
    "{}",
    String::from_utf8_lossy(&output.stderr)
  );
  let size = u32::from_ne_bytes(output.stdout[..4].try_into().unwrap()) as usize;
  assert_eq!(output.stdout.len(), size + 4);
  serde_json::from_slice(&output.stdout[4..]).unwrap()
}

#[cfg(unix)]
#[tokio::test]
async fn native_bridge_checks_binding_and_caller_and_uses_server_ttl() {
  let count = Arc::new(AtomicUsize::new(0));
  let router = Router::new().route("/api/v1/projects/{project}/secrets/runtime", get(|State(count): State<Arc<AtomicUsize>>| async move {
    let call = count.fetch_add(1, Ordering::SeqCst);
    if call > 0 { return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"success":false}))); }
    (StatusCode::OK, Json(json!({"data":{"project":"website","projectId":"prj_browser","groups":["login"],"cacheTtlSeconds":900,"entries":[{"key":"LOGIN_USER","value":"fixture-user"},{"key":"LOGIN_PASSWORD","value":"fixture-password"},{"key":"OTHER","value":"unbound-secret"}]}})))
  })).with_state(count.clone());
  let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
  let url = format!("http://{}", listener.local_addr().unwrap());
  let task = tokio::spawn(async move {
    axum::serve(listener, router).await.unwrap();
  });
  let dir = TempDir::new().unwrap();
  let server = local_config::resolve(Some(&url), Some(dir.path())).unwrap();
  session::save(
    &server,
    "dpa_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
    None,
  )
  .unwrap();
  let added = command(
    &dir,
    &url,
    &[
      "browser",
      "add",
      "work",
      "--origin",
      "https://accounts.example.com",
      "--project",
      "website",
      "--username-key",
      "LOGIN_USER",
      "--password-key",
      "LOGIN_PASSWORD",
    ],
  )
  .await;
  assert!(
    added.status.success(),
    "{}",
    String::from_utf8_lossy(&added.stderr)
  );
  let stage = TempDir::new().unwrap();
  let installed = command(
    &dir,
    &url,
    &[
      "browser",
      "install",
      "--extension-id",
      ID,
      "--manifest-dir",
      stage.path().to_str().unwrap(),
    ],
  )
  .await;
  assert!(
    installed.status.success(),
    "{}",
    String::from_utf8_lossy(&installed.stderr)
  );
  let caller = format!("chrome-extension://{ID}/");
  let listed = response(&native(&dir, &url, &caller, json!({"action":"list"})).await);
  assert_eq!(listed["bindings"][0]["name"], "work");
  assert!(!listed.to_string().contains("fixture-password"));
  let request = |origin: &str| json!({"action":"fill","name":"work","origin":origin});
  let refused = response(&native(&dir, &url, &caller, request("https://evil.example.com")).await);
  assert_eq!(refused["ok"], false);
  assert_eq!(count.load(Ordering::SeqCst), 0);
  let bad = native(
    &dir,
    &url,
    "chrome-extension://bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb/",
    json!({"action":"list"}),
  )
  .await;
  assert!(!bad.status.success());
  assert!(bad.stdout.is_empty());
  let live = native(&dir, &url, &caller, request("https://accounts.example.com")).await;
  let result = response(&live);
  assert_eq!(result["credentials"]["password"], "fixture-password");
  assert_eq!(result["source"], "live");
  assert!(!result.to_string().contains("unbound-secret"));
  assert!(live.stderr.is_empty());
  let cached =
    response(&native(&dir, &url, &caller, request("https://accounts.example.com")).await);
  assert_eq!(cached["source"], "cache");
  let edge_stage = TempDir::new().unwrap();
  let edge_id = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
  let installed = command(
    &dir,
    &url,
    &[
      "browser",
      "install",
      "--browser",
      "edge",
      "--extension-id",
      edge_id,
      "--manifest-dir",
      edge_stage.path().to_str().unwrap(),
    ],
  )
  .await;
  assert!(installed.status.success());
  let chrome_manifest: Value =
    serde_json::from_slice(&std::fs::read(stage.path().join("com.onekey.browser.json")).unwrap())
      .unwrap();
  let edge_manifest: Value = serde_json::from_slice(
    &std::fs::read(edge_stage.path().join("com.onekey.browser.json")).unwrap(),
  )
  .unwrap();
  assert_ne!(chrome_manifest["path"], edge_manifest["path"]);
  assert_eq!(
    chrome_manifest["allowed_origins"][0],
    format!("chrome-extension://{ID}/")
  );
  assert_eq!(
    edge_manifest["allowed_origins"][0],
    format!("chrome-extension://{edge_id}/")
  );
  for caller in [&caller, &format!("chrome-extension://{edge_id}/")] {
    assert_eq!(
      response(&native(&dir, &url, caller, json!({"action":"list"})).await)["ok"],
      true
    );
  }

  let mut expired = onekey_cli::cli::commands::cache::store::load(
    &server,
    "dpa_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
    "website",
  )
  .unwrap();
  expired.fetched_at = chrono::Utc::now() - chrono::Duration::seconds(900);
  onekey_cli::cli::commands::cache::store::save(
    &server,
    "dpa_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
    "website",
    &expired,
  )
  .unwrap();
  let expired =
    response(&native(&dir, &url, &caller, request("https://accounts.example.com")).await);
  assert_eq!(expired["ok"], false);
  assert!(expired.get("credentials").is_none());
  let changed_server = response(
    &native(
      &dir,
      "http://127.0.0.1:1",
      &caller,
      request("https://accounts.example.com"),
    )
    .await,
  );
  assert_eq!(changed_server["ok"], false);
  task.abort();
}

#[cfg(unix)]
#[tokio::test]
async fn ui_catalog_and_ai_queue_preserve_metadata_and_authorization_boundaries() {
  use onekey_cli::cli::commands::browser;
  let router = Router::new()
    .route("/api/v1/projects",get(||async{Json(json!({"data":[{"id":"prj_web","name":"website"}]}))}))
    .route("/api/v1/environments",get(||async{Json(json!({"data":[{"id":"env_web","projectId":"prj_web","projectName":"website","name":"login"}]}))}))
    .route("/api/v1/environments/env_web/secrets",get(||async{Json(json!({"data":[{"key":"LOGIN_USER","value":"MUST_NOT_LEAK"},{"key":"LOGIN_PASSWORD","value":"MUST_NOT_LEAK"}]}))}))
    .route("/api/v1/projects/{project}/secrets/runtime",get(||async{Json(json!({"data":{"project":"website","projectId":"prj_web","groups":["login"],"cacheTtlSeconds":900,"entries":[{"key":"LOGIN_USER","value":"fixture-user"},{"key":"LOGIN_PASSWORD","value":"fixture-password"}]}}))}));
  let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
  let url = format!("http://{}", listener.local_addr().unwrap());
  let task = tokio::spawn(async move {
    axum::serve(listener, router).await.unwrap();
  });
  let dir = TempDir::new().unwrap();
  let stage = TempDir::new().unwrap();
  let server = local_config::resolve(Some(&url), Some(dir.path())).unwrap();
  session::save(
    &server,
    "dpa_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
    None,
  )
  .unwrap();
  assert!(
    command(
      &dir,
      &url,
      &[
        "browser",
        "install",
        "--extension-id",
        ID,
        "--manifest-dir",
        stage.path().to_str().unwrap()
      ]
    )
    .await
    .status
    .success()
  );
  let caller = format!("chrome-extension://{ID}/");
  let projects = response(&native(&dir, &url, &caller, json!({"action":"catalog"})).await);
  assert_eq!(projects["projects"][0]["id"], "prj_web");
  let keys = response(
    &native(
      &dir,
      &url,
      &caller,
      json!({"action":"catalog","project":"prj_web"}),
    )
    .await,
  );
  assert_eq!(keys["keys"].as_array().unwrap().len(), 2);
  assert!(!keys.to_string().contains("MUST_NOT_LEAK"));
  let mut binding = json!({"name":"work","serverUrl":"https://ignored.example.com","origin":"https://accounts.example.com","project":"prj_web","usernameKey":"LOGIN_USER","passwordKey":"LOGIN_PASSWORD","usernameSelector":null,"passwordSelector":null,"loginOrigins":["https://sso.example.com"],"submitOrigins":["https://auth.example.com"],"allowJs":true,"allowAi":false,"enabled":true,"browser":"chrome"});
  assert_eq!(
    response(
      &native(
        &dir,
        &url,
        &caller,
        json!({"action":"save","binding":binding})
      )
      .await
    )["ok"],
    true
  );
  assert!(browser::enqueue_fill(&server, "work", browser::BrowserKind::Chrome).is_err());
  binding["allowAi"] = true.into();
  assert_eq!(
    response(
      &native(
        &dir,
        &url,
        &caller,
        json!({"action":"save","binding":binding})
      )
      .await
    )["ok"],
    true
  );
  assert_eq!(
    browser::connections(&server).unwrap()["connections"][0]["name"],
    "work"
  );
  let queued = browser::enqueue_fill(&server, "work", browser::BrowserKind::Chrome).unwrap();
  let id = queued["requestId"].as_str().unwrap();
  let wrong_browser = response(
    &native(
      &dir,
      &url,
      &caller,
      json!({"action":"poll","browser":"edge","version":"0.2.5"}),
    )
    .await,
  );
  assert!(wrong_browser["requests"].as_array().unwrap().is_empty());
  let polled = response(
    &native(
      &dir,
      &url,
      &caller,
      json!({"action":"poll","browser":"chrome","version":"0.2.5"}),
    )
    .await,
  );
  assert_eq!(polled["requests"][0]["id"], id);
  assert_eq!(
    browser::fill_result(&server, id).unwrap()["status"],
    "processing"
  );
  let username = response(&native(&dir,&url,&caller,json!({"action":"fill","name":"work","origin":"https://sso.example.com","fields":"username","requestId":id})).await);
  assert_eq!(username["credentials"]["username"], "fixture-user");
  assert_eq!(username["credentials"]["password"], "");
  assert!(!username.to_string().contains("fixture-password"));
  assert_eq!(
    response(
      &native(
        &dir,
        &url,
        &caller,
        json!({"action":"complete","id":id,"outcome":"filled"})
      )
      .await
    )["ok"],
    true
  );
  assert_eq!(
    browser::fill_result(&server, id).unwrap()["outcome"],
    "filled"
  );
  // An extension that reports an old version (or none) is refused before queueing.
  response(
    &native(
      &dir,
      &url,
      &caller,
      json!({"action":"poll","browser":"edge","version":"0.2.4"}),
    )
    .await,
  );
  let outdated = browser::enqueue_fill(&server, "work", browser::BrowserKind::Edge).unwrap_err();
  assert!(outdated.to_string().contains("older than"));
  let status = browser::browser_status(&server).unwrap();
  assert_eq!(status["browsers"]["edge"]["outdated"], true);
  assert_eq!(status["browsers"]["chrome"]["extensionVersion"], "0.2.5");
  response(
    &native(
      &dir,
      &url,
      &caller,
      json!({"action":"poll","browser":"edge","version":"0.2.5"}),
    )
    .await,
  );
  // One shared legacy Chrome binding can target either browser at call time.
  let edge_request = browser::enqueue_fill(&server, "work", browser::BrowserKind::Edge).unwrap();
  let chrome_request =
    browser::enqueue_fill(&server, "work", browser::BrowserKind::Chrome).unwrap();
  assert_eq!(edge_request["browser"], "edge");
  assert_eq!(chrome_request["browser"], "chrome");
  assert_eq!(
    browser::connections(&server).unwrap()["connections"]
      .as_array()
      .unwrap()
      .len(),
    1
  );
  let edge_id = edge_request["requestId"].as_str().unwrap();
  let chrome_id = chrome_request["requestId"].as_str().unwrap();
  let edge_poll = response(
    &native(
      &dir,
      &url,
      &caller,
      json!({"action":"poll","browser":"edge","version":"0.2.5"}),
    )
    .await,
  );
  assert_eq!(edge_poll["requests"].as_array().unwrap().len(), 1);
  assert_eq!(edge_poll["requests"][0]["id"], edge_id);
  assert_eq!(
    browser::fill_result(&server, chrome_id).unwrap()["status"],
    "pending"
  );
  let edge_again = response(
    &native(
      &dir,
      &url,
      &caller,
      json!({"action":"poll","browser":"edge","version":"0.2.5"}),
    )
    .await,
  );
  assert!(edge_again["requests"].as_array().unwrap().is_empty());
  let chrome_poll = response(
    &native(
      &dir,
      &url,
      &caller,
      json!({"action":"poll","browser":"chrome","version":"0.2.5"}),
    )
    .await,
  );
  assert_eq!(chrome_poll["requests"][0]["id"], chrome_id);
  browser::cancel_fill(&server, edge_id).unwrap();
  browser::cancel_fill(&server, chrome_id).unwrap();
  assert_eq!(
    browser::browser_status(&server).unwrap()["extensionRecentlyConnected"],
    true
  );
  // A finished result outlives its request, so a late lookup still finds it.
  let first = id.to_string();
  let result_path = dir
    .path()
    .join("browser-requests")
    .join(format!("{first}.result"));
  let age_result = |seconds: i64| {
    let mut record: Value = serde_json::from_slice(&std::fs::read(&result_path).unwrap()).unwrap();
    record["expiresAt"] = (chrono::Utc::now().timestamp() - seconds).into();
    std::fs::write(&result_path, serde_json::to_vec(&record).unwrap()).unwrap();
  };
  age_result(60);
  let queued = browser::enqueue_fill(&server, "work", browser::BrowserKind::Chrome).unwrap();
  assert_eq!(
    browser::fill_result(&server, &first).unwrap()["status"],
    "complete"
  );
  age_result(16 * 60);
  let id = queued["requestId"].as_str().unwrap();
  response(
    &native(
      &dir,
      &url,
      &caller,
      json!({"action":"poll","browser":"chrome","version":"0.2.5"}),
    )
    .await,
  );
  // A refusal records only a fixed reason code.
  let refused = browser::enqueue_fill(&server, "work", browser::BrowserKind::Chrome).unwrap();
  let refused_id = refused["requestId"].as_str().unwrap();
  response(
    &native(
      &dir,
      &url,
      &caller,
      json!({"action":"poll","browser":"chrome","version":"0.2.5"}),
    )
    .await,
  );
  let completed = response(
    &native(
      &dir,
      &url,
      &caller,
      json!({"action":"complete","id":refused_id,"outcome":"refused","reason":"multipleTabs"}),
    )
    .await,
  );
  assert_eq!(completed["ok"], true);
  let result = browser::fill_result(&server, refused_id).unwrap();
  assert_eq!(result["outcome"], "refused");
  assert_eq!(result["reason"], "multipleTabs");
  // Free text is not a reason: the host rejects the frame and records nothing.
  let free_text = native(
    &dir,
    &url,
    &caller,
    json!({"action":"complete","id":id,"outcome":"refused","reason":"page said: hello"}),
  )
  .await;
  assert!(!free_text.status.success() || response(&free_text)["ok"] == false);
  assert_ne!(
    browser::fill_result(&server, id).unwrap()["status"],
    "complete"
  );
  browser::cancel_fill(&server, id).unwrap();
  assert_eq!(
    browser::fill_result(&server, id).unwrap()["outcome"],
    "canceled"
  );
  let canceled = response(
    &native(
      &dir,
      &url,
      &caller,
      json!({"action":"fill","name":"work","origin":"https://accounts.example.com","requestId":id}),
    )
    .await,
  );
  assert_eq!(canceled["ok"], false);
  let queued = browser::enqueue_fill(&server, "work", browser::BrowserKind::Chrome).unwrap();
  // Past the retention window, the next enqueue prunes the old result.
  assert_eq!(
    browser::fill_result(&server, &first).unwrap()["status"],
    "unknown"
  );
  let id = queued["requestId"].as_str().unwrap();
  response(
    &native(
      &dir,
      &url,
      &caller,
      json!({"action":"poll","browser":"chrome","version":"0.2.5"}),
    )
    .await,
  );
  // A token rotation cannot make a queued AI request use a different identity.
  session::save(
    &server,
    "dpa_BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
    None,
  )
  .unwrap();
  let mismatch = response(
    &native(
      &dir,
      &url,
      &caller,
      json!({"action":"fill","name":"work","origin":"https://accounts.example.com","requestId":id}),
    )
    .await,
  );
  assert_eq!(mismatch["ok"], false);
  assert!(mismatch.get("credentials").is_none());
  session::save(
    &server,
    "dpa_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
    None,
  )
  .unwrap();
  let checked=response(&native(&dir,&url,&caller,json!({"action":"check","name":"work","origin":"https://accounts.example.com","requestId":id})).await);
  assert_eq!(checked["ok"], true);
  let queued_path = dir
    .path()
    .join("browser-requests")
    .join(format!("{id}.claimed"));
  let original = std::fs::read(&queued_path).unwrap();
  let mut expired: Value = serde_json::from_slice(&original).unwrap();
  expired["expiresAt"] = (chrono::Utc::now().timestamp() - 1).into();
  std::fs::write(&queued_path, serde_json::to_vec(&expired).unwrap()).unwrap();
  let expired=response(&native(&dir,&url,&caller,json!({"action":"check","name":"work","origin":"https://accounts.example.com","requestId":id})).await);
  assert_eq!(expired["ok"], false);
  std::fs::write(&queued_path, original).unwrap();
  binding["enabled"] = false.into();
  response(
    &native(
      &dir,
      &url,
      &caller,
      json!({"action":"save","binding":binding}),
    )
    .await,
  );
  assert!(
    browser::connections(&server).unwrap()["connections"]
      .as_array()
      .unwrap()
      .is_empty()
  );
  let revoked = response(
    &native(
      &dir,
      &url,
      &caller,
      json!({"action":"fill","name":"work","origin":"https://accounts.example.com","requestId":id}),
    )
    .await,
  );
  assert_eq!(revoked["ok"], false);
  assert!(revoked.get("credentials").is_none());
  task.abort();
}

#[cfg(unix)]
#[tokio::test]
async fn subscribed_port_receives_requests_without_waiting_for_the_alarm() {
  use onekey_cli::cli::commands::browser;
  use tokio::io::AsyncReadExt;
  let dir = TempDir::new().unwrap();
  let stage = TempDir::new().unwrap();
  let url = "https://one.example.com";
  let server = local_config::resolve(Some(url), Some(dir.path())).unwrap();
  session::save(
    &server,
    "dpa_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
    None,
  )
  .unwrap();
  let installed = command(
    &dir,
    url,
    &[
      "browser",
      "install",
      "--extension-id",
      ID,
      "--manifest-dir",
      stage.path().to_str().unwrap(),
    ],
  )
  .await;
  assert!(installed.status.success());
  let caller = format!("chrome-extension://{ID}/");
  let binding = json!({"name":"work","serverUrl":"","origin":"https://accounts.example.com","project":"prj_web","usernameKey":"LOGIN_USER","passwordKey":"LOGIN_PASSWORD","usernameSelector":null,"passwordSelector":null,"loginOrigins":[],"submitOrigins":[],"allowJs":false,"allowAi":true,"enabled":true,"browser":null});
  assert_eq!(
    response(
      &native(
        &dir,
        url,
        &caller,
        json!({"action":"save","binding":binding})
      )
      .await
    )["ok"],
    true
  );

  let mut child = Command::new(env!("CARGO_BIN_EXE_onekey"))
    .args([
      "--data-dir",
      dir.path().to_str().unwrap(),
      "--server",
      url,
      "browser",
      "host",
      &caller,
    ])
    .env_remove("ONEKEY_TOKEN")
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::null())
    .kill_on_drop(true)
    .spawn()
    .unwrap();
  let mut stdin = child.stdin.take().unwrap();
  stdin
    .write_all(&frame(
      json!({"action":"subscribe","browser":"edge","version":"0.2.5"}),
    ))
    .await
    .unwrap();
  let mut stdout = child.stdout.take().unwrap();
  let mut read_frame = async || -> Value {
    let mut header = [0; 4];
    stdout.read_exact(&mut header).await.unwrap();
    let mut body = vec![0; u32::from_ne_bytes(header) as usize];
    stdout.read_exact(&mut body).await.unwrap();
    serde_json::from_slice(&body).unwrap()
  };
  // The first keepalive frame confirms the subscription and records the heartbeat.
  assert_eq!(read_frame().await["ok"], true);
  assert_eq!(
    browser::browser_status(&server).unwrap()["browsers"]["edge"]["recentlyConnected"],
    true
  );
  let queued = browser::enqueue_fill(&server, "work", browser::BrowserKind::Edge).unwrap();
  let pushed = tokio::time::timeout(std::time::Duration::from_secs(5), async {
    loop {
      let frame = read_frame().await;
      if let Some(request) = frame["requests"]
        .as_array()
        .and_then(|r| r.first())
        .cloned()
      {
        return request;
      }
    }
  })
  .await
  .expect("request pushed within 5 seconds");
  assert_eq!(pushed["id"], queued["requestId"]);
  assert_eq!(
    browser::fill_result(&server, queued["requestId"].as_str().unwrap()).unwrap()["status"],
    "processing"
  );
  drop(stdin);
  let _ = child.kill().await;
}

#[cfg(unix)]
#[tokio::test]
async fn saving_a_binding_keeps_same_named_bindings_of_other_servers() {
  let dir = TempDir::new().unwrap();
  let stage = TempDir::new().unwrap();
  let caller = format!("chrome-extension://{ID}/");
  let servers = ["https://one.example.com", "https://two.example.com"];
  for url in servers {
    let installed = command(
      &dir,
      url,
      &[
        "browser",
        "install",
        "--extension-id",
        ID,
        "--manifest-dir",
        stage.path().to_str().unwrap(),
      ],
    )
    .await;
    assert!(
      installed.status.success(),
      "{}",
      String::from_utf8_lossy(&installed.stderr)
    );
    let binding = json!({"name":"work","serverUrl":"","origin":"https://accounts.example.com","project":"prj_web","usernameKey":"LOGIN_USER","passwordKey":"LOGIN_PASSWORD","usernameSelector":null,"passwordSelector":null,"loginOrigins":[],"submitOrigins":[],"allowJs":false,"allowAi":false,"enabled":true,"browser":"chrome"});
    let saved = response(
      &native(
        &dir,
        url,
        &caller,
        json!({"action":"save","binding":binding}),
      )
      .await,
    );
    assert_eq!(saved["ok"], true);
  }
  for url in servers {
    let listed = response(&native(&dir, url, &caller, json!({"action":"list"})).await);
    let bindings = listed["bindings"].as_array().unwrap();
    assert_eq!(bindings.len(), 1, "{url}");
    assert_eq!(bindings[0]["serverUrl"], url);
  }
}

#[cfg(windows)]
#[tokio::test]
async fn windows_installer_stages_executable_host_and_pinned_arguments() {
  let dir = TempDir::new().unwrap();
  let stage = TempDir::new().unwrap();
  let url = "https://onekey.example.com";
  let installed = command(
    &dir,
    url,
    &[
      "browser",
      "install",
      "--browser",
      "edge",
      "--extension-id",
      ID,
      "--manifest-dir",
      stage.path().to_str().unwrap(),
    ],
  )
  .await;
  assert!(
    installed.status.success(),
    "{}",
    String::from_utf8_lossy(&installed.stderr)
  );
  let manifest: Value = serde_json::from_slice(
    &std::fs::read(stage.path().join("com.onekey.browser-edge.json")).unwrap(),
  )
  .unwrap();
  let binary = manifest["path"].as_str().unwrap();
  assert!(binary.ends_with("onekey-native-edge.exe"));
  let caller = format!("chrome-extension://{ID}/");
  let mut child = Command::new(binary)
    .args([&caller, "--parent-window=0"])
    .env("ONEKEY_URL", "https://wrong.example.com")
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .unwrap();
  let mut stdin = child.stdin.take().unwrap();
  stdin
    .write_all(&frame(json!({"action":"list"})))
    .await
    .unwrap();
  drop(stdin);
  assert_eq!(
    response(&child.wait_with_output().await.unwrap())["ok"],
    true
  );
  assert_eq!(manifest["allowed_origins"][0], caller);
}
