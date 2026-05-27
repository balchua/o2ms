#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("invalid configuration: {0}")]
    InvalidConfig(String),
    #[error("invalid arguments: {0}")]
    InvalidArguments(String),
    #[error(transparent)]
    Config(#[from] config::ConfigError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
