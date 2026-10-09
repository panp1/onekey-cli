use clap::Args;
use std::path::PathBuf;

use crate::constants::help::PROJECT_ARG_HELP;

#[derive(Args, Debug)]
pub struct RunArgs {
  #[arg(value_name = "PROJECT", help = PROJECT_ARG_HELP)]
  pub environment: Option<String>,
  /// Runner or personal token for this invocation. Overrides ONEKEY_TOKEN and the saved credential.
  #[arg(short = 't', long, value_name = "TOKEN")]
  pub token: Option<String>,
  /// Render YAML #{{NAME}}# placeholders and pass the result to the child's stdin.
  #[arg(long, value_name = "YAML")]
  pub template: Option<PathBuf>,
  /// Shell script run with `sh -c`, so "$SECRET" expands inside the child. Use instead of `-- COMMAND`.
  #[arg(
    short = 'c',
    long = "shell",
    value_name = "SCRIPT",
    conflicts_with = "command"
  )]
  pub shell: Option<String>,
  /// Command to run with the injected secrets.
  #[arg(last = true, required_unless_present = "shell")]
  pub command: Vec<String>,
}

pub(crate) const HELP: &str = "\
Offline cache lifetime is configured by an administrator in Settings → Security.
Expired caches are refused; a lifetime of 0 disables offline fallback.
With --template, quoted and unquoted #{{NAME}}# placeholders in YAML string
values are replaced exactly once. An unquoted whole-value placeholder becomes an
integer or boolean when the secret is exactly one (3, -1, true); quoted, block
and embedded placeholders stay strings. Missing secrets refuse to start the child.
The original file is unchanged; rendered YAML goes only to the child's stdin.

Examples:
  onekey run -- npm run dev
  onekey run -c 'psql \"$DATABASE_URL\" -c \"select 1\"'
  onekey run payment-service -- npm run dev
  onekey run payment-service -t dbs_xxx -- npm start
  onekey run payment-service --template deployment.yaml -- kubectl apply -f -
  onekey run payment-service --template values.yaml -- helm upgrade --install myapp ./chart -f -
";
