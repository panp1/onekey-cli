use clap::Args;

pub(crate) const LOGIN_HELP: &str = "\
Examples:
  onekey login
  onekey login --token
  printf '%s' \"$TOKEN\" | onekey login --token
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
