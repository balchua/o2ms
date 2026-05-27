use std::path::PathBuf;

use crate::error::AppError;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CliOptions {
    pub show_help: bool,
    pub config_path: Option<PathBuf>,
    pub bind_host: Option<String>,
    pub bind_port: Option<u16>,
    pub issuer_base_url: Option<String>,
    pub log_level: Option<String>,
    pub health_endpoint_enabled: Option<bool>,
    pub runtime_client_registration_enabled: Option<bool>,
}

impl CliOptions {
    /// Parse supported CLI flags from the process argument vector.
    ///
    /// # Errors
    ///
    /// Returns an error when an argument is unknown or a flag value is missing
    /// or invalid.
    pub fn parse() -> Result<Self, AppError> {
        Self::parse_from(std::env::args())
    }

    /// Parse supported CLI flags from an arbitrary iterator.
    ///
    /// # Errors
    ///
    /// Returns an error when an argument is unknown or a flag value is missing
    /// or invalid.
    pub fn parse_from<I>(args: I) -> Result<Self, AppError>
    where
        I: IntoIterator<Item = String>,
    {
        let mut options = Self::default();
        let mut args = args.into_iter();
        let _ = args.next();

        while let Some(arg) = args.next() {
            if matches!(arg.as_str(), "--help" | "-h") {
                options.show_help = true;
                continue;
            }

            match arg.as_str() {
                "--config" => {
                    options.config_path = Some(PathBuf::from(next_value(&mut args, "--config")?));
                }
                "--bind-host" => {
                    options.bind_host = Some(next_value(&mut args, "--bind-host")?);
                }
                "--bind-port" => {
                    let value = next_value(&mut args, "--bind-port")?;
                    options.bind_port = Some(
                        value
                            .parse::<u16>()
                            .map_err(|_| AppError::InvalidArguments(format!(
                                "--bind-port must be a valid u16, got '{value}'"
                            )))?,
                    );
                }
                "--issuer-base-url" => {
                    options.issuer_base_url = Some(next_value(&mut args, "--issuer-base-url")?);
                }
                "--log-level" => {
                    options.log_level = Some(next_value(&mut args, "--log-level")?);
                }
                "--health-endpoint-enabled" => {
                    let value = next_value(&mut args, "--health-endpoint-enabled")?;
                    options.health_endpoint_enabled = Some(parse_bool(
                        "--health-endpoint-enabled",
                        value.as_str(),
                    )?);
                }
                "--runtime-client-registration-enabled" => {
                    let value = next_value(&mut args, "--runtime-client-registration-enabled")?;
                    options.runtime_client_registration_enabled =
                        Some(parse_bool("--runtime-client-registration-enabled", value.as_str())?);
                }
                _ if arg.starts_with("--config=") => {
                    options.config_path = Some(PathBuf::from(value_after_equals(&arg)?));
                }
                _ if arg.starts_with("--bind-host=") => {
                    options.bind_host = Some(value_after_equals(&arg)?);
                }
                _ if arg.starts_with("--bind-port=") => {
                    let value = value_after_equals(&arg)?;
                    options.bind_port = Some(
                        value
                            .parse::<u16>()
                            .map_err(|_| AppError::InvalidArguments(format!(
                                "--bind-port must be a valid u16, got '{value}'"
                            )))?,
                    );
                }
                _ if arg.starts_with("--issuer-base-url=") => {
                    options.issuer_base_url = Some(value_after_equals(&arg)?);
                }
                _ if arg.starts_with("--log-level=") => {
                    options.log_level = Some(value_after_equals(&arg)?);
                }
                _ if arg.starts_with("--health-endpoint-enabled=") => {
                    let value = value_after_equals(&arg)?;
                    options.health_endpoint_enabled = Some(parse_bool(
                        "--health-endpoint-enabled",
                        value.as_str(),
                    )?);
                }
                _ if arg.starts_with("--runtime-client-registration-enabled=") => {
                    let value = value_after_equals(&arg)?;
                    options.runtime_client_registration_enabled =
                        Some(parse_bool("--runtime-client-registration-enabled", value.as_str())?);
                }
                _ => {
                    return Err(AppError::InvalidArguments(format!(
                        "unknown argument '{arg}'. Use --help to see supported flags"
                    )));
                }
            }
        }

        Ok(options)
    }

    #[must_use]
    pub fn help_text() -> &'static str {
        "oauth2-mock-server\n\n\
Usage:\n  cargo run -p oauth2-mock-server -- [options]\n\n\
Options:\n  --config <path>                               Path to a YAML config file\n  --bind-host <host>                            Override server.bind_host\n  --bind-port <port>                            Override server.bind_port\n  --issuer-base-url <url>                       Override issuer.base_url\n  --log-level <level>                           Override server.log_level\n  --health-endpoint-enabled <true|false>        Override server.health_endpoint_enabled\n  --runtime-client-registration-enabled <bool>  Override server.runtime_client_registration_enabled\n  -h, --help                                    Show this help text\n"
    }
}

fn next_value<I>(args: &mut I, flag: &str) -> Result<String, AppError>
where
    I: Iterator<Item = String>,
{
    args.next().ok_or_else(|| {
        AppError::InvalidArguments(format!(
            "{flag} requires a value. Use --help to see supported flags"
        ))
    })
}

fn value_after_equals(arg: &str) -> Result<String, AppError> {
    arg.split_once('=')
        .map(|(_, value)| value.to_string())
        .ok_or_else(|| AppError::InvalidArguments(format!("missing '=' value in '{arg}'")))
}

fn parse_bool(flag: &str, value: &str) -> Result<bool, AppError> {
    value.parse::<bool>().map_err(|_| {
        AppError::InvalidArguments(format!(
            "{flag} must be 'true' or 'false', got '{value}'"
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::CliOptions;
    use crate::AppError;

    #[test]
    fn parses_supported_flags() -> Result<(), Box<dyn std::error::Error>> {
        let options = CliOptions::parse_from([
            "oauth2-mock-server".to_string(),
            "--config=configs/mock-server.yaml".to_string(),
            "--bind-host".to_string(),
            "0.0.0.0".to_string(),
            "--bind-port=9191".to_string(),
            "--issuer-base-url".to_string(),
            "http://127.0.0.1:9191".to_string(),
            "--log-level".to_string(),
            "debug".to_string(),
            "--health-endpoint-enabled=false".to_string(),
            "--runtime-client-registration-enabled".to_string(),
            "false".to_string(),
        ])?;

        assert_eq!(options.bind_host.as_deref(), Some("0.0.0.0"));
        assert_eq!(options.bind_port, Some(9191));
        assert_eq!(
            options.issuer_base_url.as_deref(),
            Some("http://127.0.0.1:9191")
        );
        assert_eq!(options.log_level.as_deref(), Some("debug"));
        assert_eq!(options.health_endpoint_enabled, Some(false));
        assert_eq!(options.runtime_client_registration_enabled, Some(false));
        Ok(())
    }

    #[test]
    fn rejects_unknown_flags() {
        let result = CliOptions::parse_from([
            "oauth2-mock-server".to_string(),
            "--wat".to_string(),
        ]);

        assert!(matches!(result, Err(AppError::InvalidArguments(message)) if message.contains("unknown argument")));
    }
}
