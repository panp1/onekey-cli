pub(crate) const AFTER_HELP: &str = "\
Environment variables:
  ONEKEY_TOKEN                   Bearer token for a machine runner or AI agent. Overrides the saved login
  ONEKEY_URL                     Server URL for client commands when --server is not set
  ONEKEY_PROFILE                 CLI profile for this invocation (see `onekey profile list`)
  ONEKEY_ENV                     Project for onekey run and curl when the argument is omitted
  ONEKEY_DATA_DIR                State and configuration directory (default: ~/.onekey)

Quickstart:
  onekey config                           # server, token and default project, once
  onekey ls                               # projects, groups, secret names and descriptions
  onekey run myapp -- node server.js      # run with the project's secrets as env vars
  onekey curl --bearer github_token https://api.github.com/user

Sign-in: email and password, or a token (`onekey login --token`). SSO sign-in is not
supported yet. A token covers ls, run, curl and mcp; project, group, secret, import and
export need a password sign-in, so on an SSO-only server manage them in the console.

Run 'onekey help <command>' for details on any command.
";

pub(crate) const PROJECT_ARG_HELP: &str = "Project name or ID. Secrets of all its groups are used. \
Defaults to ONEKEY_ENV, then the project saved with `onekey use`.";
pub(crate) const ENVIRONMENT_ARG_HELP: &str = "Existing group reference: a group ID or \
PROJECT_REF/GROUP_NAME. PROJECT_REF can be a project ID or name. For example: payment-service/production.";
