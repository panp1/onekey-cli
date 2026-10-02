use clap::Subcommand;

use crate::constants::help::ENVIRONMENT_ARG_HELP;

#[derive(Subcommand, Debug)]
pub enum SecretCommand {
  /// List secret keys in an environment (values are never shown).
  #[command(after_help = LIST_HELP)]
  List {
    #[arg(value_name = "GROUP_REF", help = ENVIRONMENT_ARG_HELP)]
    environment: String,
  },
  /// Set a secret through a masked prompt or read it from standard input.
  #[command(after_help = SET_HELP)]
  Set {
    #[arg(value_name = "GROUP_REF", help = ENVIRONMENT_ARG_HELP)]
    environment: String,
    /// Secret key name.
    key: String,
    /// Read until EOF. In a terminal, finish with Ctrl+D (Ctrl+Z then Enter on Windows).
    #[arg(long)]
    stdin: bool,
  },
  /// Set or clear a secret's usage description; the value and version stay unchanged.
  #[command(after_help = DESCRIBE_HELP)]
  Describe {
    #[arg(value_name = "GROUP_REF", help = ENVIRONMENT_ARG_HELP)]
    environment: String,
    /// Secret key name.
    key: String,
    /// Plain-text usage guidance (up to 2 KiB). Never put a secret value here.
    #[arg(required_unless_present = "clear")]
    description: Option<String>,
    /// Remove the description.
    #[arg(long, conflicts_with = "description")]
    clear: bool,
  },
  /// Show secret metadata, or print its value with --reveal.
  #[command(after_help = GET_HELP)]
  Get {
    #[arg(value_name = "GROUP_REF", help = ENVIRONMENT_ARG_HELP)]
    environment: String,
    /// Secret key name.
    key: String,
    /// Print the actual value after interactive password confirmation.
    #[arg(long)]
    reveal: bool,
  },
  /// Delete a secret from an environment.
  ///
  /// Asks for confirmation unless --yes is passed.
  #[command(after_help = DELETE_HELP)]
  Delete {
    #[arg(value_name = "GROUP_REF", help = ENVIRONMENT_ARG_HELP)]
    environment: String,
    /// Secret key name.
    key: String,
    /// Skip the confirmation prompt (for automation).
    #[arg(long)]
    yes: bool,
  },
}

pub(crate) const HELP: &str = "\
Examples:
  onekey secret list payment-service/production
  onekey secret set payment-service/production API_KEY
  onekey secret describe payment-service/production API_KEY 'Stripe live key; billing service only.'
  onekey secret get payment-service/production API_KEY
  onekey secret get payment-service/production API_KEY --reveal
  onekey secret delete payment-service/production API_KEY

Use PROJECT_REF/GROUP_NAME for readable references. PROJECT_REF can be a
project ID or name. Immutable environment IDs such as env_482731 also work.
";
const LIST_HELP: &str = "\
Examples:
  onekey secret list payment-service/production
  onekey secret list env_482731

Run `onekey group list` to find an environment.
";
const SET_HELP: &str = "\
Examples:
  onekey secret set payment-service/production API_KEY
  onekey secret set payment-service/production API_KEY --stdin
  printf '%s' \"$API_KEY\" | onekey secret set payment-service/production API_KEY --stdin

Without --stdin, OneKey uses a masked prompt and displays * for each character.

With --stdin, OneKey reads the value until EOF. In a terminal, paste or type
the value, then press Ctrl+D. On Windows, press Ctrl+Z, then Enter. Piped input
is read exactly as supplied.
";
const DESCRIBE_HELP: &str = "\
Examples:
  onekey secret describe payment-service/production API_KEY 'Stripe live key; billing service only.'
  onekey secret describe payment-service/production API_KEY --clear

The description is metadata: people and scoped tokens that can list the key see
it in `onekey ls`, the console and MCP. Needs maintainer, or a personal token
with write access to the key.
";
const GET_HELP: &str = "\
Examples:
  onekey secret get payment-service/production API_KEY
  onekey secret get payment-service/production API_KEY --reveal
";
const DELETE_HELP: &str = "\
Examples:
  onekey secret delete payment-service/production API_KEY
  onekey secret delete payment-service/production API_KEY --yes
";
