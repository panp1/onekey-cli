use clap::Subcommand;

use crate::cli::{environment_target, environment_target::EnvironmentTarget};
use crate::constants::help::ENVIRONMENT_ARG_HELP;

#[derive(Subcommand, Debug)]
pub enum EnvCommand {
  /// Create a group inside a project.
  #[command(after_help = CREATE_HELP)]
  Create {
    /// Existing project and new group, written as PROJECT_REF/GROUP_NAME.
    #[arg(value_name = "PROJECT_REF/GROUP_NAME", value_parser = environment_target::parse_create)]
    target: EnvironmentTarget,
  },
  /// List groups, either for one project or all accessible ones.
  #[command(after_help = LIST_HELP)]
  List {
    /// Limit the listing to this project (ID or name).
    #[arg(value_name = "PROJECT_REF")]
    project: Option<String>,
  },
  /// Show group metadata.
  #[command(after_help = SHOW_HELP)]
  Show {
    #[arg(value_name = "GROUP_REF", help = ENVIRONMENT_ARG_HELP)]
    environment: String,
  },
  /// Rename a group.
  #[command(after_help = RENAME_HELP)]
  Rename {
    #[arg(value_name = "GROUP_REF", help = ENVIRONMENT_ARG_HELP)]
    environment: String,
    /// New group name.
    #[arg(value_name = "NEW_GROUP_NAME")]
    new_name: String,
  },
  /// Delete a group and its secrets.
  ///
  /// Asks for confirmation unless --yes is passed.
  #[command(after_help = DELETE_HELP)]
  Delete {
    #[arg(value_name = "GROUP_REF", help = ENVIRONMENT_ARG_HELP)]
    environment: String,
    /// Skip the confirmation prompt (for automation).
    #[arg(long)]
    yes: bool,
  },
}

pub(crate) const HELP: &str = "\
Examples:
  onekey group create payment-service/production
  onekey group list payment-service
  onekey group show payment-service/production
  onekey group rename payment-service/production prod
  onekey group delete payment-service/staging
";
const CREATE_HELP: &str = "\
Examples:
  onekey group create payment-service/production
";
const LIST_HELP: &str = "\
Examples:
  onekey group list
  onekey group list payment-service
";
const SHOW_HELP: &str = "\
Examples:
  onekey group show payment-service/production
  onekey group show env_482731
";
const RENAME_HELP: &str = "\
Examples:
  onekey group rename payment-service/production prod
";
const DELETE_HELP: &str = "\
Examples:
  onekey group delete payment-service/staging
  onekey group delete payment-service/staging --yes
";
