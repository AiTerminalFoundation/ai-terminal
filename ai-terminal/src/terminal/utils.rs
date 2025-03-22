use regex::Regex;
use std::path::{Path, PathBuf};
use std::fs;
use std::process::Command;

// Extract commands from a response string
pub fn extract_commands(text: &str) -> String {
    println!("Input text: '{}'", text);  // Debug print
    
    // First try to match complete code blocks
    let re = Regex::new(r"```\s*(?:\w+)?\s*(.+?)```").unwrap();
    if let Some(captures) = re.captures(text) {
        if let Some(command_match) = captures.get(1) {
            let result = command_match.as_str().trim().to_string();
            println!("Matched (complete): '{}'", result);  // Debug print
            return result;
        }
    }

    // If no complete block found, try to match just after opening ```
    let re_open = Regex::new(r"```\s*(.+)").unwrap();
    if let Some(captures) = re_open.captures(text) {
        if let Some(command_match) = captures.get(1) {
            let result = command_match.as_str().trim().to_string();
            println!("Matched (open): '{}'", result);  // Debug print
            return result;
        }
    }
    
    println!("No match found");  // Debug print
    String::new()
}

// Checks if the given directory is a git repository
// Returns (is_git_repo, branch_name)
pub fn get_git_info(dir: &Path) -> (bool, Option<String>) {
    // Check if .git directory exists
    let git_dir = dir.join(".git");
    if !git_dir.exists() || !git_dir.is_dir() {
        return (false, None);
    }
    
    // Get the current branch
    let output = Command::new("git")
        .arg("rev-parse")
        .arg("--abbrev-ref")
        .arg("HEAD")
        .current_dir(dir)
        .output();
    
    match output {
        Ok(output) if output.status.success() => {
            let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
            (true, Some(branch))
        },
        _ => (true, None), // It's a git repo but we couldn't get the branch
    }
}