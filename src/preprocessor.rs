use crate::error::{Error, Result};
use std::process::Command;

/// Get the clang path from environment variable or use default
/// Note: This function is currently unused but kept for potential future use
/// in case clang verification is needed for diagnostic purposes.
#[allow(dead_code)]
fn get_clang_path() -> String {
    std::env::var("C2RUST_CLANG").unwrap_or_else(|_| "clang".to_string())
}

/// Verify that clang is available
/// Note: This function is currently unused since preprocessing is now done by libhook.so.
/// It is kept for potential future diagnostic purposes or if clang is needed for other operations.
#[allow(dead_code)]
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

