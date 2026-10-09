use clap::{CommandFactory, Parser, error::ErrorKind};
use onekey_cli::cli::args::{Cli, Command, ExportArgs, ImportArgs, InitArgs, RunArgs};
use onekey_cli::cli::{
  local_config::{ClientConfig, ResolvedServer, ServerSource},
  secret_format::{ExportFormat, SecretFormat},
  session,
};
use onekey_cli::constants::config::executable_environment_names;
use std::{
  io::Write,
  process::{Command as ProcessCommand, Stdio},
};

fn contextual_help(arguments: &[&str]) -> String {
  let error = Cli::try_parse_with_help_from(arguments).unwrap_err();
  assert!(
    matches!(
      error.kind(),
      ErrorKind::DisplayHelp | ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
    ),
    "unexpected error for {arguments:?}: {error}"
  );
  error.to_string()
}

#[test]
fn parses_every_v0_1_command_shape() {
  let commands: &[&[&str]] = &[
    &["onekey", "client", "connect", "http://localhost:8840"],
    &["onekey", "login"],
    &["onekey", "login", "--token"],
    &["onekey", "logout"],
    &["onekey", "status"],
    &["onekey", "client", "status"],
    &["onekey", "init", "billing/production", "--from", ".env"],
    &["onekey", "project", "create", "billing"],
    &["onekey", "project", "list"],
    &["onekey", "project", "show", "billing"],
    &["onekey", "project", "rename", "billing", "payments"],
    &["onekey", "project", "delete", "billing", "--yes"],
    &["onekey", "group", "create", "billing/production"],
    &["onekey", "group", "list", "billing"],
    &["onekey", "group", "show", "billing/production"],
    &["onekey", "group", "rename", "env_01", "staging"],
    &["onekey", "group", "delete", "env_01", "--yes"],
    &["onekey", "secret", "list", "billing/production"],
    &[
      "onekey",
      "secret",
      "set",
      "billing/production",
      "API_KEY",
      "--stdin",
    ],
    &[
      "onekey",
      "secret",
      "get",
      "billing/production",
      "API_KEY",
      "--reveal",
    ],
    &[
      "onekey",
      "secret",
      "delete",
      "billing/production",
      "API_KEY",
      "--yes",
    ],
    &[
      "onekey",
      "import",
      "billing/production",
      ".env",
      "--dry-run",
    ],
    &[
      "onekey",
      "import",
      "billing/production",
      "-",
      "--format",
      "json",
    ],
    &["onekey", "export", "billing/production", "--stdout"],
    &[
      "onekey",
      "export",
      "billing/production",
      "--stdout",
      "--format",
      "yaml",
    ],
    &["onekey", "token", "list", "billing/production"],
    &["onekey", "token", "revoke", "tok_01"],
    &["onekey", "run", "billing/production", "--", "printenv"],
    &["onekey", "run", "env_482731", "--", "printenv"],
    &[
      "onekey",
      "run",
      "env_482731",
      "--token",
      "dbs_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
      "--",
      "printenv",
    ],
    &["onekey", "cache", "list"],
    &["onekey", "cache", "clean"],
    &["onekey", "cache", "clean", "--older-than", "30d"],
    &["onekey", "cache", "clean", "--dry-run", "--all"],
    &["onekey", "cache", "clean", "--all", "--yes"],
    &[
      "onekey",
      "run",
      "env_482731",
      "-t",
      "dbs_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
      "--",
      "printenv",
    ],
    &["onekey", "update"],
    &["onekey", "backup"],
    &["onekey", "backup", "my-backup"],
    &["onekey", "backup", "--output", "/tmp/backup.dop"],
    &[
      "onekey",
      "backup",
      "my-backup",
      "--output",
      "/tmp/backup.dop",
    ],
    &["onekey", "restore", "/tmp/backup.dop"],
    &["onekey", "restore", "/tmp/backup.dop", "--yes"],
    &["onekey", "--json", "project", "list"],
    &["onekey", "project", "list", "--json"],
    &["onekey", "--data-dir", "/tmp/onekey", "status"],
  ];

  for command in commands {
    Cli::try_parse_from(*command)
      .unwrap_or_else(|error| panic!("failed to parse {command:?}: {error}"));
  }
}

#[test]
fn run_token_must_appear_before_the_child_command_separator() {
  let cli = Cli::try_parse_from([
    "onekey",
    "run",
    "env_482731",
    "--token",
    "dbs_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
    "--",
    "printenv",
    "--token",
    "child-value",
  ])
  .unwrap();
  let Command::Run(RunArgs { token, command, .. }) = cli.command else {
    panic!("expected run command");
  };
  assert_eq!(
    token.as_deref(),
    Some("dbs_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA")
  );
  assert_eq!(command, ["printenv", "--token", "child-value"]);
}

#[test]
fn run_accepts_the_short_token_flag() {
  let cli = Cli::try_parse_from([
    "onekey",
    "run",
    "env_482731",
    "-t",
    "dbs_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
    "--",
    "printenv",
    "-t",
    "child-value",
  ])
  .unwrap();
  let Command::Run(RunArgs { token, command, .. }) = cli.command else {
    panic!("expected run command");
  };
  assert_eq!(
    token.as_deref(),
    Some("dbs_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA")
  );
  assert_eq!(command, ["printenv", "-t", "child-value"]);
}

#[test]
fn run_accepts_yaml_template_before_the_child_separator() {
  for child in [
    vec!["kubectl", "apply", "-f", "-"],
    vec![
      "helm",
      "upgrade",
      "--install",
      "myapp",
      "./chart",
      "-f",
      "-",
    ],
  ] {
    let mut args = vec!["onekey", "run", "billing", "--template", "input.yaml", "--"];
    args.extend(child.iter().copied());
    let cli = Cli::try_parse_from(args).unwrap();
    let Command::Run(RunArgs {
      template, command, ..
    }) = cli.command
    else {
      panic!("expected run command");
    };
    assert_eq!(template.unwrap(), std::path::Path::new("input.yaml"));
    assert_eq!(command, child);
  }
}

#[test]
fn login_does_not_accept_the_short_token_flag() {
  assert!(Cli::try_parse_from(["onekey", "login", "-t"]).is_err());
}

#[test]
fn version_flags_name_the_cli_edition() {
  let expected = format!("OneKey CLI v{}\n", env!("CARGO_PKG_VERSION"));
  for flag in ["-v", "-V", "--version"] {
    let output = ProcessCommand::new(env!("CARGO_BIN_EXE_onekey"))
      .arg(flag)
      .output()
      .unwrap();
    assert!(output.status.success(), "{flag}: {output:?}");
    assert_eq!(output.stdout, expected.as_bytes(), "{flag}");
    assert!(output.stderr.is_empty(), "{flag}: {output:?}");
  }
}

#[test]
fn global_output_flag_works_before_or_after_the_command() {
  let before = Cli::try_parse_from(["onekey", "--json", "group", "list", "billing"]).unwrap();
  let after = Cli::try_parse_from(["onekey", "group", "list", "billing", "--json"]).unwrap();
  assert!(before.json);
  assert!(after.json);
}

#[test]
fn status_and_config_are_distinct_commands() {
  let cli = Cli::try_parse_from(["onekey", "status"]).unwrap();
  assert!(matches!(cli.command, Command::Status));
  // `config` is the OneKey one-step setup wizard (it replaced nothing upstream still uses).
  let cli = Cli::try_parse_from(["onekey", "config"]).unwrap();
  assert!(matches!(cli.command, Command::Config(_)));
}

#[test]
fn rejects_conflicting_file_options() {
  assert!(
    Cli::try_parse_from([
      "onekey",
      "import",
      "billing/production",
      ".env",
      "--dry-run",
      "--replace",
    ])
    .is_err()
  );
  assert!(
    Cli::try_parse_from([
      "onekey",
      "export",
      "billing/production",
      "--output",
      "secrets.env",
      "--stdout",
    ])
    .is_err()
  );
}

#[test]
fn secret_commands_parse_format_options() {
  let interactive_init = Cli::try_parse_from(["onekey", "init"]).unwrap();
  assert!(matches!(
    interactive_init.command,
    Command::Init(InitArgs {
      target: None,
      from: None,
      format: None,
    })
  ));

  let init = Cli::try_parse_from([
    "onekey",
    "init",
    "storefront/development",
    "--from",
    "-",
    "--format",
    "yaml",
  ])
  .unwrap();
  assert!(matches!(
    init.command,
    Command::Init(InitArgs {
      format: Some(SecretFormat::Yaml),
      ..
    })
  ));

  let import = Cli::try_parse_from([
    "onekey",
    "import",
    "storefront/development",
    "secrets.data",
    "--format",
    "json",
  ])
  .unwrap();
  assert!(matches!(
    import.command,
    Command::Import(ImportArgs {
      format: Some(SecretFormat::Json),
      ..
    })
  ));

  let toml_init = Cli::try_parse_from([
    "onekey",
    "init",
    "storefront/development",
    "--from",
    "secrets.toml",
    "--format",
    "toml",
  ])
  .unwrap();
  assert!(matches!(
    toml_init.command,
    Command::Init(InitArgs {
      format: Some(SecretFormat::Toml),
      ..
    })
  ));

  let toml_import = Cli::try_parse_from([
    "onekey",
    "import",
    "storefront/development",
    "secrets.toml",
    "--format",
    "toml",
  ])
  .unwrap();
  assert!(matches!(
    toml_import.command,
    Command::Import(ImportArgs {
      format: Some(SecretFormat::Toml),
      ..
    })
  ));

  let export = Cli::try_parse_from([
    "onekey",
    "export",
    "storefront/development",
    "--output",
    "secrets.yml",
    "--format",
    "dotenv",
  ])
  .unwrap();
  assert!(matches!(
    export.command,
    Command::Export(ExportArgs {
      format: Some(ExportFormat::Dotenv),
      ..
    })
  ));

  let toml_export = Cli::try_parse_from([
    "onekey",
    "export",
    "storefront/development",
    "--output",
    "secrets.toml",
    "--format",
    "toml",
  ])
  .unwrap();
  assert!(matches!(
    toml_export.command,
    Command::Export(ExportArgs {
      format: Some(ExportFormat::Toml),
      ..
    })
  ));

  let docker_export = Cli::try_parse_from([
    "onekey",
    "export",
    "storefront/development",
    "--stdout",
    "--format",
    "docker",
  ])
  .unwrap();
  assert!(matches!(
    docker_export.command,
    Command::Export(ExportArgs {
      format: Some(ExportFormat::Docker),
      ..
    })
  ));

  for command in ["init", "import"] {
    let mut args = vec!["onekey", command, "storefront/development"];
    if command == "init" {
      args.extend(["--from", ".env"]);
    } else {
      args.push(".env");
    }
    args.extend(["--format", "docker"]);
    assert!(Cli::try_parse_from(args).is_err());
  }

  assert!(
    Cli::try_parse_from([
      "onekey",
      "init",
      "storefront",
      "development",
      "--from",
      ".env",
    ])
    .is_err()
  );
  assert!(Cli::try_parse_from(["onekey", "group", "create", "storefront", "development"]).is_err());

  for arguments in [
    vec!["onekey", "init", "storefront/development"],
    vec!["onekey", "init", "--from", ".env"],
    vec!["onekey", "init", "--format", "dotenv"],
  ] {
    assert!(Cli::try_parse_from(arguments).is_err());
  }
}

#[test]
fn validates_qualified_environment_creation_targets() {
  for target in ["storefront/development", "prj_01JTEST/development"] {
    Cli::try_parse_from(["onekey", "group", "create", target]).unwrap();
  }
  // Names are lowercased; project IDs keep their case.
  let parsed =
    Cli::try_parse_from(["onekey", "group", "create", "StoreFront/Development"]).unwrap();
  assert!(format!("{parsed:?}").contains(r#"project: "storefront", environment: "development""#));
  let parsed =
    Cli::try_parse_from(["onekey", "group", "create", "prj_01JTEST/Development"]).unwrap();
  assert!(format!("{parsed:?}").contains(r#"project: "prj_01JTEST", environment: "development""#));

  for target in [
    "storefront",
    "/development",
    "storefront/",
    "storefront/dev/extra",
    "storefront/dev_1",
  ] {
    let error = Cli::try_parse_from(["onekey", "group", "create", target]).unwrap_err();
    assert_eq!(
      error.kind(),
      ErrorKind::ValueValidation,
      "{target}: {error}"
    );
  }

  for target in ["prj_01JTEST/development", "env_482731/development"] {
    let error = Cli::try_parse_from(["onekey", "init", target, "--from", ".env"]).unwrap_err();
    assert_eq!(
      error.kind(),
      ErrorKind::ValueValidation,
      "{target}: {error}"
    );
  }
}

#[test]
fn top_level_help_lists_every_executable_environment_variable() {
  let help = Cli::command().render_long_help().to_string();
  assert!(help.contains("Environment variables:"), "{help}");
  let environment_help = help
    .split_once("Environment variables:")
    .unwrap()
    .1
    .split_once("Run 'onekey help <command>'")
    .unwrap()
    .0;
  for name in executable_environment_names() {
    assert!(
      environment_help.contains(name),
      "missing {name} from environment variable help:\n{help}"
    );
  }
  assert!(
    environment_help.contains("Bearer token for a machine runner or AI agent"),
    "{help}"
  );
}

#[test]
fn every_command_help_has_examples() {
  fn check(
    command: &mut clap::Command,
    parent: &str,
  ) {
    for subcommand in command.get_subcommands_mut() {
      if subcommand.get_name() == "help" {
        continue;
      }
      let path = format!("{parent} {}", subcommand.get_name());
      let help = subcommand.render_long_help().to_string();
      assert!(help.contains("Examples:"), "{path}: {help}");
      check(subcommand, &path);
    }
  }

  check(&mut Cli::command(), "onekey");
}

#[test]
fn every_visible_argument_has_a_description() {
  fn check(
    command: &clap::Command,
    parent: &str,
  ) {
    for argument in command.get_arguments() {
      if argument.is_hide_set() || matches!(argument.get_id().as_str(), "help" | "version") {
        continue;
      }
      assert!(
        argument.get_help().is_some(),
        "{parent}: argument '{}' has no help text",
        argument.get_id()
      );
    }

    for subcommand in command.get_subcommands() {
      if subcommand.get_name() == "help" {
        continue;
      }
      check(subcommand, &format!("{parent} {}", subcommand.get_name()));
    }
  }

  check(&Cli::command(), "onekey");
}

#[test]
fn server_and_admin_commands_are_not_part_of_this_cli() {
  for command in ["server", "admin"] {
    let error = Cli::try_parse_from(["onekey", command]).unwrap_err();
    assert_eq!(
      error.kind(),
      ErrorKind::InvalidSubcommand,
      "{command}: {error}"
    );
  }
}

#[test]
fn missing_subcommands_show_contextual_help() {
  let cases: &[(&[&str], &str)] = &[
    (&["onekey"], "Quickstart:"),
    (&["onekey", "client"], "onekey client connect local"),
    (
      &["onekey", "project"],
      "onekey project create payment-service",
    ),
    (
      &["onekey", "group"],
      "onekey group show payment-service/production",
    ),
    (
      &["onekey", "secret"],
      "onekey secret list payment-service/production",
    ),
    (
      &["onekey", "token"],
      "onekey token create payment-service --name deploy",
    ),
    (&["onekey", "cache"], "onekey cache clean --all --yes"),
  ];

  for (arguments, expected) in cases {
    let help = contextual_help(arguments);
    assert!(help.contains("Usage:"), "{arguments:?}: {help}");
    assert!(help.contains(expected), "{arguments:?}: {help}");
  }
}

#[test]
fn token_create_expiry_flag_accepts_hours_days_and_never() {
  for value in ["1h", "26280h", "1d", "1095d", "never"] {
    assert!(
      Cli::try_parse_from([
        "onekey",
        "token",
        "create",
        "env_123456",
        "--name",
        "deploy",
        "--expires-in",
        value
      ])
      .is_ok(),
      "rejected {value}"
    );
  }
  for value in ["30m", "0h", "1096d", "26281h", "1.5d"] {
    assert!(
      Cli::try_parse_from([
        "onekey",
        "token",
        "create",
        "env_123456",
        "--name",
        "deploy",
        "--expires-in",
        value
      ])
      .is_err(),
      "accepted {value}"
    );
  }
}

#[test]
fn incomplete_secret_commands_show_examples_and_environment_help() {
  let cases: &[(&[&str], &[&str])] = &[
    (
      &["onekey", "secret", "list"],
      &[
        "Usage: onekey secret list",
        "payment-service/production",
        "env_482731",
        "onekey group list",
      ],
    ),
    (
      &["onekey", "secret", "set"],
      &[
        "Usage: onekey secret set",
        "onekey secret set payment-service/production API_KEY",
        "uses a masked prompt and displays * for each character",
      ],
    ),
    (
      &["onekey", "secret", "set", "payment-service/production"],
      &[
        "Usage: onekey secret set",
        "onekey secret set payment-service/production API_KEY",
      ],
    ),
    (
      &["onekey", "secret", "get"],
      &[
        "Usage: onekey secret get",
        "onekey secret get payment-service/production API_KEY --reveal",
      ],
    ),
    (
      &["onekey", "secret", "delete"],
      &[
        "Usage: onekey secret delete",
        "onekey secret delete payment-service/production API_KEY --yes",
      ],
    ),
  ];

  for (arguments, expected) in cases {
    let help = contextual_help(arguments);
    assert!(help.contains("Examples:"), "{arguments:?}: {help}");
    assert!(
      help.contains("PROJECT_REF/GROUP_NAME"),
      "{arguments:?}: {help}"
    );
    for text in *expected {
      assert!(help.contains(text), "{arguments:?}: {help}");
    }
  }
}

#[test]
fn resource_parameters_use_consistent_value_names() {
  let cases: &[(&[&str], &str)] = &[
    (&["onekey", "init", "--help"], "[PROJECT_NAME/GROUP_NAME]"),
    (&["onekey", "project", "create", "--help"], "<PROJECT_NAME>"),
    (&["onekey", "project", "show", "--help"], "<PROJECT_REF>"),
    (
      &["onekey", "project", "rename", "--help"],
      "<PROJECT_REF> <NEW_PROJECT_NAME>",
    ),
    (&["onekey", "project", "delete", "--help"], "<PROJECT_REF>"),
    (
      &["onekey", "group", "create", "--help"],
      "<PROJECT_REF/GROUP_NAME>",
    ),
    (&["onekey", "group", "list", "--help"], "[PROJECT_REF]"),
    (&["onekey", "group", "show", "--help"], "<GROUP_REF>"),
    (
      &["onekey", "group", "rename", "--help"],
      "<GROUP_REF> <NEW_GROUP_NAME>",
    ),
    (&["onekey", "group", "delete", "--help"], "<GROUP_REF>"),
    (&["onekey", "secret", "list", "--help"], "<GROUP_REF>"),
    (&["onekey", "secret", "set", "--help"], "<GROUP_REF> <KEY>"),
    (&["onekey", "secret", "get", "--help"], "<GROUP_REF> <KEY>"),
    (
      &["onekey", "secret", "delete", "--help"],
      "<GROUP_REF> <KEY>",
    ),
    (&["onekey", "import", "--help"], "<GROUP_REF> <PATH>"),
    (&["onekey", "export", "--help"], "<GROUP_REF>"),
    (&["onekey", "token", "create", "--help"], "<PROJECT>"),
    (&["onekey", "token", "list", "--help"], "<PROJECT>"),
    (&["onekey", "run", "--help"], "[PROJECT]"),
  ];

  for (arguments, expected) in cases {
    let help = contextual_help(arguments);
    assert!(help.contains(expected), "{arguments:?}: {help}");
  }
}

#[test]
fn other_incomplete_commands_show_full_help() {
  let cases: &[&[&str]] = &[
    &["onekey", "client", "connect"],
    &["onekey", "project", "rename", "payment-service"],
    &["onekey", "group", "show"],
    &["onekey", "import"],
    &["onekey", "export", "payment-service/production"],
    &["onekey", "token", "create", "payment-service/production"],
    &["onekey", "run"],
    &["onekey", "run", "--"],
    &["onekey", "restore"],
  ];

  for arguments in cases {
    let help = contextual_help(arguments);
    assert!(help.contains("Usage:"), "{arguments:?}: {help}");
    assert!(help.contains("Example"), "{arguments:?}: {help}");
  }
}

#[test]
fn non_missing_argument_errors_are_preserved() {
  let unknown =
    Cli::try_parse_with_help_from(["onekey", "secret", "list", "--unknown"]).unwrap_err();
  assert_eq!(unknown.kind(), ErrorKind::UnknownArgument);

  let conflict = Cli::try_parse_with_help_from([
    "onekey",
    "export",
    "payment-service/production",
    "--output",
    ".env",
    "--stdout",
  ])
  .unwrap_err();
  assert_eq!(conflict.kind(), ErrorKind::ArgumentConflict);
}

#[test]
fn binary_prints_contextual_help_and_keeps_the_usage_error_exit_code() {
  let output = ProcessCommand::new(env!("CARGO_BIN_EXE_onekey"))
    .args(["secret", "list"])
    .output()
    .unwrap();

  assert_eq!(output.status.code(), Some(2));
  let help = String::from_utf8(output.stdout).unwrap();
  assert!(help.contains("Usage: onekey secret list"), "{help}");
  assert!(
    help.contains("onekey secret list payment-service/production"),
    "{help}"
  );
  assert!(output.stderr.is_empty());
}

#[test]
fn login_token_reads_stdin_and_saves_an_encrypted_runner_credential() {
  let directory = tempfile::TempDir::new().unwrap();
  let token = "dbs_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
  let mut child = ProcessCommand::new(env!("CARGO_BIN_EXE_onekey"))
    .args([
      "--data-dir",
      directory.path().to_str().unwrap(),
      "--json",
      "login",
      "--token",
    ])
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .unwrap();
  child
    .stdin
    .take()
    .unwrap()
    .write_all(format!("{token}\n").as_bytes())
    .unwrap();
  let output = child.wait_with_output().unwrap();

  assert!(output.status.success(), "{:?}", output);
  assert!(!String::from_utf8_lossy(&output.stdout).contains(token));
  assert!(!String::from_utf8_lossy(&output.stderr).contains(token));
  let server = ResolvedServer {
    url: "http://localhost:8840".into(),
    source: ServerSource::Default,
    config_path: directory.path().join("config.toml"),
    config: ClientConfig::default(),
  };
  let stored = session::load(&server).unwrap().unwrap();
  assert_eq!(stored.token, token);
  assert!(stored.email.is_none());
  let encrypted = std::fs::read(directory.path().join("session")).unwrap();
  assert!(
    !encrypted
      .windows(token.len())
      .any(|value| value == token.as_bytes())
  );
}
