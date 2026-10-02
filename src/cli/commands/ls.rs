//! `onekey ls`: projects, groups, secret names and usage descriptions the current credential
//! can see. Never values.

use crate::cli::{client, local_config};
use crate::constants::api::{environments, projects, secrets};
use anyhow::Result;
use clap::Args;
use reqwest::Method;
use serde_json::{Value, json};

#[derive(Args, Debug)]
pub struct LsArgs {
  /// Limit to one project (name or ID).
  #[arg(value_name = "PROJECT")]
  pub target: Option<String>,
  /// Token for this invocation. Overrides ONEKEY_TOKEN and the saved credential.
  #[arg(short = 't', long, value_name = "TOKEN")]
  pub token: Option<String>,
}

pub(crate) const HELP: &str = "\
Examples:
  onekey ls
  onekey ls payment-service

A secret's usage description, when set, is printed indented under its name.

The server scopes the listing: runner tokens see their project, personal tokens
their scope, people the projects they belong to (admins: all).
";

pub(super) async fn execute(
  server: &local_config::ResolvedServer,
  args: LsArgs,
  json_output: bool,
) -> Result<i32> {
  let credential = client::credential_with_token(server, args.token)?;
  let api = client::authenticated_client(server, credential)?;
  // project id -> (name, secrets tagged with their group)
  let mut projects: std::collections::BTreeMap<String, (String, Vec<Value>)> = Default::default();
  for env in api
    .request(Method::GET, environments::COLLECTION, None)
    .await?
    .as_array()
    .into_iter()
    .flatten()
  {
    let (project_id, project, group, id) = (
      text(env, "projectId"),
      text(env, "projectName"),
      text(env, "name"),
      text(env, "id"),
    );
    if !matches_target(args.target.as_deref(), project, project_id) {
      continue;
    }
    let path = secrets::COLLECTION.replace("{environment_id}", id);
    let keys = api.request(Method::GET, &path, None).await?;
    let entry = projects
      .entry(project_id.to_owned())
      .or_insert_with(|| (project.to_owned(), Vec::new()));
    for mut secret in keys.as_array().cloned().unwrap_or_default() {
      secret["group"] = group.into();
      entry.1.push(secret);
    }
  }
  let mut listing: Vec<Value> = projects
    .into_iter()
    .map(|(id, (name, mut secrets))| {
      secrets.sort_by(|a, b| text(a, "key").cmp(text(b, "key")));
      json!({"project": name, "id": id, "secrets": secrets})
    })
    .collect();
  listing.sort_by(|a, b| text(a, "project").cmp(text(b, "project")));
  if let Some(target) = args.target.as_deref().filter(|_| listing.is_empty()) {
    anyhow::bail!("project {target:?} is not visible to this credential");
  }

  if json_output {
    println!("{}", serde_json::to_string_pretty(&listing)?);
    return Ok(0);
  }
  for project in &listing {
    let secrets = project["secrets"].as_array().cloned().unwrap_or_default();
    println!(
      "{} ({}) - {} secret(s)",
      text(project, "project"),
      text(project, "id"),
      secrets.len()
    );
    for s in secrets {
      let group = format!("[{}]", text(&s, "group"));
      match s.get("version") {
        Some(version) => println!(
          "    {:<30} {:<12} v{:<3} updated {}",
          text(&s, "key"),
          group,
          version,
          text(&s, "updatedAt")
            .get(..16)
            .unwrap_or("")
            .replace('T', " ")
        ),
        None => println!("    {:<30} {}", text(&s, "key"), group),
      }
      for line in description_lines(text(&s, "description")) {
        println!("        {line}");
      }
    }
  }
  Ok(0)
}

fn text<'a>(
  value: &'a Value,
  field: &str,
) -> &'a str {
  value.get(field).and_then(Value::as_str).unwrap_or("")
}

/// A secret's usage description, one entry per non-empty line. Other project members
/// write it, so control characters (terminal escapes) are dropped before printing.
fn description_lines(description: &str) -> Vec<String> {
  description
    .lines()
    .map(|line| line.chars().filter(|c| !c.is_control()).collect::<String>())
    .map(|line| line.trim().to_owned())
    .filter(|line| !line.is_empty())
    .collect()
}

/// `None` matches all; otherwise a project name or id.
pub fn matches_target(
  target: Option<&str>,
  project: &str,
  project_id: &str,
) -> bool {
  target.is_none_or(|t| t == project || t == project_id)
}

/// Projects visible to the credential as (id, name). Humans and personal tokens list
/// `/projects` (so a project without groups still shows); runner tokens cannot, and fall
/// back to the environment list every credential type may read.
pub(crate) async fn visible_projects(api: &client::ApiClient) -> Result<Vec<(String, String)>> {
  if let Ok(value) = api.request(Method::GET, projects::COLLECTION, None).await {
    let mut listed: Vec<(String, String)> = value
      .as_array()
      .into_iter()
      .flatten()
      .map(|project| {
        (
          text(project, "id").to_owned(),
          text(project, "name").to_owned(),
        )
      })
      .filter(|(id, _)| !id.is_empty())
      .collect();
    listed.sort_by(|a, b| a.1.cmp(&b.1));
    return Ok(listed);
  }
  let mut projects: Vec<(String, String)> = api
    .request(Method::GET, environments::COLLECTION, None)
    .await?
    .as_array()
    .into_iter()
    .flatten()
    .map(|env| {
      (
        text(env, "projectId").to_owned(),
        text(env, "projectName").to_owned(),
      )
    })
    .collect();
  projects.sort_by(|a, b| a.1.cmp(&b.1));
  projects.dedup();
  Ok(projects)
}

/// `onekey use PROJECT` / `onekey use --clear`.
pub(super) async fn use_project(
  server: &local_config::ResolvedServer,
  project: Option<String>,
  clear: bool,
  json_output: bool,
) -> Result<i32> {
  if clear {
    let cleared = local_config::clear_default_environment(server)?;
    if json_output {
      println!("{}", json!({"cleared": cleared}));
    } else {
      println!(
        "{}",
        if cleared {
          "Default project cleared."
        } else {
          "No default project was set."
        }
      );
    }
    return Ok(0);
  }
  let reference = project.unwrap_or_default();
  let credential = client::credential(server)?;
  let api = client::authenticated_client(server, credential)?;
  let projects = visible_projects(&api).await?;
  let Some((id, name)) = projects
    .iter()
    .find(|(id, name)| *id == reference || *name == reference)
    .cloned()
  else {
    let names: Vec<&str> = projects.iter().map(|(_, name)| name.as_str()).collect();
    anyhow::bail!(
      "project {reference:?} is not visible to this credential. Visible: {}",
      names.join(", ")
    );
  };
  local_config::save_default_environment(server, &id)?;
  if json_output {
    println!("{}", json!({"project": name, "id": id}));
  } else {
    println!("Default project: {name} ({id})");
  }
  Ok(0)
}

#[cfg(test)]
mod tests {
  use super::description_lines;

  #[test]
  fn description_lines_drop_escapes_and_blank_lines() {
    assert!(description_lines("").is_empty());
    assert_eq!(
      description_lines("Billing API bearer token\n\n  read only \u{1b}[31m\r\n"),
      ["Billing API bearer token", "read only [31m"]
    );
  }
}
