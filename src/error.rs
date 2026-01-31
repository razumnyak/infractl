use thiserror::Error;

#[derive(Error, Debug)]
#[allow(dead_code)]
pub enum InfraError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("YAML parsing error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("Server error: {0}")]
    Server(String),

    #[error("Authentication error: {0}")]
    Auth(String),

    #[error("Network isolation violation from {0}")]
    NetworkViolation(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Docker error: {0}")]
    Docker(String),

    #[error("Git error: {0}")]
    Git(String),

    #[error("Deployment error: {0}")]
    Deploy(String),

    #[error("Environment variable '{0}' not set")]
    EnvVar(String),
}

pub type Result<T> = std::result::Result<T, InfraError>;
