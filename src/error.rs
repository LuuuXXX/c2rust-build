use std::fmt;

#[derive(Debug)]
pub enum Error {
    ConfigToolNotFound,
    CommandExecutionFailed(String),
    ConfigSaveFailed(String),
    ConfigReadFailed(String),
    MissingParameter(String),
    IoError(std::io::Error),
    JsonError(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::ConfigToolNotFound => {
                write!(f, "c2rust-config not found. Please install c2rust-config first.")
            }
            Error::CommandExecutionFailed(msg) => {
                write!(f, "Command execution failed: {}", msg)
            }
            Error::ConfigSaveFailed(msg) => {
                write!(f, "Failed to save configuration: {}", msg)
            }
            Error::ConfigReadFailed(msg) => {
                write!(f, "Failed to read configuration: {}", msg)
            }
            Error::MissingParameter(msg) => {
                write!(f, "Missing parameter: {}", msg)
            }
            Error::IoError(err) => {
                write!(f, "IO error: {}", err)
            }
            Error::JsonError(msg) => {
                write!(f, "JSON error: {}", msg)
            }
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::IoError(err)
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::JsonError(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
