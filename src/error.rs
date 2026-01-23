use std::fmt;

#[derive(Debug)]
pub enum Error {
    ConfigToolNotFound,
    CommandExecutionFailed(String),
    ConfigSaveFailed(String),
    ConfigReadFailed(String),
    ConfigNotFound(String),
    Io(std::io::Error),
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
            Error::ConfigNotFound(msg) => {
                write!(f, "Configuration not found: {}", msg)
            }
            Error::Io(err) => {
                write!(f, "IO error: {}", err)
            }
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io(err)
    }
}

pub type Result<T> = std::result::Result<T, Error>;
