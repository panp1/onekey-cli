//! Local MCP server for scoped metadata discovery. Secret values never enter MCP responses.

use crate::{
  cli::{client, local_config},
  constants::{
    api::{environments, secrets},
    tokens::{AGENT_TOKEN_PREFIX, RUNNER_TOKEN_PREFIX},
  },
};
use anyhow::{Result, bail};
use clap::Subcommand;
use reqwest::Method;
use rmcp::{
  Json, ServerHandler, ServiceExt,
  handler::server::{router::tool::ToolRouter, wrapper::Parameters},
  schemars::{self, JsonSchema},
  tool, tool_handler, tool_router,
  transport::stdio,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Subcommand, Debug)]
pub enum McpCommand {
  /// Start a local MCP server on stdin/stdout.
  #[command(after_help = HELP)]
  Serve,
}

pub(crate) const HELP: &str = "\
Examples:
  onekey mcp serve                 # stdio MCP server for a local AI client

MCP client configuration (stdio subprocess):
  command: onekey
  args: [mcp, serve]

Use a scoped personal token saved with `onekey config`, or set ONEKEY_TOKEN.
Runner tokens also work. Human login sessions are deliberately refused.
Only project, group, and secret metadata are available; values are never returned.
";

pub(super) async fn execute(
  command: McpCommand,
  server: &local_config::ResolvedServer,
) -> Result<i32> {
  match command {
    McpCommand::Serve => serve(server).await,
  }
}

async fn serve(server: &local_config::ResolvedServer) -> Result<i32> {
  let credential = client::credential(server)?;
  let Some(token) = credential.token.as_deref() else {
    bail!("MCP requires a scoped personal or runner token. Configure one with `onekey config`.");
  };
  if !token.starts_with(AGENT_TOKEN_PREFIX) && !token.starts_with(RUNNER_TOKEN_PREFIX) {
    bail!(
      "MCP requires a scoped personal or runner token; human login sessions are not supported."
    );
  }
  client::validate_runner_token(token)?;
  let api = client::authenticated_client(server, credential)?;
  let service = OneKeyMcp::new(api).serve(stdio()).await?;
  service.waiting().await?;
  Ok(0)
}

#[derive(Clone)]
struct OneKeyMcp {
  api: Arc<client::ApiClient>,
  tool_router: ToolRouter<Self>,
}

impl OneKeyMcp {
  fn new(api: client::ApiClient) -> Self {
    Self {
      api: Arc::new(api),
      tool_router: Self::tool_router(),
    }
  }
}

#[derive(Debug, Deserialize, JsonSchema)]
struct PageRequest {
  /// Zero-based index of the first item. Defaults to 0.
  offset: Option<usize>,
  /// Number of items to return, from 1 to 100. Defaults to 50.
  limit: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct GroupsRequest {
  /// Project name or ID visible to the configured token.
  project: String,
  /// Zero-based index of the first group. Defaults to 0.
  offset: Option<usize>,
  /// Number of groups to return, from 1 to 100. Defaults to 50.
  limit: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct SecretsRequest {
  /// Project name or ID visible to the configured token.
  project: String,
  /// Group name or ID inside that project.
  group: String,
  /// Zero-based index of the first secret. Defaults to 0.
  offset: Option<usize>,
  /// Number of secrets to return, from 1 to 100. Defaults to 50.
  limit: Option<usize>,
}

#[derive(Debug, Serialize, JsonSchema)]
struct Page<T> {
  items: Vec<T>,
  total_count: usize,
  next_offset: Option<usize>,
}

#[derive(Debug, Serialize, JsonSchema)]
struct Project {
  id: String,
  name: String,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct Group {
  id: String,
  project_id: String,
  project_name: String,
  name: String,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct Secret {
  key: String,
  description: Option<String>,
  version: i64,
  updated_at: String,
}

fn page<T>(
  items: Vec<T>,
  offset: Option<usize>,
  limit: Option<usize>,
) -> Page<T> {
  let total_count = items.len();
  let offset = offset.unwrap_or(0).min(total_count);
  let limit = limit.unwrap_or(50).clamp(1, 100);
  let end = offset.saturating_add(limit).min(total_count);
  Page {
    items: items.into_iter().skip(offset).take(limit).collect(),
    total_count,
    next_offset: (end < total_count).then_some(end),
  }
}

#[tool_router(router = tool_router)]
impl OneKeyMcp {
  #[tool(
    name = "onekey_list_projects",
    description = "List projects visible to the configured OneKey token. Returns IDs and names only.",
    annotations(
      read_only_hint = true,
      destructive_hint = false,
      idempotent_hint = true
    )
  )]
  async fn list_projects(
    &self,
    Parameters(request): Parameters<PageRequest>,
  ) -> Result<Json<Page<Project>>, String> {
    let projects = super::ls::visible_projects(&self.api)
      .await
      .map_err(|error| format!("Could not list OneKey projects: {error:#}"))?
      .into_iter()
      .map(|(id, name)| Project { id, name })
      .collect();
    Ok(Json(page(projects, request.offset, request.limit)))
  }

  #[tool(
    name = "onekey_list_groups",
    description = "List groups in one visible OneKey project. Returns IDs and names only.",
    annotations(
      read_only_hint = true,
      destructive_hint = false,
      idempotent_hint = true
    )
  )]
  async fn list_groups(
    &self,
    Parameters(request): Parameters<GroupsRequest>,
  ) -> Result<Json<Page<Group>>, String> {
    if request.project.trim().is_empty() {
      return Err("project must be a non-empty name or ID".into());
    }
    let value = self
      .api
      .request(
        Method::GET,
        &environments::list(Some(&request.project)),
        None,
      )
      .await
      .map_err(|error| format!("Could not list OneKey groups: {error:#}"))?;
    let mut groups: Vec<Group> = serde_json::from_value(value)
      .map_err(|_| "OneKey returned invalid group metadata".to_owned())?;
    groups.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(Json(page(groups, request.offset, request.limit)))
  }

  #[tool(
    name = "onekey_list_secret_names",
    description = "List secret names, usage descriptions, and versions in one project group. Never returns secret values.",
    annotations(
      read_only_hint = true,
      destructive_hint = false,
      idempotent_hint = true
    )
  )]
  async fn list_secret_names(
    &self,
    Parameters(request): Parameters<SecretsRequest>,
  ) -> Result<Json<Page<Secret>>, String> {
    if request.project.trim().is_empty() || request.group.trim().is_empty() {
      return Err("project and group must be non-empty names or IDs".into());
    }
    let value = self
      .api
      .request(
        Method::GET,
        &environments::list(Some(&request.project)),
        None,
      )
      .await
      .map_err(|error| format!("Could not list OneKey groups: {error:#}"))?;
    let groups: Vec<Group> = serde_json::from_value(value)
      .map_err(|_| "OneKey returned invalid group metadata".to_owned())?;
    let group = groups
      .iter()
      .find(|group| group.id == request.group || group.name == request.group)
      .ok_or_else(|| "That group is not visible in the selected project".to_owned())?;
    let value = self
      .api
      .request(Method::GET, &secrets::collection(&group.id), None)
      .await
      .map_err(|error| format!("Could not list OneKey secret names: {error:#}"))?;
    let mut secrets: Vec<Secret> = serde_json::from_value(value)
      .map_err(|_| "OneKey returned invalid secret metadata".to_owned())?;
    secrets.sort_by(|a, b| a.key.cmp(&b.key));
    Ok(Json(page(secrets, request.offset, request.limit)))
  }
}

#[tool_handler(
  router = self.tool_router,
  name = "onekey",
  instructions = "Read-only access to project, group, and secret metadata. Never ask this server to reveal secret values."
)]
impl ServerHandler for OneKeyMcp {}
