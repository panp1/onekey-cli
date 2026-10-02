use clap::Subcommand;

use crate::services::token;

fn parse_expiry(value: &str) -> Result<String, String> {
  token::expiry_duration(value)
    .map(|_| value.to_owned())
    .map_err(str::to_owned)
}

#[derive(Subcommand, Debug)]
pub enum TokenCommand {
  /// Create a runner token for a project (e.g. for CI/CD). It reads every group.
  ///
  /// The token value is shown once at creation. Pass it to client commands
  /// via the ONEKEY_TOKEN environment variable.
  #[command(after_help = CREATE_HELP)]
  Create {
    /// Project name or ID.
    #[arg(value_name = "PROJECT")]
    project: String,
    /// Display name for the token.
    #[arg(long)]
    name: String,
    /// Token role.
    #[arg(long, default_value = "runner")]
    role: String,
    /// Token lifetime: never or a whole number followed by h or d (maximum 3 years).
    #[arg(long, value_name = "DURATION", value_parser = parse_expiry)]
    expires_in: Option<String>,
  },
  /// List a project's runner tokens.
  #[command(after_help = LIST_HELP)]
  List {
    /// Project name or ID.
    #[arg(value_name = "PROJECT")]
    project: String,
  },
  /// Revoke a token by ID.
  #[command(after_help = REVOKE_HELP)]
  Revoke {
    /// ID of the token to revoke.
    token_id: String,
  },
}

pub(crate) const HELP: &str = "\
Examples:
  onekey token create payment-service --name deploy
  onekey token create payment-service --name deploy --expires-in 12h
  onekey token list payment-service
  onekey token revoke tok_01ABCDEF
";
const CREATE_HELP: &str = "\
Examples:
  onekey token create payment-service --name deploy
  onekey token create payment-service --name deploy --expires-in 45d
  onekey token create payment-service --name deploy --expires-in never
";
const LIST_HELP: &str = "\
Examples:
  onekey token list payment-service
";
const REVOKE_HELP: &str = "\
Examples:
  onekey token revoke tok_01ABCDEF
";
