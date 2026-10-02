use clap::Parser;
use onekey_cli::cli::args::{Cli, Command, ProfileCommand};
use onekey_cli::config::{list_profiles, profile_dir, set_active_profile, validate_profile_name};
use tempfile::TempDir;

#[test]
fn profile_names_are_restricted() {
  for ok in ["default", "public", "vpn-1", "ci_2"] {
    assert!(validate_profile_name(ok).is_ok(), "{ok}");
  }
  for bad in ["", "Public", "a/b", "..", "has space", &"x".repeat(33)] {
    assert!(validate_profile_name(bad).is_err(), "{bad:?}");
  }
}

#[test]
fn default_profile_is_the_home_and_others_are_subdirectories() {
  let home = TempDir::new().unwrap();
  assert_eq!(profile_dir(home.path(), "default").unwrap(), home.path());
  assert_eq!(
    profile_dir(home.path(), "public").unwrap(),
    home.path().join("profiles").join("public")
  );
  assert!(profile_dir(home.path(), "../escape").is_err());
}

#[test]
fn profiles_are_listed_and_the_active_one_is_saved() {
  let home = TempDir::new().unwrap();
  std::fs::create_dir_all(home.path().join("profiles/vpn")).unwrap();
  std::fs::create_dir_all(home.path().join("profiles/public")).unwrap();
  std::fs::create_dir_all(home.path().join("profiles/Bad Name")).unwrap();
  assert_eq!(list_profiles(home.path()), ["default", "public", "vpn"]);
  set_active_profile(home.path(), "public").unwrap();
  assert_eq!(
    std::fs::read_to_string(home.path().join("profile"))
      .unwrap()
      .trim(),
    "public"
  );
  assert!(set_active_profile(home.path(), "No").is_err());
}

#[test]
fn config_and_profile_commands_parse() {
  let cli = Cli::try_parse_from([
    "onekey",
    "config",
    "-p",
    "ci",
    "--url",
    "https://x/cli",
    "--token-stdin",
    "--env",
    "app/prod",
    "--yes",
  ])
  .unwrap();
  let Command::Config(args) = cli.command else {
    panic!("expected config")
  };
  assert_eq!(args.profile.as_deref(), Some("ci"));
  assert_eq!(args.url.as_deref(), Some("https://x/cli"));
  assert!(args.token_stdin && args.yes);
  assert_eq!(args.environment.as_deref(), Some("app/prod"));

  let cli = Cli::try_parse_from(["onekey", "profile", "use", "public"]).unwrap();
  assert!(
    matches!(cli.command, Command::Profile { command: ProfileCommand::Use { name } } if name == "public")
  );
  assert!(Cli::try_parse_from(["onekey", "profile", "remove"]).is_err());
}
