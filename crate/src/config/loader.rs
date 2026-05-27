use std::{
    env,
    path::{Path, PathBuf},
};

use config::{builder::DefaultState, Config as RawConfig, ConfigBuilder, Environment, File, FileFormat};

use crate::{cli::CliOptions, config::model::AppConfig, error::AppError};

use super::validate::validate;

pub const DEFAULT_CONFIG_PATH: &str = "configs/mock-server.yaml";
pub const CONFIG_ENV_VAR: &str = "O2MS_CONFIG";
pub const CONFIG_ENV_PREFIX: &str = "O2MS";

/// Resolve the effective application config and validate it.
///
/// # Errors
///
/// Returns an error when the resulting config is invalid.
pub fn load(config: Option<AppConfig>) -> Result<AppConfig, AppError> {
    let resolved = config.unwrap_or_default();
    validate(&resolved)?;
    Ok(resolved)
}

/// Load the application config from a YAML string.
///
/// # Errors
///
/// Returns an error when the YAML is invalid or the config fails validation.
pub fn load_from_yaml(yaml: &str) -> Result<AppConfig, AppError> {
    let builder = RawConfig::builder().add_source(File::from_str(yaml, FileFormat::Yaml));
    load_from_builder(builder)
}

/// Load the application config from a YAML file.
///
/// # Errors
///
/// Returns an error when the file cannot be read, the YAML is invalid, or the
/// config fails validation.
pub fn load_from_path(path: &Path) -> Result<AppConfig, AppError> {
    let builder = RawConfig::builder().add_source(File::from(path).format(FileFormat::Yaml));
    load_from_builder(builder)
}

/// Load config from the environment-configured path, the default config path,
/// nested environment variables, or built-in defaults if no file exists.
///
/// # Errors
///
/// Returns an error when a selected config file cannot be read, the YAML is
/// invalid, or the resulting config fails validation.
pub fn load_from_sources() -> Result<AppConfig, AppError> {
    load_from_sources_with_overrides(&CliOptions::default())
}

/// Load config from file/env sources and then apply CLI overrides.
///
/// # Errors
///
/// Returns an error when a selected config file cannot be read, the YAML is
/// invalid, or the resulting config fails validation.
pub fn load_from_sources_with_overrides(overrides: &CliOptions) -> Result<AppConfig, AppError> {
    let mut builder = RawConfig::builder();

    if let Some(path_buf) = &overrides.config_path {
        tracing::info!(config_path = %path_buf.display(), "loading config from CLI path");
        builder = builder.add_source(File::from(path_buf.clone()).format(FileFormat::Yaml));
    } else if let Ok(path) = env::var(CONFIG_ENV_VAR) {
        let path_buf = PathBuf::from(&path);
        tracing::info!(config_path = %path_buf.display(), "loading config from environment path");
        builder = builder.add_source(File::from(path_buf).format(FileFormat::Yaml));
    } else {
        let default_path = Path::new(DEFAULT_CONFIG_PATH);
        if default_path.exists() {
            tracing::info!(config_path = %default_path.display(), "loading config from default path");
            builder = builder.add_source(File::from(default_path).format(FileFormat::Yaml));
        } else {
            tracing::info!("no YAML config file found; using built-in defaults");
        }
    }

    builder = builder.add_source(
        Environment::with_prefix(CONFIG_ENV_PREFIX)
            .prefix_separator("_")
            .separator("__")
            .list_separator(",")
            .try_parsing(true)
            .with_list_parse_key("server.cors_allowed_origins"),
    );

    if let Some(bind_host) = &overrides.bind_host {
        builder = builder.set_override("server.bind_host", bind_host.clone())?;
    }
    if let Some(bind_port) = overrides.bind_port {
        builder = builder.set_override("server.bind_port", bind_port)?;
    }
    if let Some(issuer_base_url) = &overrides.issuer_base_url {
        builder = builder.set_override("issuer.base_url", issuer_base_url.clone())?;
    }
    if let Some(log_level) = &overrides.log_level {
        builder = builder.set_override("server.log_level", log_level.clone())?;
    }
    if let Some(health_endpoint_enabled) = overrides.health_endpoint_enabled {
        builder = builder.set_override("server.health_endpoint_enabled", health_endpoint_enabled)?;
    }
    if let Some(runtime_client_registration_enabled) =
        overrides.runtime_client_registration_enabled
    {
        builder = builder.set_override(
            "server.runtime_client_registration_enabled",
            runtime_client_registration_enabled,
        )?;
    }

    load_from_builder(builder)
}

fn load_from_builder(builder: ConfigBuilder<DefaultState>) -> Result<AppConfig, AppError> {
    let config = builder.build()?.try_deserialize::<AppConfig>()?;
    load(Some(config))
}

#[cfg(test)]
mod tests {
    use std::{
        env, fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use config::Config as RawConfig;

    use crate::cli::CliOptions;

    use super::{
        CONFIG_ENV_PREFIX, CONFIG_ENV_VAR, load_from_builder, load_from_path,
        load_from_sources_with_overrides, load_from_yaml,
    };

    fn temp_file_path(file_name: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0_u128, |duration| duration.as_nanos());
        std::env::temp_dir().join(format!("{file_name}-{unique}.yaml"))
    }

    #[test]
    fn loads_config_from_yaml() -> Result<(), Box<dyn std::error::Error>> {
        let config = load_from_yaml(
            r"
server:
  bind_host: 0.0.0.0
  bind_port: 9999
issuer:
  base_url: http://127.0.0.1:9999
",
        )?;

        assert_eq!(config.server.bind_host, "0.0.0.0");
        assert_eq!(config.server.bind_port, 9999);
        Ok(())
    }

    #[test]
    fn loads_config_from_file_path() -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_file_path("oauth2-mock-server-config");
        fs::write(
            &path,
            r"
server:
  bind_port: 9191
issuer:
  base_url: http://127.0.0.1:9191
",
        )?;

        let config = load_from_path(&path)?;
        assert_eq!(config.server.bind_port, 9191);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn config_env_var_name_is_stable() {
        let value = env::var(CONFIG_ENV_VAR).ok();
        let _ = value;
        assert_eq!(CONFIG_ENV_VAR, "O2MS_CONFIG");
    }

    #[test]
    fn config_env_prefix_is_stable() {
        assert_eq!(CONFIG_ENV_PREFIX, "O2MS");
    }

    #[test]
    fn layered_overrides_apply_to_runtime_and_oauth_settings() -> Result<(), Box<dyn std::error::Error>> {
        let builder = RawConfig::builder()
            .set_override("server.bind_port", 9191)?
            .set_override("issuer.base_url", "http://127.0.0.1:9191")?
            .set_override("oauth.access_token_ttl_seconds", 120_u64)?;

        let config = load_from_builder(builder)?;
        assert_eq!(config.server.bind_port, 9191);
        assert_eq!(config.issuer.base_url, "http://127.0.0.1:9191");
        assert_eq!(config.oauth.access_token_ttl_seconds, 120);
        Ok(())
    }

    #[test]
    fn cli_overrides_apply_after_file_loading() -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_file_path("oauth2-mock-server-cli-overrides");
        fs::write(
            &path,
            r"
server:
  bind_port: 8090
issuer:
  base_url: http://127.0.0.1:8090
",
        )?;

        let config = load_from_sources_with_overrides(&CliOptions {
            config_path: Some(path.clone()),
            bind_port: Some(9191),
            issuer_base_url: Some("http://127.0.0.1:9191".to_string()),
            ..CliOptions::default()
        })?;

        assert_eq!(config.server.bind_port, 9191);
        assert_eq!(config.issuer.base_url, "http://127.0.0.1:9191");
        fs::remove_file(path)?;
        Ok(())
    }
}
