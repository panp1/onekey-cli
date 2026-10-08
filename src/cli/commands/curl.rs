//! `onekey curl`: call an HTTP API with a secret as the auth header.
//! The header reaches curl through its stdin config (`-K -`), never argv or the child environment.

use super::run;
use crate::cli::local_config;
use anyhow::{Context, Result, bail};
use base64::{Engine, engine::general_purpose::STANDARD};
use clap::Args;
use std::{io::ErrorKind, process::Stdio};
use tokio::io::AsyncWriteExt;

#[derive(Args, Debug)]
pub struct CurlArgs {
  /// Project holding the secret. Defaults like `run`: ONEKEY_ENV, then the project saved with `onekey use`.
  #[arg(
    short = 'p',
    long = "project",
    visible_alias = "env",
    value_name = "PROJECT"
  )]
  pub environment: Option<String>,
  /// Runner or personal token for this invocation. Overrides ONEKEY_TOKEN and the saved credential.
  #[arg(short = 't', long, value_name = "TOKEN")]
  pub token: Option<String>,
  /// Basic auth. USER is literal, SECRET is the name of the secret holding the password.
  #[arg(
    long,
    value_name = "USER:SECRET",
    required_unless_present = "bearer",
    conflicts_with = "bearer"
  )]
  pub basic: Option<String>,
  /// Bearer auth. SECRET is the name of the secret holding the token.
  #[arg(long, value_name = "SECRET")]
  pub bearer: Option<String>,
  /// URL and any extra curl arguments, passed through unchanged.
  #[arg(
    required = true,
    trailing_var_arg = true,
    allow_hyphen_values = true,
    value_name = "URL|CURL_ARG"
  )]
  pub args: Vec<String>,
}

pub(crate) const HELP: &str = "\
Offline cache lifetime is configured by an administrator in Settings → Security.
Expired caches are refused; a lifetime of 0 disables offline fallback.

Examples:
  onekey curl --bearer github_token https://api.github.com/user
  onekey curl --basic me@example.com:jira_token https://example.atlassian.net/rest/api/3/myself
  onekey curl --basic :ado_pat https://dev.azure.com/org/_apis/projects -G -d api-version=7.1
  onekey curl -p payments --bearer api_key https://api.example.com/items -X POST -d '{}'

Request bodies cannot be read from stdin (`-d @-`): curl's stdin carries the auth header.
Verbose output (`-v`, `--verbose`, `--trace`, `--trace-ascii`) is refused: it would print the Authorization header.
";

pub(super) async fn execute(
  server: &local_config::ResolvedServer,
  args: CurlArgs,
) -> Result<i32> {
  let loaded = run::load(server, args.environment, args.token).await?;
  let secret = |name: &str| {
    loaded
      .entries
      .iter()
      .find(|entry| entry.key == name)
      .map(|entry| entry.value.as_str())
      .with_context(|| {
        format!(
          "secret {name} not found in {}/{}",
          loaded.project, loaded.environment
        )
      })
  };
  let header = match (&args.basic, &args.bearer) {
    (Some(basic), _) => {
      let (user, name) = basic
        .split_once(':')
        .context("--basic expects USER:SECRET")?;
      format!(
        "Basic {}",
        STANDARD.encode(format!("{user}:{}", secret(name)?))
      )
    }
    (None, Some(name)) => format!("Bearer {}", secret(name)?),
    (None, None) => bail!("pass --basic USER:SECRET or --bearer SECRET"),
  };
  if let Some(flag) = verbose_flag(&args.args) {
    bail!("{flag} would print the Authorization header; drop it");
  }
  let config = curl_config(&header)?;

  let mut child = tokio::process::Command::new("curl")
    .args(["-sS", "-K", "-"])
    .args(&args.args)
    .stdin(Stdio::piped())
    .spawn()
    .context("failed to start curl; is it installed?")?;
  let mut stdin = child.stdin.take().context("curl stdin unavailable")?;
  // curl exits before reading its config on a bad flag: the pipe breaks, curl's own
  // error and exit code are what the user needs.
  let written = stdin.write_all(config.as_bytes()).await;
  drop(stdin);
  let status = child.wait().await?;
  match written {
    Err(error) if error.kind() != ErrorKind::BrokenPipe => Err(error.into()),
    _ => Ok(status.code().unwrap_or(1)),
  }
}

/// The first pass-through argument that would make curl print the request headers.
/// Short-option clusters containing `v` (`-sv`) are refused too.
pub fn verbose_flag(args: &[String]) -> Option<&str> {
  args.iter().map(String::as_str).find(|arg| {
    if let Some(long) = arg.strip_prefix("--") {
      let name = long.split_once('=').map_or(long, |(name, _)| name);
      return matches!(name, "verbose" | "trace" | "trace-ascii");
    }
    arg.len() > 1 && arg.starts_with('-') && arg.contains('v')
  })
}

/// One curl config line carrying the Authorization header.
pub fn curl_config(header: &str) -> Result<String> {
  if header.contains(['\r', '\n']) {
    bail!("secret contains a line break and cannot be used as an HTTP header");
  }
  let quoted = header.replace('\\', "\\\\").replace('"', "\\\"");
  Ok(format!("header = \"Authorization: {quoted}\"\n"))
}
