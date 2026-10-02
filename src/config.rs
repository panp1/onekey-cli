//! Client configuration paths: the OneKey home, named profiles and the data directory.
use std::{
  env, fs,
  path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use url::Url;

use crate::constants::config::{
  ACTIVE_PROFILE_FILENAME, CLIENT_CONFIG_FILENAME, DATA_DIRECTORY_NAME, DEFAULT_PROFILE,
  ENV_DATA_DIR, ENV_PROFILE, PROFILES_DIRECTORY_NAME,
};

pub fn validate_endpoint_transport(url: &Url) -> Result<()> {
  if !matches!(url.scheme(), "http" | "https") {
    bail!("URL must use HTTP or HTTPS");
  }
  Ok(())
}

pub fn resolve_data_dir(argument: Option<&Path>) -> Result<PathBuf> {
  let environment = env::var_os(ENV_DATA_DIR).map(PathBuf::from);
  let fallback = onekey_home();
  resolve_path(argument.or(environment.as_deref()).unwrap_or(&fallback))
}

pub fn client_config_path(data_dir: Option<&Path>) -> Result<PathBuf> {
  Ok(client_data_dir(data_dir)?.join(CLIENT_CONFIG_FILENAME))
}

/// Client state directory: `--data-dir` / `ONEKEY_DATA_DIR` win; otherwise the active
/// profile's directory (`ONEKEY_PROFILE`, else the name saved in `~/.onekey/profile`).
/// The `default` profile is the OneKey home itself, so pre-profile setups keep working.
pub fn client_data_dir(data_dir: Option<&Path>) -> Result<PathBuf> {
  if data_dir.is_some() || env::var_os(ENV_DATA_DIR).is_some() {
    return resolve_data_dir(data_dir);
  }
  let home = onekey_home();
  let profile = active_profile(&home)?;
  let dir = profile_dir(&home, &profile)?;
  if profile != DEFAULT_PROFILE && !dir.is_dir() {
    bail!(
      "profile '{profile}' does not exist. Run `onekey config --profile {profile}` or `onekey profile use default`"
    );
  }
  Ok(dir)
}

/// `ONEKEY_PROFILE`, else the saved active profile, else `default`.
pub fn active_profile(home: &Path) -> Result<String> {
  let name = match env::var(ENV_PROFILE) {
    Ok(value) if !value.trim().is_empty() => value.trim().to_owned(),
    _ => saved_active_profile(home).unwrap_or_default(),
  };
  if name.is_empty() {
    return Ok(DEFAULT_PROFILE.into());
  }
  validate_profile_name(&name)?;
  Ok(name)
}

/// The profile name written by `onekey profile use` / `onekey config`, ignoring `ONEKEY_PROFILE`.
pub fn saved_active_profile(home: &Path) -> Option<String> {
  fs::read_to_string(home.join(ACTIVE_PROFILE_FILENAME))
    .ok()
    .map(|value| value.trim().to_owned())
    .filter(|value| !value.is_empty())
}

pub fn validate_profile_name(name: &str) -> Result<()> {
  if name.is_empty()
    || name.len() > 32
    || !name
      .chars()
      .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
  {
    bail!("invalid profile name {name:?}: use 1-32 lowercase letters, digits, '-' or '_'");
  }
  Ok(())
}

pub fn profile_dir(
  home: &Path,
  name: &str,
) -> Result<PathBuf> {
  validate_profile_name(name)?;
  Ok(if name == DEFAULT_PROFILE {
    home.to_path_buf()
  } else {
    home.join(PROFILES_DIRECTORY_NAME).join(name)
  })
}

/// `default` plus every directory under `~/.onekey/profiles`, sorted.
pub fn list_profiles(home: &Path) -> Vec<String> {
  let mut names = vec![DEFAULT_PROFILE.to_owned()];
  if let Ok(entries) = fs::read_dir(home.join(PROFILES_DIRECTORY_NAME)) {
    names.extend(
      entries
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| validate_profile_name(name).is_ok() && name != DEFAULT_PROFILE),
    );
  }
  names.sort();
  names
}

pub fn set_active_profile(
  home: &Path,
  name: &str,
) -> Result<()> {
  validate_profile_name(name)?;
  ensure_data_dir(home)?;
  crate::utils::private_file::write(
    &home.join(ACTIVE_PROFILE_FILENAME),
    format!("{name}\n").as_bytes(),
    true,
  )
}

pub fn onekey_home() -> PathBuf {
  env::home_dir()
    .map(|home| home.join(DATA_DIRECTORY_NAME))
    .unwrap_or_else(|| PathBuf::from(DATA_DIRECTORY_NAME))
}

pub fn ensure_data_dir(path: &Path) -> Result<()> {
  let existed = path.exists();
  fs::create_dir_all(path)
    .with_context(|| format!("failed to create data directory {}", path.display()))?;
  #[cfg(unix)]
  if !existed {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
      .with_context(|| format!("failed to secure data directory {}", path.display()))?;
  }
  Ok(())
}

fn resolve_path(path: &Path) -> Result<PathBuf> {
  let expanded = expand_home(path);
  if expanded.is_absolute() {
    Ok(expanded)
  } else {
    Ok(
      env::current_dir()
        .context("failed to resolve current directory")?
        .join(expanded),
    )
  }
}

fn expand_home(path: &Path) -> PathBuf {
  let value = path.to_string_lossy();
  let Some(home) = env::home_dir() else {
    return path.to_path_buf();
  };
  if value == "~" {
    return home;
  }
  if let Some(rest) = value.strip_prefix("~/") {
    return home.join(rest);
  }
  path.to_path_buf()
}
