use clap::Args;

pub(crate) const LOGIN_HELP: &str = "\
Examples:
  onekey login
  onekey login --token
  printf '%s' \"$TOKEN\" | onekey login --token

SSO sign-in is not supported yet. On a server that only allows SSO, create a personal or
runner token in the console and use `onekey login --token`: it covers ls, run, curl and
mcp; manage projects, groups and secrets in the console.
";
pub(crate) const LOGOUT_HELP: &str = "\
Examples:
  onekey logout
";

#[derive(Args, Debug)]
pub struct LoginArgs {
  /// Save a runner or personal token instead of signing in with email and password.
  ///
  /// In a terminal, OneKey prompts for the token without echoing it. When
  /// standard input is piped, OneKey reads the token from standard input.
  #[arg(long)]
  pub token: bool,
}
