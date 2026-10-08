use crate::{
  cli::{
    client::{self, ApiClient},
    commands::cache::store::{self, CachedRuntime},
    local_config::ResolvedServer,
  },
  constants::{
    api,
    limits::{MAX_SECRET_COLLECTION_BYTES, MAX_SECRETS_PER_ENVIRONMENT},
  },
  models::SecretInput,
};
use anyhow::{Context, Result, bail};
use chrono::{DateTime, SecondsFormat, Utc};
use reqwest::Method;
use serde::Deserialize;
use std::{collections::HashSet, time::Duration};

const LIVE_TIMEOUT: Duration = Duration::from_secs(5);

fn validate_cache_age(
  fetched_at: DateTime<Utc>,
  ttl_seconds: Option<u64>,
  now: DateTime<Utc>,
) -> Result<()> {
  let ttl = ttl_seconds.context("offline cache has no server-issued TTL; fetch secrets from an updated server before using them offline")?;
  if ttl == 0 {
    bail!("offline cache fallback is disabled by the server");
  }
  if ttl > 86_400 {
    bail!("offline cache contains an invalid server-issued TTL");
  }
  let age = now.signed_duration_since(fetched_at).to_std().context(
    "offline cache fetch time is in the future; refresh it while the server is available",
  )?;
  if age >= Duration::from_secs(ttl) {
    bail!(
      "offline cache has expired (age {}, TTL {}); refresh it while the server is available",
      format_age(now.signed_duration_since(fetched_at)),
      format_age(chrono::Duration::seconds(ttl as i64))
    );
  }
  Ok(())
}

#[derive(Debug)]
pub enum RuntimeSource {
  Live {
    cache_warning: Option<String>,
  },
  Cache {
    fetched_at: String,
    age: String,
    reason: String,
  },
}

impl RuntimeSource {
  pub fn as_str(&self) -> &'static str {
    match self {
      Self::Live { .. } => "live",
      Self::Cache { .. } => "cache",
    }
  }
}

#[derive(Debug)]
pub struct RuntimeLoad {
  pub project: String,
  pub environment: String,
  pub environment_id: String,
  pub entries: Vec<SecretInput>,
  pub source: RuntimeSource,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeResponse {
  project: String,
  environment: String,
  environment_id: String,
  entries: Vec<SecretInput>,
  cache_ttl_seconds: Option<u64>,
}

pub async fn load(
  server: &ResolvedServer,
  api: &ApiClient,
  reference: &str,
) -> Result<RuntimeLoad> {
  let live = tokio::time::timeout(LIVE_TIMEOUT, fetch_live(api, reference)).await;
  match live {
    Ok(Ok(runtime)) => {
      let cached = CachedRuntime {
        project: runtime.project.clone(),
        environment: runtime.environment.clone(),
        environment_id: runtime.environment_id.clone(),
        aliases: Vec::new(),
        fetched_at: Utc::now(),
        cache_ttl_seconds: runtime.cache_ttl_seconds,
        entries: runtime.entries.clone(),
      };
      let credential = api.credential_token()?;
      // No usable offline lifetime: keep no copy on disk rather than one that is always refused.
      let cache_warning = if matches!(cached.cache_ttl_seconds, Some(ttl) if ttl > 0) {
        store::save(server, credential, reference, &cached)
          .err()
          .map(|error| {
            format!("live secrets were loaded, but the encrypted cache was not updated: {error}")
          })
      } else {
        store::forget(server, credential, &cached.environment_id)
          .err()
          .map(|error| {
            format!(
              "live secrets were loaded, but the old encrypted cache was not removed: {error}"
            )
          })
      };
      Ok(RuntimeLoad {
        project: runtime.project,
        environment: runtime.environment,
        environment_id: runtime.environment_id,
        entries: runtime.entries,
        source: RuntimeSource::Live { cache_warning },
      })
    }
    Ok(Err(error)) if client::is_availability_error(&error) => {
      load_after_failure(server, api, reference, concise_reason(&error))
    }
    Err(_) => load_after_failure(
      server,
      api,
      reference,
      format!(
        "OneKey at {} did not complete the runtime fetch within 5 seconds",
        server.url
      ),
    ),
    Ok(Err(error)) => Err(error),
  }
}

async fn fetch_live(
  api: &ApiClient,
  reference: &str,
) -> Result<RuntimeResponse> {
  if reference.contains('/') {
    bail!(
      "{reference:?}: pass the project name. `onekey run <project>` injects every secret of the project across its groups"
    );
  }
  let value = api
    .request_runtime(Method::GET, &api::projects::runtime(reference), None)
    .await?;
  let runtime: ProjectRuntimeResponse =
    serde_json::from_value(value).context("runtime response was invalid")?;
  if runtime.project.is_empty() || runtime.project_id.is_empty() {
    bail!("runtime response contained empty project metadata");
  }
  validate_entries(&runtime.entries)?;
  Ok(RuntimeResponse {
    project: runtime.project,
    environment: runtime.groups.join(","),
    environment_id: runtime.project_id,
    entries: runtime.entries,
    cache_ttl_seconds: runtime.cache_ttl_seconds,
  })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectRuntimeResponse {
  project: String,
  project_id: String,
  groups: Vec<String>,
  entries: Vec<SecretInput>,
  #[serde(default)]
  cache_ttl_seconds: Option<u64>,
}

fn validate_entries(entries: &[SecretInput]) -> Result<()> {
  if entries.len() > MAX_SECRETS_PER_ENVIRONMENT {
    bail!("runtime response exceeded the maximum number of environment variables");
  }
  let total_bytes = entries.iter().try_fold(0_usize, |total, entry| {
    total
      .checked_add(entry.key.len())
      .and_then(|total| total.checked_add(entry.value.len()))
      .context("runtime response size overflowed")
  })?;
  if total_bytes > MAX_SECRET_COLLECTION_BYTES {
    bail!("runtime response exceeded the maximum secret collection size");
  }
  let mut keys = HashSet::with_capacity(entries.len());
  for entry in entries {
    if entry.key.is_empty() || entry.key.contains('=') || entry.key.contains('\0') {
      bail!("runtime response contained an invalid environment variable name");
    }
    if entry.value.contains('\0') {
      bail!("runtime response contained an invalid environment variable value");
    }
    if !keys.insert(&entry.key) {
      bail!("runtime response contained a duplicate environment variable name");
    }
  }
  Ok(())
}

fn load_after_failure(
  server: &ResolvedServer,
  api: &ApiClient,
  reference: &str,
  reason: String,
) -> Result<RuntimeLoad> {
  let cached = store::load(server, api.credential_token()?, reference).with_context(|| {
    format!(
      "{reason}. \nEnvironment variables were not injected, and the child was not started because no usable encrypted cache is available for {reference}"
    )
  })?;
  let now = Utc::now();
  validate_cache_age(cached.fetched_at, cached.cache_ttl_seconds, now).with_context(|| {
    format!("{reason}. Environment variables were not injected, and the child was not started for {reference}")
  })?;
  validate_entries(&cached.entries)?;
  let fetched_at = cached.fetched_at.to_rfc3339_opts(SecondsFormat::Secs, true);
  let age = format_age(now.signed_duration_since(cached.fetched_at));
  Ok(RuntimeLoad {
    project: cached.project,
    environment: cached.environment,
    environment_id: cached.environment_id,
    entries: cached.entries,
    source: RuntimeSource::Cache {
      fetched_at,
      age,
      reason,
    },
  })
}

fn concise_reason(error: &anyhow::Error) -> String {
  error
    .to_string()
    .lines()
    .next()
    .unwrap_or("OneKey is unavailable")
    .to_owned()
}

fn format_age(age: chrono::Duration) -> String {
  let seconds = age.num_seconds().max(0);
  if seconds < 60 {
    format!("{seconds}s")
  } else if seconds < 60 * 60 {
    format!("{}m", seconds / 60)
  } else if seconds < 24 * 60 * 60 {
    format!("{}h", seconds / (60 * 60))
  } else {
    format!("{}d", seconds / (24 * 60 * 60))
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn cache_expires_at_the_boundary_and_reads_do_not_extend_its_lifetime() {
    let fetched = DateTime::from_timestamp(1_700_000_000, 0).unwrap();
    for ttl in [900_u64, 3600] {
      for (age, allowed) in [(ttl - 1, true), (ttl, false), (ttl + 1, false)] {
        assert_eq!(
          validate_cache_age(
            fetched,
            Some(ttl),
            fetched + chrono::Duration::seconds(age as i64)
          )
          .is_ok(),
          allowed
        );
      }
    }
  }

  #[test]
  fn missing_disabled_invalid_policies_and_clock_rollback_fail_closed() {
    let fetched = Utc::now();
    for ttl in [None, Some(0), Some(86_401), Some(u64::MAX)] {
      assert!(validate_cache_age(fetched, ttl, fetched).is_err());
    }
    assert!(
      validate_cache_age(fetched, Some(3600), fetched - chrono::Duration::seconds(1)).is_err()
    );
  }
}
