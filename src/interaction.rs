use crate::error::Result;
use crate::preprocessor::PreprocessedFile;
use dialoguer::{theme::ColorfulTheme, MultiSelect};
use std::collections::HashMap;

/// Display preprocessed modules and let user select which ones to keep
pub fn select_modules(
    modules: &HashMap<String, Vec<PreprocessedFile>>,
) -> Result<Vec<String>> {
    if modules.is_empty() {
        println!("No modules found.");
        return Ok(Vec::new());
    }
    
    // Create a sorted list of module names
    let mut module_names: Vec<String> = modules.keys().cloned().collect();
    module_names.sort();
    
    println!("\n=== Discovered Modules ===\n");
    
    // Display module information
    for name in &module_names {
        let files = &modules[name];
        println!("Module: {}", name);
        println!("  Files: {}", files.len());
        for file in files {
            println!("    - {}", file.original_path.display());
        }
        println!();
    }
    
    // Ask user to select modules
    println!("Please select which modules you want to keep:");
    println!("(Use Space to select/deselect, Enter to confirm)\n");
    
    let selections = MultiSelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Select modules")
        .items(&module_names)
        .defaults(&vec![true; module_names.len()]) // All selected by default
        .interact()
        .map_err(|e| crate::error::Error::IoError(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Failed to get user selection: {}", e)
        )))?;
    
    // Get selected module names
    let selected_modules: Vec<String> = selections
        .iter()
        .map(|&idx| module_names[idx].clone())
        .collect();
    
    println!("\nSelected {} module(s):", selected_modules.len());
    for module in &selected_modules {
        println!("  - {}", module);
    }
    
    Ok(selected_modules)
}
