use clap::Parser;
use onekey_cli::cli::args::{Cli, Command};
use onekey_cli::cli::commands::{
  curl::{curl_config, verbose_flag},
  ls::matches_target,
};

#[test]
fn curl_config_quotes_the_header_and_rejects_line_breaks() {
  assert_eq!(
    curl_config("Bearer abc").unwrap(),
    "header = \"Authorization: Bearer abc\"\n"
  );
  assert_eq!(
    curl_config(r#"Bearer a"b\c"#).unwrap(),
    "header = \"Authorization: Bearer a\\\"b\\\\c\"\n"
  );
  for injected in ["Bearer a\nX-Evil: 1", "Bearer a\r\nX-Evil: 1"] {
    assert!(curl_config(injected).is_err(), "{injected:?}");
  }
}

#[test]
fn curl_refuses_flags_that_would_print_the_authorization_header() {
  let args = |list: &[&str]| list.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
  for flag in [
    "-v",
    "--verbose",
    "--trace",
    "--trace-ascii",
    "-sv",
    "-vL",
    "--config",
    "--config=debug.conf",
    "-Kdebug.conf",
    "-sKdebug.conf",
    "--libcurl=generated.c",
    "--verb",
    "--trac",
    "--conf",
    "--libc",
  ] {
    assert_eq!(
      verbose_flag(&args(&["https://x", flag, "-G"])),
      Some(flag),
      "{flag}"
    );
  }
  assert_eq!(
    verbose_flag(&args(&["--trace=out.log"])),
    Some("--trace=out.log")
  );
  assert_eq!(
    verbose_flag(&args(&[
      "https://x/v1",
      "-sS",
      "-X",
      "POST",
      "-d",
      "verbose",
      "--data-urlencode",
      "v=1",
      "-"
    ])),
    None
  );
}

#[test]
fn ls_target_matches_project_name_or_id() {
  assert!(matches_target(None, "p", "prj_1"));
  assert!(matches_target(Some("p"), "p", "prj_1"));
  assert!(matches_target(Some("prj_1"), "p", "prj_1"));
  assert!(!matches_target(Some("p/e"), "p", "prj_1"));
  assert!(!matches_target(Some("other"), "p", "prj_1"));
}

#[test]
fn run_accepts_a_shell_script_instead_of_a_command() {
  let cli = Cli::try_parse_from(["onekey", "run", "p/e", "-c", "echo \"$X\""]).unwrap();
  let Command::Run(args) = cli.command else {
    panic!("expected run")
  };
  assert_eq!(args.environment.as_deref(), Some("p/e"));
  assert_eq!(args.shell.as_deref(), Some("echo \"$X\""));
  assert!(Cli::try_parse_from(["onekey", "run", "-c", "x", "--", "y"]).is_err());
  assert!(Cli::try_parse_from(["onekey", "run"]).is_err());
}

#[test]
fn curl_passes_curl_arguments_through_and_needs_one_auth_mode() {
  let cli = Cli::try_parse_from([
    "onekey",
    "curl",
    "--basic",
    "me:tok",
    "https://x/api",
    "-G",
    "--data-urlencode",
    "q=a b",
  ])
  .unwrap();
  let Command::Curl(args) = cli.command else {
    panic!("expected curl")
  };
  assert_eq!(args.basic.as_deref(), Some("me:tok"));
  assert_eq!(
    args.args,
    ["https://x/api", "-G", "--data-urlencode", "q=a b"]
  );
  assert!(Cli::try_parse_from(["onekey", "curl", "https://x"]).is_err());
  assert!(
    Cli::try_parse_from([
      "onekey",
      "curl",
      "--basic",
      "a:b",
      "--bearer",
      "c",
      "https://x"
    ])
    .is_err()
  );
}
