use crate::error::Result;
use std::path::Path;

/// Check if there are any modifications in the .c2rust directory and auto-commit if needed.
/// 
/// This function checks the git repository located at <project_root>/.c2rust/.git
/// for any changes in the .c2rust directory and commits them if changes exist.
/// 
/// This is a best-effort operation - any errors are logged but do not fail the overall
/// workflow, since auto-commit is a final-stage convenience feature.
/// 
/// # Arguments
/// 
/// * `project_root` - The absolute path to the project root directory
/// 
/// # Returns
/// 
/// Returns `Ok(())` in all cases. Git operation errors are logged to stderr but not propagated.
/// This ensures that auto-commit failures never cause the overall build process to fail.
pub fn auto_commit_if_modified(project_root: &Path) -> Result<()> {
    let c2rust_dir = project_root.join(".c2rust");
    let git_dir = c2rust_dir.join(".git");
    
    // Check if .git directory exists
    if !git_dir.exists() || !git_dir.is_dir() {
        // .git doesn't exist, nothing to commit
        return Ok(());
    }
    
    // All git operations are best-effort - log errors but don't fail
    if let Err(e) = try_auto_commit(&c2rust_dir) {
        eprintln!("Warning: Auto-commit failed: {}", e);
        eprintln!("Continuing without auto-commit.");
    }
    
    Ok(())
}

/// Internal helper that performs the actual git operations.
/// Errors are returned to the caller for logging.
fn try_auto_commit(c2rust_dir: &Path) -> std::result::Result<(), String> {
    // Open the repository
    let repo = git2::Repository::open(c2rust_dir)
        .map_err(|e| format!("Failed to open git repository at {}: {}", c2rust_dir.display(), e))?;
    
    // Check if there are any modifications
    let mut index = repo.index()
        .map_err(|e| format!("Failed to get git index: {}", e))?;
    
    // Add all changes to the index
    index.add_all(["."].iter(), git2::IndexAddOption::DEFAULT, None)
        .map_err(|e| format!("Failed to add files to git index: {}", e))?;
    
    index.write()
        .map_err(|e| format!("Failed to write git index: {}", e))?;
    
    // Get the tree for the index
    let tree_id = index.write_tree()
        .map_err(|e| format!("Failed to write tree: {}", e))?;
    
    let tree = repo.find_tree(tree_id)
        .map_err(|e| format!("Failed to find tree: {}", e))?;
    
    // Get HEAD commit
    let head = match repo.head() {
        Ok(head) => head,
        Err(_) => {
            // No HEAD, this might be the first commit
            // Check if there are any changes to commit
            let diff = repo.diff_tree_to_index(None, Some(&index), None)
                .map_err(|e| format!("Failed to create diff: {}", e))?;
            
            // If there are no changes, return early
            // Note: Using len() == 0 because Deltas::is_empty() is nightly-only
            if diff.deltas().len() == 0 {
                return Ok(());
            }
            
            // Create an initial commit
            let sig = repo.signature()
                .map_err(|e| format!("Failed to get git signature (ensure git user.name and user.email are configured): {}", e))?;
            
            repo.commit(
                Some("HEAD"),
                &sig,
                &sig,
                "Auto-commit: c2rust-build changes",
                &tree,
                &[],
            ).map_err(|e| format!("Failed to create initial commit: {}", e))?;
            
            return Ok(());
        }
    };
    
    let parent_commit = head.peel_to_commit()
        .map_err(|e| format!("Failed to get parent commit: {}", e))?;
    
    // Check if there are any changes compared to the parent commit
    let parent_tree = parent_commit.tree()
        .map_err(|e| format!("Failed to get parent tree: {}", e))?;
    
    let diff = repo.diff_tree_to_tree(Some(&parent_tree), Some(&tree), None)
        .map_err(|e| format!("Failed to create diff: {}", e))?;
    
    // If there are no changes, return early
    // Note: Using len() == 0 because Deltas::is_empty() is nightly-only
    if diff.deltas().len() == 0 {
        return Ok(());
    }
    
    // Create the commit
    let sig = repo.signature()
        .map_err(|e| format!("Failed to get git signature (ensure git user.name and user.email are configured): {}", e))?;
    
    repo.commit(
        Some("HEAD"),
        &sig,
        &sig,
        "Auto-commit: c2rust-build changes",
        &tree,
        &[&parent_commit],
    ).map_err(|e| format!("Failed to create commit: {}", e))?;
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;
    
    #[test]
    fn test_auto_commit_no_git_dir() {
        // Test that when .c2rust/.git doesn't exist, function returns Ok
        let temp_dir = TempDir::new().unwrap();
        let result = auto_commit_if_modified(temp_dir.path());
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_auto_commit_with_git_repo() {
        // Create a temp directory with .c2rust/.git
        let temp_dir = TempDir::new().unwrap();
        let c2rust_dir = temp_dir.path().join(".c2rust");
        fs::create_dir(&c2rust_dir).unwrap();
        
        // Initialize a git repository
        let repo = git2::Repository::init(&c2rust_dir).unwrap();
        
        // Set git config for the test repository
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test User").unwrap();
        config.set_str("user.email", "test@example.com").unwrap();
        
        // Create a test file
        let test_file = c2rust_dir.join("test.txt");
        fs::write(&test_file, "test content").unwrap();
        
        // Run auto_commit_if_modified
        let result = auto_commit_if_modified(temp_dir.path());
        assert!(result.is_ok(), "Expected auto_commit to succeed, got: {:?}", result);
        
        // Verify a commit was created
        let head = repo.head().unwrap();
        let commit = head.peel_to_commit().unwrap();
        assert!(commit.message().unwrap().contains("Auto-commit"));
        
        let first_commit_id = commit.id();
        
        // Run auto_commit_if_modified again without any changes
        let result2 = auto_commit_if_modified(temp_dir.path());
        assert!(result2.is_ok(), "Expected second auto_commit to succeed, got: {:?}", result2);
        
        // Verify no new commit was created
        let head2 = repo.head().unwrap();
        let commit2 = head2.peel_to_commit().unwrap();
        assert_eq!(commit2.id(), first_commit_id, "Expected no new commit when there are no changes");
    }
    
    #[test]
    fn test_auto_commit_git_error_is_non_fatal() {
        // Test that git errors don't fail the overall operation
        // This ensures the best-effort behavior is maintained
        let temp_dir = TempDir::new().unwrap();
        let c2rust_dir = temp_dir.path().join(".c2rust");
        fs::create_dir(&c2rust_dir).unwrap();
        
        // Initialize a git repository WITHOUT setting user.name/user.email
        // This will cause git operations to fail when trying to commit
        let _repo = git2::Repository::init(&c2rust_dir).unwrap();
        
        // Create a test file to trigger commit attempt
        let test_file = c2rust_dir.join("test.txt");
        fs::write(&test_file, "test content").unwrap();
        
        // Run auto_commit_if_modified - it should succeed despite git config errors
        let result = auto_commit_if_modified(temp_dir.path());
        
        // The function should return Ok(()) even though git operations failed
        assert!(result.is_ok(), "Expected auto_commit to succeed (non-fatal) even with git errors, got: {:?}", result);
        
        // Note: The warning message would be printed to stderr but we can't easily capture it in unit tests
        // Integration tests can verify the warning output
    }
}
