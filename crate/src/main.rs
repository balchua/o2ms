use std::io::Write;

use oauth2_mock_server::{
    cli::CliOptions,
    config::loader::load_from_sources_with_overrides,
    run,
};

#[tokio::main]
async fn main() -> Result<(), oauth2_mock_server::AppError> {
    let cli_options = CliOptions::parse()?;

    if cli_options.show_help {
        let mut stdout = std::io::stdout().lock();
        stdout.write_all(CliOptions::help_text().as_bytes())?;
        return Ok(());
    }

    let config = load_from_sources_with_overrides(&cli_options)?;
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(config.server.log_level.clone()));

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .without_time()
        .init();

    if let Err(error) = run(config).await {
        tracing::error!(%error, "oauth2 mock server exited with an error");
        return Err(error);
    }

    Ok(())
}
