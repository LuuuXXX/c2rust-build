use crate::error::{Error, Result};
use std::process::Command;

/// Get the clang path from environment variable or use default
fn get_clang_path() -> String {
    std::env::var("C2RUST_CLANG").unwrap_or_else(|_| "clang".to_string())
}

/// Verify that clang is available
/// Note: This function is kept for backward compatibility but is no longer
/// strictly required since preprocessing is now done by libhook.so
pub fn verify_clang() -> Result<()> {
    let clang_path = get_clang_path();
    Command::new(&clang_path)
        .arg("--version")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|_| ())
        .ok_or(Error::ClangNotFound)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_clang_path_default() {
        // Clear the environment variable for this test
        std::env::remove_var("C2RUST_CLANG");
        assert_eq!(get_clang_path(), "clang");
    }

    #[test]
    fn test_get_clang_path_custom() {
        std::env::set_var("C2RUST_CLANG", "/custom/path/clang");
        assert_eq!(get_clang_path(), "/custom/path/clang");
        std::env::remove_var("C2RUST_CLANG");
    }
}

