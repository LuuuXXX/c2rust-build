use std::fmt;

#[derive(Debug)]
pub enum Error {
    ConfigToolNotFound,
    CommandExecutionFailed(String),
    ConfigSaveFailed(String),
    Io(std::io::Error),
    Json(String),
    HookLibraryNotFound,
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
            Error::Io(err) => {
                write!(f, "IO error: {}", err)
            }
            Error::Json(msg) => {
                write!(f, "JSON error: {}", msg)
            }
            Error::HookLibraryNotFound => {
                write!(f, "Hook library not found. Set C2RUST_HOOK_LIB environment variable to the path of libhook.so")
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
