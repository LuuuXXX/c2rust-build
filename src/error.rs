use std::fmt;

#[derive(Debug)]
pub enum Error {
    ConfigToolNotFound,
    CommandExecutionFailed(String),
    ConfigSaveFailed(String),
    ConfigError(String),
    Io(std::io::Error),
    IoError(String),
    Json(String),
    HookLibraryNotFound,
    FileSelectionCancelled(String),
    TargetsListNotFound(String),
    NoTargetsFound(String),
    InvalidInput(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::ConfigToolNotFound => {
                write!(
                    f,
                    "c2rust-config not found. Please install c2rust-config first."
                )
            }
            Error::CommandExecutionFailed(msg) => {
                write!(f, "Command execution failed: {}", msg)
            }
            Error::ConfigSaveFailed(msg) => {
                write!(f, "Failed to save configuration: {}", msg)
            }
            Error::ConfigError(msg) => {
                write!(f, "Configuration error: {}", msg)
            }
            Error::Io(err) => {
                write!(f, "IO error: {}", err)
            }
            Error::IoError(msg) => {
                write!(f, "IO error: {}", msg)
            }
            Error::Json(msg) => {
                write!(f, "JSON error: {}", msg)
            }
            Error::HookLibraryNotFound => {
                write!(f, "Hook library not found. Set C2RUST_HOOK_LIB environment variable to the path of libhook.so")
            }
            Error::FileSelectionCancelled(msg) => {
                write!(f, "File selection cancelled: {}", msg)
            }
            Error::TargetsListNotFound(msg) => {
                write!(f, "Targets list not found: {}", msg)
            }
            Error::NoTargetsFound(msg) => {
                write!(f, "No targets found: {}", msg)
            }
            Error::InvalidInput(msg) => {
                write!(f, "Invalid input: {}", msg)
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

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::Json(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
