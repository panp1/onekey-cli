use onekey_cli::cli::{
  client::{Credential, CredentialSource, credential_from_sources, validate_runner_token},
  commands::{run_environment, server_switch_confirmed, status_document},
  local_config::{ClientConfig, DefaultEnvironment, ResolvedServer, ServerSource},
};

mod cache;
mod init;

#[cfg(unix)]
#[test]
fn insecure_transport_warning_names_the_server_and_the_risk() {
  assert_eq!(
    onekey_cli::cli::commands::insecure_transport_warning("http://192.168.1.20:8840"),
    "Plain HTTP does not encrypt traffic to http://192.168.1.20:8840. Credentials and secrets could be exposed. Use HTTPS whenever possible."
  );
}
use std::env::VarError;
use tempfile::TempDir;

fn server(directory: &TempDir) -> ResolvedServer {
  ResolvedServer {
    url: "https://onekey.example.com".into(),
    source: ServerSource::Config,
    config_path: directory.path().join("config.toml"),
    config: ClientConfig {
      version: 1,
      server_url: Some("https://onekey.example.com".into()),
      default_environment: Some(DefaultEnvironment {
        server_url: "https://onekey.example.com".into(),
        environment_id: "env_default".into(),
      }),
    },
  }
}

#[test]
fn run_environment_uses_explicit_then_variable_then_saved_default() {
  assert_eq!(
    run_environment(
      Some("env_explicit".into()),
      Ok("env_variable".into()),
      Some("env_default")
    )
    .unwrap()
    .reference,
    "env_explicit"
  );
  assert_eq!(
    run_environment(None, Ok("env_variable".into()), Some("env_default"))
      .unwrap()
      .reference,
    "env_variable"
  );
  assert_eq!(
    run_environment(None, Err(VarError::NotPresent), Some("env_default"))
      .unwrap()
      .reference,
    "env_default"
  );
}

#[test]
fn run_environment_rejects_empty_variable_and_explains_how_to_set_a_default() {
  assert_eq!(
    run_environment(None, Ok(String::new()), Some("env_default"))
      .err()
      .unwrap()
      .to_string(),
    "ONEKEY_ENV is set but empty"
  );
  let message = run_environment(None, Err(VarError::NotPresent), None)
    .err()
    .unwrap()
    .to_string();
  assert!(message.contains("No default project is set."), "{message}");
  assert!(message.contains("onekey use <project>"), "{message}");
}

#[test]
fn run_environment_rejects_a_non_unicode_variable() {
  let invalid = std::ffi::OsString::from("invalid");
  assert_eq!(
    run_environment(
      None,
      Err(VarError::NotUnicode(invalid)),
      Some("env_default")
    )
    .err()
    .unwrap()
    .to_string(),
    "ONEKEY_ENV contains invalid Unicode"
  );
}

#[test]
fn server_switch_requires_an_explicit_yes_answer() {
  for accepted in ["y", "Y", "yes", "YES", " yes "] {
    assert!(server_switch_confirmed(accepted), "{accepted:?}");
  }
  for rejected in ["", "n", "no", "true", "1", "switch"] {
    assert!(!server_switch_confirmed(rejected), "{rejected:?}");
  }
}

#[test]
fn status_includes_cached_admin_email_and_default_environment() {
  let directory = TempDir::new().unwrap();
  let server = server(&directory);
  let credential = Credential {
    token: Some("dbc_secret-token".into()),
    source: CredentialSource::EncryptedSession,
    email: Some("admin@example.com".into()),
  };

  let value = status_document(&server, &credential, true);
  assert_eq!(value["authentication"], "encrypted_session");
  assert_eq!(value["identity"], "admin");
  assert_eq!(value["email"], "admin@example.com");
  assert_eq!(value["environment"], "env_default");
  assert_eq!(value["server_status"], "connected");
  assert_eq!(value["status_source"], "live");
  assert!(!value.to_string().contains("dbc_secret-token"));
}

#[test]
fn status_identifies_an_encrypted_runner_token() {
  let directory = TempDir::new().unwrap();
  let server = server(&directory);
  let token = "dbs_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
  let credential = Credential {
    token: Some(token.into()),
    source: CredentialSource::EncryptedSession,
    email: None,
  };

  let value = status_document(&server, &credential, true);
  assert_eq!(value["authentication"], "encrypted_session");
  assert_eq!(value["identity"], "runner");
  assert!(value["email"].is_null());
  assert!(!value.to_string().contains(token));
}

#[test]
fn explicit_runner_token_has_priority_over_environment_and_saved_credentials() {
  let explicit = "dbs_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
  let credential = credential_from_sources(
    Some(explicit.into()),
    Ok("environment-token".into()),
    || panic!("saved credentials must not be loaded for an explicit token"),
  )
  .unwrap();

  assert_eq!(credential.token.as_deref(), Some(explicit));
  assert!(matches!(credential.source, CredentialSource::Argument));
}

#[test]
fn environment_token_has_priority_over_the_saved_credential() {
  let environment = "dbs_BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB";
  let credential = credential_from_sources(None, Ok(environment.into()), || {
    panic!("saved credentials must not be loaded when ONEKEY_TOKEN is set")
  })
  .unwrap();

  assert_eq!(credential.token.as_deref(), Some(environment));
  assert!(matches!(credential.source, CredentialSource::Environment));
}

#[test]
fn runner_token_validation_rejects_empty_wrong_prefix_and_wrong_length() {
  assert!(validate_runner_token("dbs_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA").is_ok());
  for token in [
    "",
    "dbc_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
    "dbs_too-short",
    "dbs_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA!",
  ] {
    assert!(validate_runner_token(token).is_err(), "accepted {token:?}");
  }
}

#[test]
fn status_identifies_environment_token_types_without_exposing_tokens() {
  let directory = TempDir::new().unwrap();
  let server = server(&directory);
  for (token, identity) in [
    ("dbc_admin-secret", "human"),
    ("dbs_runner-secret", "runner"),
    ("dpa_agent-secret", "ai_agent"),
    ("other-secret", "unknown"),
  ] {
    let credential = Credential {
      token: Some(token.into()),
      source: CredentialSource::Environment,
      email: None,
    };

    let value = status_document(&server, &credential, true);
    assert_eq!(value["identity"], identity);
    assert!(value["email"].is_null());
    assert!(!value.to_string().contains(token));
  }
}

#[test]
fn status_reports_no_identity_without_credentials() {
  let directory = TempDir::new().unwrap();
  let mut server = server(&directory);
  server.config.default_environment = None;
  let credential = Credential {
    token: None,
    source: CredentialSource::None,
    email: None,
  };

  let value = status_document(&server, &credential, false);
  assert_eq!(value["identity"], "none");
  assert!(value["email"].is_null());
  assert!(value["environment"].is_null());
  assert_eq!(value["server_status"], "offline");
  assert_eq!(value["status_source"], "cache");
}

#[tokio::test]
async fn backup_rejects_when_server_is_offline() {
  use clap::Parser;

  let cli = onekey_cli::cli::args::Cli::try_parse_from([
    "onekey",
    "--server",
    "http://127.0.0.1:1",
    "backup",
  ])
  .unwrap();

  let err = onekey_cli::cli::commands::execute(cli).await.unwrap_err();
  let msg = err.to_string();
  assert!(
    msg.contains("Cannot perform backup: OneKey server at http://127.0.0.1:1 is not connected or offline (live status required)"),
    "unexpected message: {msg}"
  );
}

#[tokio::test]
async fn restore_rejects_when_server_is_offline() {
  use clap::Parser;

  let dir = TempDir::new().unwrap();
  let backup_file = dir.path().join("test_backup.dop");
  std::fs::write(&backup_file, b"dummy content").unwrap();

  let cli = onekey_cli::cli::args::Cli::try_parse_from([
    "onekey",
    "--server",
    "http://127.0.0.1:1",
    "restore",
    backup_file.to_str().unwrap(),
    "--yes",
  ])
  .unwrap();

  let err = onekey_cli::cli::commands::execute(cli).await.unwrap_err();
  let msg = err.to_string();
  assert!(
    msg.contains("Cannot perform restore: OneKey server at http://127.0.0.1:1 is not connected or offline (live status required)"),
    "unexpected message: {msg}"
  );
}

mod environment;
mod import;
mod run;
mod shortcuts;
mod update;

#[test]
fn personal_tokens_pass_token_validation() {
  assert!(validate_runner_token("dpa_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA").is_ok());
  assert!(validate_runner_token("dbc_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA").is_err());
}
