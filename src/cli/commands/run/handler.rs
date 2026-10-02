use super::RunArgs;
use super::cache::{self as runtime_cache, RuntimeLoad, RuntimeSource};
use crate::cli::{client, local_config};
use crate::constants::config::ENV_RUN_ENVIRONMENT;
use anyhow::{Context, Result, bail};
use std::env;

pub(crate) async fn execute(
  server: &local_config::ResolvedServer,
  args: RunArgs,
) -> Result<i32> {
  let RunArgs {
    environment,
    token,
    shell,
    command,
  } = args;
  let command = match shell {
    Some(script) => shell_command(script),
    None => command,
  };
  let loaded = load(server, environment, token).await?;
  let program = command.first().context("run requires a child command")?;
  let mut child_command = tokio::process::Command::new(program);
  child_command.args(&command[1..]).env_remove("ONEKEY_TOKEN");
  for entry in loaded.entries {
    child_command.env(entry.key, entry.value);
  }
  #[cfg(unix)]
  {
    use std::os::unix::process::CommandExt;

    let error = child_command.as_std_mut().exec();
    Err(error).with_context(|| format!("failed to start {program}"))
  }
  #[cfg(not(unix))]
  {
    let mut child = child_command
      .spawn()
      .with_context(|| format!("failed to start {program}"))?;
    Ok(child.wait().await?.code().unwrap_or(1))
  }
}

/// `-c SCRIPT`: `sh -c` everywhere it exists (macOS, Linux, Git Bash on Windows);
/// plain Windows falls back to `cmd /C`, where variables are `%NAME%`.
fn shell_command(script: String) -> Vec<String> {
  #[cfg(windows)]
  {
    let has_sh = env::var_os("PATH")
      .is_some_and(|path| env::split_paths(&path).any(|dir| dir.join("sh.exe").is_file()));
    if !has_sh {
      return vec!["cmd".into(), "/C".into(), script];
    }
  }
  vec!["sh".into(), "-c".into(), script]
}

/// Resolve the target environment, fetch its secrets (or fall back to the
/// encrypted cache), and print the one-line summary to stderr.
pub(crate) async fn load(
  server: &local_config::ResolvedServer,
  environment: Option<String>,
  token: Option<String>,
) -> Result<RuntimeLoad> {
  let api = client::any_authenticated_client(server, token).await?;
  let selection = run_environment(
    environment,
    env::var(ENV_RUN_ENVIRONMENT),
    server.default_environment(),
  )?;
  let loaded = runtime_cache::load(server, &api, &selection.reference).await;
  let loaded = match loaded {
    Err(error) if client::is_authentication_error(&error) => {
      bail!("OneKey authentication failed. Run `onekey login` again or set a valid ONEKEY_TOKEN.")
    }
    Err(error)
      if selection.source == RunEnvironmentSource::Default
        && (error.to_string().contains("ENVIRONMENT_NOT_FOUND")
          || error.to_string().contains("PROJECT_NOT_FOUND")) =>
    {
      bail!(
        "Saved default project {} is unavailable. Set a new one with `onekey use <project>` or clear it with `onekey use --clear`.",
        selection.reference
      )
    }
    result => result?,
  };
  match &loaded.source {
    RuntimeSource::Live { cache_warning } => {
      if let Some(warning) = cache_warning {
        eprintln!("OneKey warning: {warning}");
      }
    }
    RuntimeSource::Cache {
      fetched_at,
      age,
      reason,
    } => {
      eprintln!(
        "OneKey warning: {reason}. Using encrypted cache fetched at {fetched_at} ({age} old)."
      );
    }
  }
  eprintln!(
    "OneKey: {} [{}] ({}), {} key(s), source={}",
    loaded.project,
    loaded.environment,
    loaded.environment_id,
    loaded.entries.len(),
    loaded.source.as_str(),
  );
  Ok(loaded)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RunEnvironmentSource {
  Argument,
  Environment,
  Default,
}

pub struct RunEnvironment {
  pub reference: String,
  source: RunEnvironmentSource,
}

pub fn run_environment(
  argument: Option<String>,
  environment: Result<String, env::VarError>,
  default: Option<&str>,
) -> Result<RunEnvironment> {
  if let Some(reference) = argument {
    return Ok(RunEnvironment {
      reference,
      source: RunEnvironmentSource::Argument,
    });
  }
  match environment {
    Ok(reference) if reference.is_empty() => bail!("ONEKEY_ENV is set but empty"),
    Ok(reference) => {
      return Ok(RunEnvironment {
        reference,
        source: RunEnvironmentSource::Environment,
      });
    }
    Err(env::VarError::NotUnicode(_)) => bail!("ONEKEY_ENV contains invalid Unicode"),
    Err(env::VarError::NotPresent) => {}
  }
  if let Some(reference) = default {
    return Ok(RunEnvironment {
      reference: reference.into(),
      source: RunEnvironmentSource::Default,
    });
  }
  bail!("No default project is set. Set one with: onekey use <project>")
}
