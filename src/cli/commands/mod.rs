//! Feature-owned CLI command modules.
//!
//! Each command keeps its Clap arguments, help text, handler, and private
//! support code together. This file only routes parsed commands and re-exports
//! the small set of helpers used outside their owning module.

pub mod auth;
pub mod backup;
pub mod cache;
pub mod client;
pub mod config;
pub mod curl;
pub mod environment;
pub mod export;
pub mod import;
pub mod init;
pub mod ls;
pub mod mcp;
pub mod project;
pub mod restore;
pub mod run;
pub mod secret;
pub mod token;
pub mod update;

#[doc(hidden)]
pub use super::output::{render_fields, render_table};
#[doc(hidden)]
pub use super::prompt::remove_one_line_ending;
pub use client::{insecure_transport_warning, server_switch_confirmed, status_document};
pub use run::{RunEnvironment, run_environment};

use super::{args::*, local_config};
use anyhow::{Result, bail};

pub async fn execute(cli: Cli) -> Result<i32> {
  let server_argument = cli.server.clone();
  let data_dir = cli.data_dir.clone();
  let json_output = cli.json;
  match cli.command {
    Command::Client {
      command: ClientCommand::Connect { server_url },
    } => {
      if server_argument.is_some() {
        bail!(
          "--server cannot be used with `onekey client connect`. Pass the destination as the positional server URL"
        );
      }
      client::connect(&server_url, data_dir.as_deref(), json_output).await?;
      Ok(0)
    }
    Command::Client {
      command: ClientCommand::Status,
    }
    | Command::Status => {
      client::show_status(server_argument.as_deref(), data_dir.as_deref(), json_output).await?;
      Ok(0)
    }
    Command::Login(args) => {
      let server = local_config::resolve(server_argument.as_deref(), data_dir.as_deref())?;
      auth::login(&server, args, json_output).await
    }
    Command::Logout => {
      let server = local_config::resolve(server_argument.as_deref(), data_dir.as_deref())?;
      auth::logout(&server, json_output)
    }
    Command::Update => update::run(json_output).await,
    Command::Config(args) => {
      if server_argument.is_some() || data_dir.is_some() {
        bail!(
          "use `onekey config --url` instead of --server; profiles always live in the OneKey home"
        );
      }
      config::reject_data_dir_override()?;
      config::configure(args, json_output).await
    }
    Command::Profile { command } => {
      config::reject_data_dir_override()?;
      config::profile(command, json_output)
    }
    command => {
      let server = local_config::resolve(server_argument.as_deref(), data_dir.as_deref())?;
      execute_client(command, &server, json_output).await
    }
  }
}

async fn execute_client(
  command: Command,
  server: &local_config::ResolvedServer,
  json_output: bool,
) -> Result<i32> {
  match command {
    Command::Init(args) => init::execute(server, args, json_output).await,
    Command::Project { command } => project::execute(command, server, json_output).await,
    Command::Env { command } => environment::execute(command, server, json_output).await,
    Command::Secret { command } => secret::execute(command, server, json_output).await,
    Command::Import(args) => import::execute(server, args, json_output).await,
    Command::Export(args) => export::execute(server, args, json_output).await,
    Command::Token { command } => token::execute(command, server, json_output).await,
    Command::Run(args) => run::execute(server, args).await,
    Command::Curl(args) => curl::execute(server, args).await,
    Command::Ls(args) => ls::execute(server, args, json_output).await,
    Command::Mcp { command } => mcp::execute(command, server).await,
    Command::Use { environment, clear } => {
      ls::use_project(server, environment, clear, json_output).await
    }
    Command::Cache { command } => cache::execute(command, server, json_output),
    Command::Backup(args) => backup::execute(server, args, json_output).await,
    Command::Restore(args) => restore::execute(server, args, json_output).await,
    _ => bail!("unsupported command"),
  }
}
