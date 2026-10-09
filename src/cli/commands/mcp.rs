//! Local MCP metadata and authorized browser filling. Secret values never enter MCP responses.

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
Metadata and authorized browser filling are available; values are never returned.
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
  let service = OneKeyMcp::new(api, server).serve(stdio()).await?;
  service.waiting().await?;
  Ok(0)
}

#[derive(Clone)]
struct OneKeyMcp {
  api: Arc<client::ApiClient>,
  tool_router: ToolRouter<Self>,
  server: Arc<local_config::ResolvedServer>,
}

impl OneKeyMcp {
  fn new(
    api: client::ApiClient,
    server: &local_config::ResolvedServer,
  ) -> Self {
    Self {
      api: Arc::new(api),
      server: Arc::new(local_config::ResolvedServer {
        url: server.url.clone(),
        source: server.source,
        config_path: server.config_path.clone(),
        config: server.config.clone(),
      }),
      tool_router: Self::tool_router(),
    }
  }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct BrowserRequestId {
  /// requestId returned by onekey_request_browser_fill.
  request_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct BrowserFillRequest {
  /// Name of an existing user-authorized browser connection.
  name: String,
  /// Browser chosen by the user for this request: chrome or edge. Never switches browsers automatically.
  browser: super::browser::BrowserKind,
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
    name = "onekey_get_browser_status",
    description = "Check, per browser (chrome, edge), whether the OneKey extension connected recently, its version and whether it is outdated, plus request TTL and setup requirements. Returns no credentials.",
    annotations(
      read_only_hint = true,
      destructive_hint = false,
      idempotent_hint = true
    )
  )]
  async fn browser_status(&self) -> Result<Json<serde_json::Value>, String> {
    super::browser::browser_status(&self.server)
      .map(Json)
      .map_err(|_| "Could not inspect browser bridge".into())
  }
  #[tool(
    name = "onekey_request_browser_fill",
    description = "Queue filling by authorized connection name in the user-selected browser (chrome or edge) and immediately return a requestId. The same connection can be used in both browsers. Only the selected browser may claim the request; no fallback to another browser. Executes in one existing authorized tab within 75 seconds. Poll onekey_get_browser_fill_result. Never submits or returns credentials.",
    annotations(
      read_only_hint = false,
      destructive_hint = false,
      idempotent_hint = false
    )
  )]
  async fn request_browser_fill(
    &self,
    Parameters(request): Parameters<BrowserFillRequest>,
  ) -> Result<Json<serde_json::Value>, String> {
    super::browser::enqueue_fill(&self.server,&request.name,request.browser).map(Json).map_err(|error| format!("Request refused: {error}. Select chrome or edge, approve AI filling for this connection in that browser's extension and use the same saved PAT/runner token for MCP and native host."))
  }
  #[tool(
    name = "onekey_get_browser_fill_result",
    description = "Get pending, processing, complete, expired or unknown status for a browser fill requestId. Complete outcome is filled/refused/unavailable/canceled; a refusal carries a reason code (notAuthorized, notApproved, noPermission, noTab, multipleTabs, noFields, bridge, tabChanged, authorizationChanged, expired, fillFailed, other). No page content or credentials returned.",
    annotations(
      read_only_hint = true,
      destructive_hint = false,
      idempotent_hint = true
    )
  )]
  async fn browser_fill_result(
    &self,
    Parameters(request): Parameters<BrowserRequestId>,
  ) -> Result<Json<serde_json::Value>, String> {
    super::browser::fill_result(&self.server, &request.request_id)
      .map(Json)
      .map_err(|_| "Invalid request ID or identity".into())
  }
  #[tool(
    name = "onekey_cancel_browser_fill",
    description = "Cancel a pending browser fill requestId. Cannot undo credentials already inserted into page inputs. Returns only cancellation status.",
    annotations(
      read_only_hint = false,
      destructive_hint = false,
      idempotent_hint = true
    )
  )]
  async fn cancel_browser_fill(
    &self,
    Parameters(request): Parameters<BrowserRequestId>,
  ) -> Result<Json<serde_json::Value>, String> {
    super::browser::cancel_fill(&self.server, &request.request_id)
      .map(Json)
      .map_err(|_| "Invalid request ID or identity".into())
  }
  #[tool(
    name = "onekey_list_browser_connections",
    description = "List enabled website connections explicitly authorized for AI filling. Returns connection names and approved origins, never credentials.",
    annotations(
      read_only_hint = true,
      destructive_hint = false,
      idempotent_hint = true
    )
  )]
  async fn list_browser_connections(&self) -> Result<Json<serde_json::Value>, String> {
    super::browser::connections(&self.server)
      .map(Json)
      .map_err(|_| "Could not read browser authorizations".into())
  }
  #[tool(
    name = "onekey_fill_browser_connection",
    description = "Fill an authorized connection in one existing login tab in the user-selected browser (chrome or edge). The connection is shared; never switches browsers automatically. Does not submit. Supports username/password steps. Returns only filled/refused/unavailable plus a refusal reason code; never credentials. Requires AI filling approval and website permissions in the selected browser's extension. Waits up to 60 seconds.",
    annotations(
      read_only_hint = false,
      destructive_hint = false,
      idempotent_hint = false
    )
  )]
  async fn fill_browser_connection(
    &self,
    Parameters(request): Parameters<BrowserFillRequest>,
  ) -> Result<Json<serde_json::Value>, String> {
    let outcome = super::browser::request_fill(&self.server, &request.name, request.browser).await.map_err(|error| format!("Fill refused or timed out: {error}. Authorize the connection and AI filling in the selected browser's extension; keep one matching login tab open in that browser."))?;
    Ok(Json(outcome))
  }
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
      .map_err(|_| "Could not list OneKey projects. Check login and token scope.".to_owned())?
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
      .map_err(|_| "Could not list OneKey groups. Check login and token scope.".to_owned())?;
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
      .map_err(|_| "Could not list OneKey groups. Check login and token scope.".to_owned())?;
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
      .map_err(|_| "Could not list OneKey secret names. Check login and token scope.".to_owned())?;
    let mut secrets: Vec<Secret> = serde_json::from_value(value)
      .map_err(|_| "OneKey returned invalid secret metadata".to_owned())?;
    secrets.sort_by(|a, b| a.key.cmp(&b.key));
    Ok(Json(page(secrets, request.offset, request.limit)))
  }
}

#[tool_handler(
  router = self.tool_router,
  name = "onekey",
  instructions = "Discover metadata and fill explicitly authorized browser connections. Browser filling changes page inputs but never submits. Never return secret values."
)]
impl ServerHandler for OneKeyMcp {}
