//! Chrome/Edge native messaging: credentials go to the extension, never to ordinary CLI output.
use crate::cli::{client, commands::run::cache, local_config::ResolvedServer};
use crate::constants::api::{environments, secrets};
use anyhow::{Context, Result, bail};
use clap::{Subcommand, ValueEnum};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
  collections::BTreeMap,
  fs,
  io::{Read, Write},
  path::{Path, PathBuf},
};
use url::Url;
use zeroize::Zeroize;

const HOST_NAME: &str = "com.onekey.browser";
const MAX_FRAME: usize = 64 * 1024;
pub const HELP: &str = "Examples:\n  onekey browser add work --origin https://accounts.example.com --project website-logins --username-key LOGIN_USER --password-key LOGIN_PASSWORD\n  onekey browser list\n  onekey browser remove work\n  onekey browser install --browser chrome --extension-id <CHROME_EXTENSION_ID>\n  onekey browser install --browser edge --extension-id <EDGE_EXTENSION_ID>\n\nLoad browser-extension/dist in Chrome or Edge first. Host is an internal, framed stdio transport invoked by Chrome's registered launcher.\n";

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum BrowserKind {
  Chrome,
  Edge,
}
impl BrowserKind {
  fn name(self) -> &'static str {
    match self {
      Self::Chrome => "chrome",
      Self::Edge => "edge",
    }
  }
  #[cfg(unix)]
  fn directory(
    self,
    home: &Path,
  ) -> PathBuf {
    if cfg!(target_os = "macos") {
      return home.join(match self {
        Self::Chrome => "Library/Application Support/Google/Chrome/NativeMessagingHosts",
        Self::Edge => "Library/Application Support/Microsoft Edge/NativeMessagingHosts",
      });
    }
    // Chrome and Edge keep their Linux profiles under $XDG_CONFIG_HOME when it is set.
    std::env::var_os("XDG_CONFIG_HOME")
      .map(PathBuf::from)
      .filter(|path| path.is_absolute())
      .unwrap_or_else(|| home.join(".config"))
      .join(match self {
        Self::Chrome => "google-chrome/NativeMessagingHosts",
        Self::Edge => "microsoft-edge/NativeMessagingHosts",
      })
  }
}

#[derive(Debug, Subcommand)]
pub enum BrowserCommand {
  /// Bind a website to existing username and password secrets (no secret values in this file).
  #[command(after_help = HELP)]
  Add {
    /// Local connection name.
    name: String,
    /// Exact HTTPS origin; HTTP is accepted only for localhost / loopback testing.
    #[arg(long)]
    origin: String,
    /// Project name or ID containing the two bound secrets.
    #[arg(long)]
    project: String,
    /// Existing username secret name (never a username value).
    #[arg(long)]
    username_key: String,
    /// Existing password secret name (never a password value).
    #[arg(long)]
    password_key: String,
    /// Optional CSS selector when the default username detection is ambiguous.
    #[arg(long)]
    username_selector: Option<String>,
    /// Optional CSS selector when multiple password inputs are present.
    #[arg(long)]
    password_selector: Option<String>,
  },
  /// List local bindings without fetching or printing credentials.
  #[command(after_help = HELP)]
  List,
  /// Remove a local website binding.
  #[command(after_help = HELP)]
  Remove {
    /// Local connection name.
    name: String,
  },
  /// Register the current binary as a Chrome or Edge native messaging host (macOS/Linux/Windows).
  #[command(after_help = HELP)]
  Install {
    /// Browser to register; each keeps its own extension ID and launcher.
    #[arg(long, value_enum, default_value = "chrome")]
    browser: BrowserKind,
    /// Extension ID shown by chrome://extensions after loading the unpacked extension.
    #[arg(long)]
    extension_id: String,
    /// Stage the manifest in a different directory instead of installing it into Chrome.
    #[arg(long)]
    manifest_dir: Option<PathBuf>,
  },
  /// Native messaging transport. Invoked by Chrome, not an interactive secret-reveal command.
  #[command(hide = true, after_help = HELP)]
  Host {
    /// Calling Chrome extension origin supplied by the native messaging launcher.
    caller: String,
  },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Binding {
  pub name: String,
  pub server_url: String,
  pub origin: String,
  pub project: String,
  pub username_key: String,
  pub password_key: String,
  pub username_selector: Option<String>,
  pub password_selector: Option<String>,
  #[serde(default)]
  pub login_origins: Vec<String>,
  #[serde(default)]
  pub submit_origins: Vec<String>,
  #[serde(default)]
  pub allow_js: bool,
  #[serde(default)]
  pub allow_ai: bool,
  #[serde(default = "enabled")]
  pub enabled: bool,
  #[serde(default)]
  pub browser: Option<String>,
}
fn enabled() -> bool {
  true
}
impl Binding {
  fn permits(
    &self,
    origin: &str,
  ) -> bool {
    self.enabled && (self.origin == origin || self.login_origins.iter().any(|item| item == origin))
  }
  fn fingerprint(&self) -> Result<String> {
    Ok(hex::encode(Sha256::digest(serde_json::to_vec(self)?)))
  }
}

#[derive(Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Settings {
  #[serde(default)]
  bindings: Vec<Binding>,
  #[serde(default)]
  extension_ids: BTreeMap<String, String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  extension_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(
  tag = "action",
  rename_all = "camelCase",
  rename_all_fields = "camelCase",
  deny_unknown_fields
)]
enum Request {
  List,
  Catalog {
    project: Option<String>,
  },
  Save {
    binding: Binding,
  },
  Remove {
    name: String,
  },
  Poll {
    browser: Option<String>,
  },
  Check {
    name: String,
    origin: String,
    request_id: String,
  },
  Complete {
    id: String,
    outcome: Outcome,
    #[serde(default)]
    reason: Option<RefusalReason>,
  },
  Fill {
    name: String,
    origin: String,
    #[serde(default)]
    fields: Fields,
    #[serde(default)]
    request_id: Option<String>,
  },
}

pub async fn execute(
  command: BrowserCommand,
  server: &ResolvedServer,
) -> Result<i32> {
  match command {
    BrowserCommand::Add {
      name,
      origin,
      project,
      username_key,
      password_key,
      username_selector,
      password_selector,
    } => {
      if name.trim().is_empty()
        || project.trim().is_empty()
        || username_key.trim().is_empty()
        || password_key.trim().is_empty()
      {
        bail!("binding name, project, and secret names must not be empty");
      }
      let binding = Binding {
        name,
        server_url: server.url.clone(),
        origin: normalize_origin(&origin)?,
        project,
        username_key,
        password_key,
        username_selector,
        password_selector,
        login_origins: Vec::new(),
        submit_origins: Vec::new(),
        allow_js: false,
        allow_ai: false,
        enabled: true,
        browser: None,
      };
      let _lock = settings_lock(server)?;
      let mut settings = read_settings(server)?;
      if settings
        .bindings
        .iter()
        .any(|item| item.name == binding.name)
      {
        bail!("a binding with this name already exists; remove it before changing its destination");
      }
      settings.bindings.push(binding);
      write_settings(server, &settings)?;
      println!("Website binding saved. No credential values were fetched.");
    }
    BrowserCommand::List => {
      println!(
        "{}",
        serde_json::to_string_pretty(&read_settings(server)?.bindings)?
      );
    }
    BrowserCommand::Remove { name } => {
      let _lock = settings_lock(server)?;
      let mut settings = read_settings(server)?;
      let before = settings.bindings.len();
      settings.bindings.retain(|binding| binding.name != name);
      if settings.bindings.len() == before {
        bail!("website binding not found");
      }
      write_settings(server, &settings)?;
      println!("Website binding removed.");
    }
    BrowserCommand::Install {
      browser,
      extension_id,
      manifest_dir,
    } => install(server, browser, &extension_id, manifest_dir.as_deref())?,
    BrowserCommand::Host { caller } => {
      let settings = read_settings(server)?;
      let id = caller
        .strip_prefix("chrome-extension://")
        .and_then(|value| value.strip_suffix('/'))
        .context("invalid native messaging caller")?;
      validate_extension_id(id)?;
      let registered = settings.extension_ids.values().any(|value| value == id)
        || (settings.extension_ids.is_empty() && settings.extension_id.as_deref() == Some(id));
      if !registered {
        bail!("native messaging caller is not the registered extension");
      }
      serve(
        server,
        &mut std::io::stdin().lock(),
        &mut std::io::stdout().lock(),
      )
      .await?;
    }
  }
  Ok(0)
}

pub fn normalize_origin(value: &str) -> Result<String> {
  let url = Url::parse(value).context("website origin must be an absolute URL")?;
  let local = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
  if !(url.scheme() == "https" || (url.scheme() == "http" && local))
    || url.host_str().is_none()
    || url.host_str().is_some_and(|host| host.contains('*'))
    || !url.username().is_empty()
    || url.password().is_some()
    || url.path() != "/"
    || url.query().is_some()
    || url.fragment().is_some()
  {
    bail!(
      "use an exact HTTPS origin with no credentials, path, query, or fragment (HTTP only for localhost / loopback)"
    );
  }
  Ok(url.origin().ascii_serialization())
}

fn settings_path(server: &ResolvedServer) -> Result<PathBuf> {
  Ok(
    server
      .config_path
      .parent()
      .context("client configuration has no directory")?
      .join("browser-logins.json"),
  )
}
fn read_settings(server: &ResolvedServer) -> Result<Settings> {
  let path = settings_path(server)?;
  if !path.exists() {
    return Ok(Settings::default());
  }
  serde_json::from_slice(&fs::read(path)?).context("invalid local browser login configuration")
}
fn write_settings(
  server: &ResolvedServer,
  settings: &Settings,
) -> Result<()> {
  let path = settings_path(server)?;
  fs::create_dir_all(path.parent().unwrap())?;
  private_write(&path, &serde_json::to_vec_pretty(settings)?)?;
  Ok(())
}
fn validate_extension_id(id: &str) -> Result<()> {
  if id.len() != 32 || !id.bytes().all(|byte| (b'a'..=b'p').contains(&byte)) {
    bail!("extension ID must be 32 letters from a through p");
  }
  Ok(())
}
#[cfg(unix)]
fn install(
  server: &ResolvedServer,
  browser: BrowserKind,
  id: &str,
  directory: Option<&Path>,
) -> Result<()> {
  use std::os::unix::fs::PermissionsExt;
  validate_extension_id(id)?;
  let home = std::env::var_os("HOME").context("HOME is not set")?;
  let default = browser.directory(Path::new(&home));
  let directory = directory.unwrap_or(&default);
  fs::create_dir_all(directory)?;
  let state_dir = server
    .config_path
    .parent()
    .context("missing state directory")?;
  fs::create_dir_all(state_dir)?;
  let state_dir = fs::canonicalize(state_dir)?;
  let wrapper = state_dir.join(format!("browser-native-host-{}.sh", browser.name()));
  let quote = |s: &str| format!("'{}'", s.replace('\'', "'\\''"));
  let binary = std::env::current_exe()?;
  let script = format!(
    "#!/bin/sh\nunset ONEKEY_TOKEN ONEKEY_URL ONEKEY_DATA_DIR\nexec {} --data-dir {} --server {} browser host \"$@\"\n",
    quote(&binary.to_string_lossy()),
    quote(&state_dir.to_string_lossy()),
    quote(&server.url)
  );
  // private_write renames a fresh file into place, so a planted symlink is replaced, not followed.
  private_write(&wrapper, script.as_bytes())?;
  fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o700))?;
  let manifest = serde_json::json!({"name":HOST_NAME,"description":"OneKey website login bridge","path":wrapper,"type":"stdio","allowed_origins":[format!("chrome-extension://{id}/")]});
  let path = directory.join(format!("{HOST_NAME}.json"));
  fs::write(&path, serde_json::to_vec_pretty(&manifest)?)?;
  let _lock = settings_lock(server)?;
  let mut settings = read_settings(server)?;
  if let Some(legacy) = settings.extension_id.take() {
    settings
      .extension_ids
      .entry("chrome".into())
      .or_insert(legacy);
  }
  settings
    .extension_ids
    .insert(browser.name().into(), id.into());
  write_settings(server, &settings)?;
  println!("Native messaging manifest: {}", path.display());
  println!(
    "Pinned OneKey server: {}. Re-run install after moving the binary or changing profile/server.",
    server.url
  );
  Ok(())
}
#[cfg(windows)]
fn install(
  server: &ResolvedServer,
  browser: BrowserKind,
  id: &str,
  directory: Option<&Path>,
) -> Result<()> {
  install_windows(server, browser, id, directory)
}
#[cfg(any(windows, test))]
fn install_windows(
  server: &ResolvedServer,
  browser: BrowserKind,
  id: &str,
  directory: Option<&Path>,
) -> Result<()> {
  validate_extension_id(id)?;
  let state = settings_path(server)?.parent().unwrap().to_path_buf();
  fs::create_dir_all(&state)?;
  let state = fs::canonicalize(state)?;
  let binary = state.join(format!("onekey-native-{}.exe", browser.name()));
  // A running host exe cannot be overwritten on Windows, but it can be renamed away.
  let staged = binary.with_extension("exe.new");
  fs::copy(std::env::current_exe()?, &staged)?;
  if binary.exists() {
    let old = binary.with_extension("exe.old");
    let _ = fs::remove_file(&old);
    fs::rename(&binary, &old)?;
  }
  fs::rename(&staged, &binary)?;
  private_write(
    &binary.with_extension("json"),
    &serde_json::to_vec(&serde_json::json!({"server":server.url,"state":state}))?,
  )?;
  let register = directory.is_none();
  let directory = directory.unwrap_or(&state);
  fs::create_dir_all(directory)?;
  let path = directory.join(format!("{HOST_NAME}-{}.json", browser.name()));
  let manifest = serde_json::json!({"name":HOST_NAME,"description":"OneKey browser bridge","path":binary,"type":"stdio","allowed_origins":[format!("chrome-extension://{id}/")]});
  private_write(&path, &serde_json::to_vec_pretty(&manifest)?)?;
  if register {
    let vendor = match browser {
      BrowserKind::Chrome => "Google\\Chrome",
      BrowserKind::Edge => "Microsoft\\Edge",
    };
    let key = format!("HKCU\\Software\\{vendor}\\NativeMessagingHosts\\{HOST_NAME}");
    if !std::process::Command::new("reg.exe")
      .args(["add", &key, "/ve", "/t", "REG_SZ", "/d"])
      .arg(&path)
      .arg("/f")
      .status()?
      .success()
    {
      bail!("failed to register native host in HKCU");
    }
  }
  let _lock = settings_lock(server)?;
  let mut settings = read_settings(server)?;
  settings
    .extension_ids
    .insert(browser.name().into(), id.into());
  write_settings(server, &settings)?;
  println!("Native messaging manifest: {}", path.display());
  Ok(())
}
#[cfg(not(any(unix, windows)))]
fn install(
  _: &ResolvedServer,
  _: BrowserKind,
  _: &str,
  _: Option<&Path>,
) -> Result<()> {
  bail!("native messaging is supported on macOS, Linux and Windows")
}

/// Copied Windows host executable uses a pinned sidecar instead of shell/batch commands.
pub fn native_arguments() -> Option<Vec<std::ffi::OsString>> {
  #[cfg(windows)]
  {
    let exe = std::env::current_exe().ok()?;
    if !exe.file_stem()?.to_str()?.starts_with("onekey-native-") {
      return None;
    }
    let config: serde_json::Value =
      serde_json::from_slice(&fs::read(exe.with_extension("json")).ok()?).ok()?;
    let caller = std::env::args_os().nth(1)?;
    // Browser's additional --parent-window argument is intentionally not passed to clap.
    Some(vec![
      exe.into_os_string(),
      "--data-dir".into(),
      config["state"].as_str()?.into(),
      "--server".into(),
      config["server"].as_str()?.into(),
      "browser".into(),
      "host".into(),
      caller,
    ])
  }
  #[cfg(not(windows))]
  None
}

fn read_frame(input: &mut impl Read) -> Result<Option<Request>> {
  let mut header = [0; 4];
  if input.read(&mut header[..1])? == 0 {
    return Ok(None);
  }
  input
    .read_exact(&mut header[1..])
    .context("truncated native message header")?;
  let size = u32::from_ne_bytes(header) as usize;
  if size == 0 || size > MAX_FRAME {
    bail!("native message size is out of bounds");
  }
  let mut bytes = vec![0; size];
  input
    .read_exact(&mut bytes)
    .context("truncated native message")?;
  Ok(Some(
    serde_json::from_slice(&bytes).context("invalid native message")?,
  ))
}
fn write_frame(
  output: &mut impl Write,
  value: &serde_json::Value,
) -> Result<()> {
  let mut bytes = serde_json::to_vec(value)?;
  let result = (|| {
    // Chrome accepts at most 1 MiB from a native host.
    if bytes.len() > 1024 * 1024 {
      bail!("native response is too large");
    }
    output.write_all(&(bytes.len() as u32).to_ne_bytes())?;
    output.write_all(&bytes)?;
    output.flush()?;
    Ok(())
  })();
  bytes.zeroize();
  result
}
async fn serve(
  server: &ResolvedServer,
  input: &mut impl Read,
  output: &mut impl Write,
) -> Result<()> {
  while let Some(request) = read_frame(input)? {
    let result = handle(server, request).await;
    // Errors deliberately exclude upstream response bodies and secret/key values.
    let mut response = result.unwrap_or_else(|_| serde_json::json!({"ok":false,"error":"Request refused or credentials unavailable. Check the local binding, OneKey login, permissions and cache TTL."}));
    let written = write_frame(output, &response);
    if let Some(credentials) = response.get_mut("credentials") {
      for key in ["username", "password"] {
        if let Some(serde_json::Value::String(value)) = credentials.get_mut(key) {
          value.zeroize();
        }
      }
    }
    written?;
  }
  Ok(())
}
async fn handle(
  server: &ResolvedServer,
  request: Request,
) -> Result<serde_json::Value> {
  let settings = read_settings(server)?;
  match request {
    Request::List => Ok(
      serde_json::json!({"ok":true,"bindings":settings.bindings.into_iter().filter(|binding| binding.server_url == server.url).collect::<Vec<_>>()}),
    ),
    Request::Catalog { project } => catalog(server, project.as_deref()).await,
    Request::Save { mut binding } => {
      // Extension pages are the only mutation UI. Domain confirmation happens there.
      binding.server_url = server.url.clone();
      validate_binding(&mut binding)?;
      let _lock = settings_lock(server)?;
      let mut settings = read_settings(server)?;
      settings
        .bindings
        .retain(|item| item.name != binding.name || item.server_url != server.url);
      settings.bindings.push(binding);
      write_settings(server, &settings)?;
      Ok(serde_json::json!({"ok":true}))
    }
    Request::Remove { name } => {
      let _lock = settings_lock(server)?;
      let mut settings = read_settings(server)?;
      settings
        .bindings
        .retain(|item| item.name != name || item.server_url != server.url);
      write_settings(server, &settings)?;
      Ok(serde_json::json!({"ok":true}))
    }
    Request::Poll { browser } => poll(server, browser.as_deref()),
    Request::Check {
      name,
      origin,
      request_id,
    } => {
      let queued = read_queued(server, &request_id, "claimed")?;
      let binding = settings
        .bindings
        .iter()
        .find(|b| b.name == name && b.server_url == server.url && b.allow_ai && b.permits(&origin))
        .context("authorization revoked")?;
      let credential =
        client::credential_from_sources(None, Err(std::env::VarError::NotPresent), || {
          crate::cli::session::load(server)
        })?;
      if queued.name != name
        || queued.fingerprint != binding.fingerprint()?
        || queued.expires_at <= chrono::Utc::now().timestamp()
        || queue_path(server, &request_id, "result")?.exists()
        || queued.credential_hash
          != credential_hash(credential.token.as_deref().context("missing credential")?)
      {
        bail!("AI request refused");
      }
      Ok(serde_json::json!({"ok":true,"expiresAt":queued.expires_at}))
    }
    Request::Complete {
      id,
      outcome,
      reason,
    } => complete(server, &id, outcome, reason),
    Request::Fill {
      name,
      origin,
      fields,
      request_id,
    } => {
      let origin = normalize_origin(&origin)?;
      let binding = settings
        .bindings
        .iter()
        .find(|binding| binding.name == name && binding.server_url == server.url)
        .context("binding not found")?;
      if !binding.permits(&origin) {
        bail!("website or server origin does not match binding");
      }
      if let Some(id) = request_id.as_deref() {
        let queued = read_queued(server, id, "claimed")?;
        if !binding.allow_ai
          || queued.name != name
          || queued.expires_at <= chrono::Utc::now().timestamp()
          || queued.fingerprint != binding.fingerprint()?
        {
          bail!("AI authorization revoked or request expired");
        }
      }
      // AI requests must use the same saved identity as MCP. A human session
      // cannot silently expand the AI token scope or its server-selected TTL.
      let credential =
        client::credential_from_sources(None, Err(std::env::VarError::NotPresent), || {
          crate::cli::session::load(server)
        })?;
      if let Some(id) = request_id.as_deref() {
        let queued = read_queued(server, id, "claimed")?;
        let token = credential
          .token
          .as_deref()
          .context("missing bridge identity")?;
        if queued.credential_hash != credential_hash(token)
          || queue_path(server, id, "result")?.exists()
        {
          bail!("bridge and MCP credentials differ or request canceled");
        }
      }
      let api = client::authenticated_client(server, credential)?;
      let mut runtime = cache::load(server, &api, &binding.project).await?;
      let extract = |key: &str| -> Result<String> {
        Ok(
          runtime
            .entries
            .iter()
            .find(|entry| entry.key == key)
            .context("bound secret is unavailable")?
            .value
            .clone(),
        )
      };
      let values = (
        if matches!(fields, Fields::Both | Fields::Username) {
          extract(&binding.username_key)
        } else {
          Ok(String::new())
        },
        if matches!(fields, Fields::Both | Fields::Password) {
          extract(&binding.password_key)
        } else {
          Ok(String::new())
        },
      );
      for entry in &mut runtime.entries {
        entry.value.zeroize();
      }
      let current = read_settings(server)?
        .bindings
        .into_iter()
        .find(|item| item.name == name && item.server_url == server.url)
        .context("authorization revoked")?;
      if current.fingerprint()? != binding.fingerprint()? {
        bail!("authorization changed during fetch");
      }
      if let Some(id) = request_id.as_deref() {
        let queued = read_queued(server, id, "claimed")?;
        if queued.expires_at <= chrono::Utc::now().timestamp()
          || queue_path(server, id, "result")?.exists()
        {
          bail!("request expired or canceled");
        }
      }
      let (username, password) = (values.0?, values.1?);
      Ok(
        serde_json::json!({"ok":true,"origin":origin,"usernameSelector":binding.username_selector,"passwordSelector":binding.password_selector,"source":runtime.source.as_str(),"credentials":{"username":username,"password":password}}),
      )
    }
  }
}

#[derive(Debug, Default, Deserialize, Serialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
enum Fields {
  #[default]
  Both,
  Username,
  Password,
}
/// Why the extension refused an AI fill. A closed set, so no page or secret text can pass through.
#[derive(Debug, Deserialize, Serialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub enum RefusalReason {
  NotAuthorized,
  NotApproved,
  NoPermission,
  NoTab,
  MultipleTabs,
  NoFields,
  Bridge,
  TabChanged,
  AuthorizationChanged,
  Expired,
  FillFailed,
  Other,
}
#[derive(Debug, Deserialize, Serialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub enum Outcome {
  Filled,
  Refused,
  Unavailable,
  Canceled,
  Expired,
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Queued {
  id: String,
  name: String,
  server_url: String,
  fingerprint: String,
  expires_at: i64,
  credential_hash: String,
  browser: Option<String>,
}
fn private_write(
  path: &Path,
  bytes: &[u8],
) -> Result<()> {
  let temp = path.with_extension(format!("tmp-{}", ulid::Ulid::new()));
  let mut options = fs::OpenOptions::new();
  options.create_new(true).write(true);
  #[cfg(unix)]
  {
    use std::os::unix::fs::OpenOptionsExt;
    options.mode(0o600);
  }
  let result = (|| {
    let mut file = options.open(&temp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    fs::rename(&temp, path)?;
    Ok(())
  })();
  if temp.exists() {
    let _ = fs::remove_file(temp);
  }
  result
}
fn settings_lock(server: &ResolvedServer) -> Result<fs::File> {
  let path = settings_path(server)?.with_extension("lock");
  fs::create_dir_all(path.parent().unwrap())?;
  let lock = fs::OpenOptions::new()
    .create(true)
    .truncate(false)
    .read(true)
    .write(true)
    .open(path)?;
  lock.lock()?;
  Ok(lock)
}
fn validate_binding(binding: &mut Binding) -> Result<()> {
  if binding
    .browser
    .as_deref()
    .is_some_and(|browser| !matches!(browser, "chrome" | "edge"))
  {
    bail!("invalid browser");
  }
  for value in [
    &binding.name,
    &binding.project,
    &binding.username_key,
    &binding.password_key,
  ] {
    if value.trim().is_empty() || value.len() > 256 {
      bail!("invalid binding metadata");
    }
  }
  if binding.login_origins.len() > 16 || binding.submit_origins.len() > 16 {
    bail!("too many origins");
  }
  binding.origin = normalize_origin(&binding.origin)?;
  for origin in binding
    .login_origins
    .iter_mut()
    .chain(binding.submit_origins.iter_mut())
  {
    *origin = normalize_origin(origin)?;
  }
  for selector in [&binding.username_selector, &binding.password_selector]
    .into_iter()
    .flatten()
  {
    if selector.len() > 1024 {
      bail!("selector too long");
    }
  }
  Ok(())
}
async fn catalog(
  server: &ResolvedServer,
  project: Option<&str>,
) -> Result<serde_json::Value> {
  let credential =
    client::credential_from_sources(None, Err(std::env::VarError::NotPresent), || {
      crate::cli::session::load(server)
    })?;
  let api = client::authenticated_client(server, credential)?;
  if let Some(project) = project {
    let groups = api
      .request(Method::GET, &environments::list(Some(project)), None)
      .await?;
    let mut names = std::collections::BTreeSet::new();
    for group in groups.as_array().context("invalid group metadata")? {
      let id = group["id"].as_str().context("missing group ID")?;
      let values = api
        .request(Method::GET, &secrets::collection(id), None)
        .await?;
      for item in values.as_array().context("invalid secret metadata")? {
        if let Some(key) = item["key"].as_str() {
          names.insert(key.to_owned());
        }
      }
    }
    Ok(serde_json::json!({"ok":true,"keys":names}))
  } else {
    let projects = super::ls::visible_projects(&api)
      .await?
      .into_iter()
      .map(|(id, name)| serde_json::json!({"id":id,"name":name}))
      .collect::<Vec<_>>();
    Ok(serde_json::json!({"ok":true,"projects":projects}))
  }
}
fn queue_dir(server: &ResolvedServer) -> Result<PathBuf> {
  let dir = settings_path(server)?
    .parent()
    .unwrap()
    .join("browser-requests");
  fs::create_dir_all(&dir)?;
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))?;
  }
  Ok(dir)
}
fn queue_path(
  server: &ResolvedServer,
  id: &str,
  suffix: &str,
) -> Result<PathBuf> {
  let parsed: ulid::Ulid = id.parse().context("invalid request ID")?;
  if parsed.to_string() != id {
    bail!("invalid request ID");
  }
  Ok(queue_dir(server)?.join(format!("{id}.{suffix}")))
}
fn read_queued(
  server: &ResolvedServer,
  id: &str,
  suffix: &str,
) -> Result<Queued> {
  let queued: Queued = serde_json::from_slice(&fs::read(queue_path(server, id, suffix)?)?)?;
  if queued.server_url != server.url || queued.id != id {
    bail!("request belongs to another server");
  }
  Ok(queued)
}
pub fn connections(server: &ResolvedServer) -> Result<serde_json::Value> {
  Ok(
    serde_json::json!({"connections":read_settings(server)?.bindings.into_iter().filter(|b| b.server_url == server.url && b.enabled && b.allow_ai).map(|b| serde_json::json!({"name":b.name,"origin":b.origin,"loginOrigins":b.login_origins})).collect::<Vec<_>>()}),
  )
}
fn credential_hash(token: &str) -> String {
  hex::encode(Sha256::digest(token.as_bytes()))
}
fn requester_hash(server: &ResolvedServer) -> Result<String> {
  let credential = client::credential(server)?;
  let token = credential
    .token
    .as_deref()
    .context("scoped token required")?;
  if !token.starts_with(crate::constants::tokens::AGENT_TOKEN_PREFIX)
    && !token.starts_with(crate::constants::tokens::RUNNER_TOKEN_PREFIX)
  {
    bail!("scoped token required");
  }
  Ok(credential_hash(token))
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Recorded {
  server_url: String,
  credential_hash: String,
  expires_at: i64,
  outcome: Outcome,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  reason: Option<RefusalReason>,
}
const RESULT_RETENTION_SECONDS: i64 = 15 * 60;
pub fn enqueue_fill(
  server: &ResolvedServer,
  name: &str,
) -> Result<serde_json::Value> {
  let _lock = settings_lock(server)?;
  let binding = read_settings(server)?
    .bindings
    .into_iter()
    .find(|b| b.name == name && b.server_url == server.url && b.enabled && b.allow_ai)
    .context("Enable AI filling for this connection in the extension first")?;
  let credential_hash = requester_hash(server)?;
  let mut outstanding = 0;
  for entry in fs::read_dir(queue_dir(server)?)? {
    let path = entry?.path();
    // Bounded retention includes results, so an unused AI queue does not grow indefinitely.
    // Results outlive their request by RESULT_RETENTION_SECONDS so a later lookup still finds them.
    let grace = if path.extension().is_some_and(|ext| ext == "result") {
      RESULT_RETENTION_SECONDS
    } else {
      0
    };
    if let Ok(value) = fs::read(&path).and_then(|bytes| {
      serde_json::from_slice::<serde_json::Value>(&bytes).map_err(std::io::Error::other)
    }) && value["expiresAt"]
      .as_i64()
      .is_some_and(|expiry| expiry + grace <= chrono::Utc::now().timestamp())
    {
      let _ = fs::remove_file(&path);
      continue;
    }
    if matches!(
      path.extension().and_then(|s| s.to_str()),
      Some("pending" | "claimed")
    ) {
      outstanding += 1;
    }
  }
  if outstanding >= 32 {
    bail!("browser request queue is full");
  }
  let queued = Queued {
    id: ulid::Ulid::new().to_string(),
    name: name.into(),
    server_url: server.url.clone(),
    fingerprint: binding.fingerprint()?,
    expires_at: chrono::Utc::now().timestamp() + 75,
    credential_hash,
    browser: binding.browser,
  };
  private_write(
    &queue_path(server, &queued.id, "pending")?,
    &serde_json::to_vec(&queued)?,
  )?;
  Ok(serde_json::json!({"requestId":queued.id,"status":"pending","expiresAt":queued.expires_at}))
}
pub fn fill_result(
  server: &ResolvedServer,
  id: &str,
) -> Result<serde_json::Value> {
  let hash = requester_hash(server)?;
  let result = queue_path(server, id, "result")?;
  if result.exists() {
    let record: Recorded = serde_json::from_slice(&fs::read(result)?)?;
    if record.server_url != server.url || record.credential_hash != hash {
      bail!("request belongs to another identity");
    }
    return Ok(
      serde_json::json!({"requestId":id,"status":"complete","outcome":record.outcome,"reason":record.reason}),
    );
  }
  for suffix in ["pending", "claimed"] {
    if queue_path(server, id, suffix)?.exists() {
      let queued = read_queued(server, id, suffix)?;
      if queued.credential_hash != hash {
        bail!("request belongs to another identity");
      }
      return Ok(
        serde_json::json!({"requestId":id,"status":if queued.expires_at <= chrono::Utc::now().timestamp() { "expired" } else if suffix == "pending" { "pending" } else { "processing" }}),
      );
    }
  }
  Ok(serde_json::json!({"requestId":id,"status":"unknown"}))
}
pub fn cancel_fill(
  server: &ResolvedServer,
  id: &str,
) -> Result<serde_json::Value> {
  let _lock = settings_lock(server)?;
  let hash = requester_hash(server)?;
  for suffix in ["pending", "claimed"] {
    if queue_path(server, id, suffix)?.exists() {
      let queued = read_queued(server, id, suffix)?;
      if queued.credential_hash != hash {
        bail!("request belongs to another identity");
      }
      let record = Recorded {
        server_url: server.url.clone(),
        credential_hash: hash,
        expires_at: queued.expires_at,
        outcome: Outcome::Canceled,
        reason: None,
      };
      private_write(
        &queue_path(server, id, "result")?,
        &serde_json::to_vec(&record)?,
      )?;
      fs::remove_file(queue_path(server, id, suffix)?)?;
      return Ok(serde_json::json!({"requestId":id,"canceled":true}));
    }
  }
  Ok(serde_json::json!({"requestId":id,"canceled":false}))
}
pub async fn request_fill(
  server: &ResolvedServer,
  name: &str,
) -> Result<serde_json::Value> {
  let request = enqueue_fill(server, name)?;
  let id = request["requestId"].as_str().unwrap().to_owned();
  struct Cleanup(Vec<PathBuf>);
  impl Drop for Cleanup {
    fn drop(&mut self) {
      for path in &self.0 {
        let _ = fs::remove_file(path);
      }
    }
  }
  let _cleanup = Cleanup(
    ["pending", "claimed", "result"]
      .iter()
      .map(|suffix| queue_path(server, &id, suffix))
      .collect::<Result<Vec<_>>>()?,
  );
  for _ in 0..120 {
    let result = fill_result(server, &id)?;
    if result["status"] == "complete" {
      return Ok(serde_json::json!({"outcome":result["outcome"],"reason":result["reason"]}));
    }
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
  }
  bail!(
    "Browser did not respond within 60 seconds. Authorize the site in Chrome/Edge and leave one matching login tab open."
  )
}
pub fn browser_status(server: &ResolvedServer) -> Result<serde_json::Value> {
  let path = queue_dir(server)?.join(format!("heartbeat-{}.json", credential_hash(&server.url)));
  let heartbeat = fs::read(path)
    .ok()
    .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok());
  let last = heartbeat.and_then(|value| value["lastPollAt"].as_i64());
  let connected = last.is_some_and(|time| {
    let age = chrono::Utc::now().timestamp() - time;
    (0..=90).contains(&age)
  });
  Ok(
    serde_json::json!({"extensionRecentlyConnected":connected,"lastPollAt":last,"pollIntervalSeconds":30,"requestTtlSeconds":75,"requiresSameSavedToken":true,"autoSubmit":false}),
  )
}

fn poll(
  server: &ResolvedServer,
  browser: Option<&str>,
) -> Result<serde_json::Value> {
  let _lock = settings_lock(server)?;
  private_write(
    &queue_dir(server)?.join(format!("heartbeat-{}.json", credential_hash(&server.url))),
    &serde_json::to_vec(&serde_json::json!({"lastPollAt":chrono::Utc::now().timestamp()}))?,
  )?;
  let mut requests = Vec::new();
  for entry in fs::read_dir(queue_dir(server)?)?.take(256) {
    let path = entry?.path();
    let Some(id) = path.file_stem().and_then(|s| s.to_str()) else {
      continue;
    };
    if !matches!(
      path.extension().and_then(|s| s.to_str()),
      Some("pending" | "claimed")
    ) {
      continue;
    }
    let queued = match read_queued(server, id, path.extension().unwrap().to_str().unwrap()) {
      Ok(q) => q,
      Err(_) => continue,
    };
    if queued.expires_at <= chrono::Utc::now().timestamp() {
      let _ = fs::remove_file(path);
      continue;
    }
    if queued
      .browser
      .as_deref()
      .is_some_and(|target| Some(target) != browser)
    {
      continue;
    }
    if path.extension().unwrap() != "pending" {
      continue;
    }
    if fs::rename(&path, queue_path(server, id, "claimed")?).is_ok() {
      requests.push(serde_json::json!({"id":queued.id,"name":queued.name}));
    }
    if requests.len() >= 16 {
      break;
    }
  }
  Ok(serde_json::json!({"ok":true,"requests":requests}))
}
fn complete(
  server: &ResolvedServer,
  id: &str,
  outcome: Outcome,
  reason: Option<RefusalReason>,
) -> Result<serde_json::Value> {
  let _lock = settings_lock(server)?;
  let queued = read_queued(server, id, "claimed")?;
  if queue_path(server, id, "result")?.exists() {
    bail!("already completed");
  }
  if queued.expires_at <= chrono::Utc::now().timestamp() {
    bail!("request expired");
  }
  private_write(
    &queue_path(server, id, "result")?,
    &serde_json::to_vec(&Recorded {
      server_url: server.url.clone(),
      credential_hash: queued.credential_hash,
      expires_at: queued.expires_at,
      outcome,
      reason,
    })?,
  )?;
  fs::remove_file(queue_path(server, id, "claimed")?)?;
  Ok(serde_json::json!({"ok":true}))
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn origins_reject_paths_credentials_and_insecure_destinations() {
    assert_eq!(
      normalize_origin("https://EXAMPLE.com:443/").unwrap(),
      "https://example.com"
    );
    assert!(normalize_origin("http://127.0.0.1:8080").is_ok());
    for origin in [
      "http://example.com",
      "https://user:pass@example.com",
      "https://example.com/login",
      "https://example.com?q=1",
      "https://example.com#x",
      "file:///tmp/x",
      "https://*.example.com",
    ] {
      assert!(normalize_origin(origin).is_err(), "{origin}");
    }
  }
  #[test]
  fn windows_registration_staging_preserves_executable_and_independent_browser_ids() {
    let state = tempfile::TempDir::new().unwrap();
    let stage = tempfile::TempDir::new().unwrap();
    let server =
      crate::cli::local_config::resolve(Some("https://onekey.example.com"), Some(state.path()))
        .unwrap();
    for (browser, id) in [
      (BrowserKind::Chrome, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
      (BrowserKind::Edge, "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
    ] {
      install_windows(&server, browser, id, Some(stage.path())).unwrap();
      let manifest: serde_json::Value = serde_json::from_slice(
        &fs::read(
          stage
            .path()
            .join(format!("com.onekey.browser-{}.json", browser.name())),
        )
        .unwrap(),
      )
      .unwrap();
      let binary = PathBuf::from(manifest["path"].as_str().unwrap());
      assert!(binary.exists());
      assert_eq!(
        manifest["allowed_origins"][0],
        format!("chrome-extension://{id}/")
      );
      let sidecar: serde_json::Value =
        serde_json::from_slice(&fs::read(binary.with_extension("json")).unwrap()).unwrap();
      assert_eq!(sidecar["server"], server.url);
    }
    assert_eq!(read_settings(&server).unwrap().extension_ids.len(), 2);
  }
  #[test]
  fn bounded_framing_rejects_truncation_and_unknown_requests() {
    let mut bytes = Vec::new();
    write_frame(&mut bytes, &serde_json::json!({"action":"list"})).unwrap();
    assert!(matches!(
      read_frame(&mut &bytes[..]).unwrap(),
      Some(Request::List)
    ));
    assert!(read_frame(&mut &[][..]).unwrap().is_none());
    assert!(read_frame(&mut &[1, 0][..]).is_err());
    assert!(read_frame(&mut &(MAX_FRAME as u32 + 1).to_ne_bytes()[..]).is_err());
    let mut invalid = Vec::new();
    write_frame(&mut invalid, &serde_json::json!({"action":"fill","name":"x","origin":"https://example.com","passwordKey":"other"})).unwrap();
    assert!(read_frame(&mut &invalid[..]).is_err());
  }
}
