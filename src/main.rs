#[tokio::main]
async fn main() {
  let cli = onekey_cli::cli::Cli::parse_with_help();
  let json = cli.json;
  match onekey_cli::cli::execute(cli).await {
    Ok(code) => std::process::exit(code),
    Err(error) => {
      if let Some(cancelled) = error.downcast_ref::<onekey_cli::cli::CliCancelled>() {
        if json {
          eprintln!(
            "{}",
            serde_json::json!({"success":false,"error":{"CLI_CANCELLED":cancelled.to_string()}})
          );
        } else {
          eprintln!("{cancelled}");
        }
        std::process::exit(130)
      } else if json {
        eprintln!(
          "{}",
          serde_json::json!({"success":false,"error":{"CLI_ERROR":error.to_string()}})
        );
        std::process::exit(1)
      } else {
        let style = anstyle::Style::new()
          .fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::Red)))
          .effects(anstyle::Effects::BOLD);
        anstream::eprintln!("{style}Error:{style:#} {error:#}");
        std::process::exit(1)
      }
    }
  }
}
