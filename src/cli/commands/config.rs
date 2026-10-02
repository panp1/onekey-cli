//! `onekey config` (one-step setup: server, token, default environment into a profile)
//! and `onekey profile` (list / use / remove). A profile is a client data directory:
//! `default` is `~/.onekey`, others live in `~/.onekey/profiles/<name>`.

use crate::cli::{
  client::{self, ApiClient, CliCancelled},
  local_config::{self, ClientConfig, DefaultEnvironment, ResolvedServer, ServerSource},
  output, prompt,
};
use crate::config::{
  active_profile, list_profiles, onekey_home, profile_dir, saved_active_profile,
  set_active_profile, validate_profile_name,
};
use crate::constants::config::{CLIENT_CONFIG_FILENAME, DEFAULT_PROFILE, ENV_DATA_DIR};
use anyhow::{Context, Result, bail};
use clap::{Args, Subcommand};
use inquire::{Select, Text};
use serde_json::{Value, json};
use std::io::{self, IsTerminal};

#[derive(Args, Debug)]
pub struct ConfigArgs {
  /// Profile to configure. Defaults to the active profile.
  #[arg(short, long, value_name = "NAME")]
  pub profile: Option<String>,
  /// Server URL, e.g. https://onekey.example.com/cli
  #[arg(long, value_name = "URL")]
  pub url: Option<String>,
  /// Read the token (personal or runner) from standard input instead of prompting.
  #[arg(long)]
  pub token_stdin: bool,
  /// Default project (name or ID).
  #[arg(long = "project", visible_alias = "env", value_name = "PROJECT")]
  pub environment: Option<String>,
  /// Never prompt: fail if --url or the token is missing; without --project, keep the saved default if it is still visible.
  #[arg(short, long)]
  pub yes: bool,
}

pub(crate) const CONFIG_HELP: &str = "\
Examples:
  onekey config
  onekey config --profile public --url https://sso.example.com/apps/onekey/cli
  printf '%s' \"$TOKEN\" | onekey config -p ci --url https://onekey.example.com --token-stdin --project app --yes

Tokens come from the console: Account -> Personal tokens (or a project's runner tokens).
";

#[derive(Subcommand, Debug)]
pub enum ProfileCommand {
  /// List profiles with their server and default environment.
  #[command(after_help = "Examples:\n  onekey profile list\n  onekey --json profile list\n")]
  List,
  /// Make a profile the active one for later commands.
  #[command(after_help = "Examples:\n  onekey profile use public\n  onekey profile use default\n")]
  Use {
    /// Profile name (`default` is ~/.onekey itself).
    #[arg(value_name = "NAME")]
    name: String,
  },
  /// Delete a profile's saved server, token and cache.
  #[command(
    after_help = "Examples:\n  onekey profile remove old-vpn\n  onekey profile remove ci --yes\n"
  )]
  Remove {
    /// Profile name to delete (not `default`).
    #[arg(value_name = "NAME")]
    name: String,
    /// Skip the confirmation prompt.
    #[arg(short, long)]
    yes: bool,
  },
}

pub(crate) const PROFILE_HELP: &str = "\
Examples:
  onekey profile list
  onekey profile use public
  ONEKEY_PROFILE=vpn onekey ls        # one command against another profile
";

/// Profiles always live in the OneKey home; a data-dir override would write the profile
/// somewhere later commands never read.
pub(crate) fn reject_data_dir_override() -> Result<()> {
  if std::env::var_os(ENV_DATA_DIR).is_some() {
    bail!("unset {ENV_DATA_DIR} to manage profiles; they live in the OneKey home");
  }
  Ok(())
}

fn server_for(
  dir: &std::path::Path,
  url: &str,
) -> Result<ResolvedServer> {
  let config_path = dir.join(CLIENT_CONFIG_FILENAME);
  Ok(ResolvedServer {
    url: url.to_owned(),
    source: ServerSource::Config,
    config: local_config::read(&config_path)?,
    config_path,
  })
}

/// Servers already saved in any profile, for the picker.
fn known_servers(home: &std::path::Path) -> Vec<String> {
  let mut urls: Vec<String> = list_profiles(home)
    .iter()
    .filter_map(|name| profile_dir(home, name).ok())
    .filter_map(|dir| local_config::read(&dir.join(CLIENT_CONFIG_FILENAME)).ok())
    .filter_map(|config| config.server_url)
    .collect();
  urls.sort();
  urls.dedup();
  urls
}

fn cancelled(error: inquire::InquireError) -> anyhow::Error {
  match error {
    inquire::InquireError::OperationCanceled | inquire::InquireError::OperationInterrupted => {
      CliCancelled::Confirmation.into()
    }
    error => anyhow::anyhow!(error),
  }
}

const OTHER_SERVER: &str = "Other URL…";

pub(super) async fn configure(
  args: ConfigArgs,
  json_output: bool,
) -> Result<i32> {
  let home = onekey_home();
  let profile = match args.profile {
    Some(name) => name,
    None => active_profile(&home)?,
  };
  validate_profile_name(&profile)?;
  let dir = profile_dir(&home, &profile)?;
  let existing = local_config::read(&dir.join(CLIENT_CONFIG_FILENAME))?;
  let interactive = !args.yes && io::stdin().is_terminal();

  // 1. Server
  let raw_url = match (args.url, interactive) {
    (Some(url), _) => url,
    (None, false) => existing
      .server_url
      .clone()
      .context("pass --url (no server saved in this profile)")?,
    (None, true) => {
      let mut options = known_servers(&home);
      options.push(OTHER_SERVER.to_owned());
      let default = existing
        .server_url
        .as_ref()
        .and_then(|url| options.iter().position(|o| o == url))
        .unwrap_or(0);
      let picked = Select::new(&format!("Server for profile '{profile}':"), options)
        .with_starting_cursor(default)
        .prompt()
        .map_err(cancelled)?;
      if picked == OTHER_SERVER {
        Text::new("Server URL:")
          .with_help_message("e.g. https://onekey.example.com/cli")
          .prompt()
          .map_err(cancelled)?
      } else {
        picked
      }
    }
  };
  let url = local_config::normalize_connect_target(raw_url.trim())?;
  if url.starts_with("http://") {
    output::print_warning(&super::client::insecure_transport_warning(&url));
  }
  let server = server_for(&dir, &url)?;
  let health = ApiClient::new(&server, None)?.health().await?;
  if health.get("product").and_then(Value::as_str) != Some("onekey") {
    bail!("{url} is not a OneKey server");
  }

  // 2. Token
  let token = if args.token_stdin {
    let mut value = prompt::read_secret_stdin("token")?;
    prompt::remove_one_line_ending(&mut value);
    value
  } else if interactive {
    prompt::password(
      "Token (Account → Personal tokens, or a runner token):",
      false,
      CliCancelled::TokenInput,
    )?
  } else {
    bail!("pass the token with --token-stdin");
  };
  let token = token.trim().to_owned();
  if token.is_empty() {
    bail!("the token is empty");
  }
  let api = ApiClient::new(&server, Some(token.clone()))?;
  let projects = super::ls::visible_projects(&api)
    .await
    .context("the server rejected the token")?;

  // 3. Default project
  let chosen = match (&args.environment, interactive, projects.len()) {
    (Some(reference), _, _) => Some(
      projects
        .iter()
        .find(|(id, name)| id == reference || name == reference)
        .cloned()
        .with_context(|| format!("project {reference} is not visible to this token"))?,
    ),
    (None, _, 0) => None,
    // Non-interactive re-run (e.g. token rotation): keep a still-visible default.
    (None, false, _) => existing
      .default_environment
      .as_ref()
      .filter(|default| default.server_url == url)
      .and_then(|default| {
        projects
          .iter()
          .find(|(id, _)| *id == default.environment_id)
      })
      .cloned(),
    (None, true, 1) => projects.first().cloned(),
    (None, true, _) => {
      let names: Vec<String> = projects.iter().map(|(_, name)| name.clone()).collect();
      let name = Select::new("Default project:", names)
        .prompt()
        .map_err(cancelled)?;
      projects.iter().find(|(_, n)| *n == name).cloned()
    }
  };

  // 4. Save
  let config = ClientConfig {
    version: 1,
    server_url: Some(url.clone()),
    default_environment: chosen.as_ref().map(|(id, _)| DefaultEnvironment {
      server_url: url.clone(),
      environment_id: id.clone(),
    }),
  };
  // Credential first: a config pointing at a server with no token is worse than no change.
  client::save_credential(&server_for(&dir, &url)?, &token, None)?;
  local_config::write(&dir.join(CLIENT_CONFIG_FILENAME), &config)?;
  // A scripted `config -p ci --yes` must not flip an interactive user's active profile.
  let saved = saved_active_profile(&home);
  let activate = interactive || saved.is_none() || saved.as_deref() == Some(profile.as_str());
  if activate {
    set_active_profile(&home, &profile)?;
  }

  if json_output {
    output::print_json(&json!({
      "profile": profile,
      "active": activate,
      "server": url,
      "project": chosen.as_ref().map(|(_, name)| name),
      "projects": projects.len(),
    }))?;
  } else {
    if activate {
      println!("Profile '{profile}' is active.");
    } else {
      println!("Profile '{profile}' saved (activate with `onekey profile use {profile}`).");
    }
    println!("  Server:       {url}");
    println!(
      "  Project:      {}",
      chosen
        .as_ref()
        .map_or("(none; pass one to run, or `onekey use`)", |(_, n)| n
          .as_str())
    );
    println!("  Visible:      {} project(s)", projects.len());
    println!("Try: onekey ls");
  }
  Ok(0)
}

pub(super) fn profile(
  command: ProfileCommand,
  json_output: bool,
) -> Result<i32> {
  let home = onekey_home();
  match command {
    ProfileCommand::List => {
      let active = active_profile(&home)?;
      let rows: Vec<Value> = list_profiles(&home)
        .into_iter()
        .map(|name| {
          let config = profile_dir(&home, &name)
            .ok()
            .and_then(|dir| local_config::read(&dir.join(CLIENT_CONFIG_FILENAME)).ok());
          json!({
            "name": name,
            "active": name == active,
            "server": config.as_ref().and_then(|c| c.server_url.clone()),
            "project": config.as_ref().and_then(|c| c.default_environment.as_ref().map(|d| d.environment_id.clone())),
          })
        })
        .collect();
      if json_output {
        output::print_json(&Value::Array(rows))?;
      } else {
        let table = rows
          .iter()
          .map(|row| {
            let text = |f: &str| row[f].as_str().unwrap_or("-").to_owned();
            vec![
              format!(
                "{}{}",
                if row["active"] == true { "* " } else { "  " },
                text("name")
              ),
              text("server"),
              text("project"),
            ]
          })
          .collect::<Vec<_>>();
        output::print_table(
          &["PROFILE", "SERVER", "PROJECT"],
          &table,
          "No profiles.",
          "* = active",
        );
      }
    }
    ProfileCommand::Use { name } => {
      let dir = profile_dir(&home, &name)?;
      if name != DEFAULT_PROFILE && !dir.is_dir() {
        bail!("profile '{name}' does not exist. Create it with: onekey config --profile {name}");
      }
      set_active_profile(&home, &name)?;
      println!("Profile '{name}' is active.");
    }
    ProfileCommand::Remove { name, yes } => {
      if name == DEFAULT_PROFILE {
        bail!("the default profile lives in the OneKey home and cannot be removed");
      }
      let dir = profile_dir(&home, &name)?;
      if !dir.is_dir() {
        bail!("profile '{name}' does not exist");
      }
      prompt::confirm(
        &format!("Delete profile '{name}' (server, token, cache)?"),
        yes,
      )?;
      std::fs::remove_dir_all(&dir)?;
      // Compare with the saved name, not ONEKEY_PROFILE, so no dangling pointer remains.
      if saved_active_profile(&home).as_deref() == Some(name.as_str()) {
        set_active_profile(&home, DEFAULT_PROFILE)?;
      }
      println!("Profile '{name}' removed.");
    }
  }
  Ok(0)
}
