use crate::config::{COMMON_COMMANDS, PATH_COMMANDS};
use crate::model::App;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

impl App {
    // Get autocomplete suggestions based on current input
    pub fn get_autocomplete_suggestions(&mut self) -> Vec<String> {
        let input = self.input.clone();
        let mut suggestions = Vec::new();

        // If input is empty, return empty suggestions
        if input.is_empty() {
            return suggestions;
        }

        // Split input into parts
        let parts: Vec<&str> = input.split_whitespace().collect();

        // Check if we're trying to autocomplete a path (for cd, ls, etc.)
        if parts.len() >= 2 && PATH_COMMANDS.contains(&parts[0]) {
            let command = parts[0];
            let path_part = if parts.len() > 1 {
                // Get the last part which is being typed
                parts.last().unwrap()
            } else {
                ""
            };

            // For cd command, only suggest directories
            if command == "cd" {
                suggestions = self
                    .get_path_suggestions(path_part)
                    .into_iter()
                    .filter(|s| s.ends_with('/'))
                    .collect();
            } else {
                // For other commands, suggest both files and directories
                suggestions = self.get_path_suggestions(path_part);
            }

            // Format suggestions to include the command and any intermediate arguments
            if parts.len() > 2 {
                let prefix = parts[..parts.len() - 1].join(" ") + " ";
                suggestions = suggestions
                    .into_iter()
                    .map(|s| format!("{}{}", prefix, s))
                    .collect();
            } else if parts.len() == 2 {
                let prefix = format!("{} ", command);
                suggestions = suggestions
                    .into_iter()
                    .map(|s| format!("{}{}", prefix, s))
                    .collect();
            }
        } else if !input.contains(' ') {
            // We're at the beginning of a command (no space yet)
            // Common Unix commands for autocompletion
            for cmd in COMMON_COMMANDS.iter() {
                if cmd.starts_with(&input) {
                    suggestions.push(cmd.to_string());
                }
            }

            // Also add commands from history
            for cmd in &self.command_history {
                let cmd_part = cmd.split_whitespace().next().unwrap_or("");
                if cmd_part.starts_with(&input) && !suggestions.contains(&cmd_part.to_string()) {
                    suggestions.push(cmd_part.to_string());
                }
            }
        }

        suggestions.sort();
        suggestions
    }

    // Get path suggestions for cd command
    pub fn get_path_suggestions(&self, path_part: &str) -> Vec<String> {
        let mut suggestions = Vec::new();

        // Determine the directory to search in and the prefix to match
        let (search_dir, prefix) = if path_part.is_empty() {
            // If no path specified, suggest directories in current directory
            (self.current_dir.clone(), "".to_string())
        } else if path_part == "~" {
            // Suggest home directory
            if let Some(home) = dirs_next::home_dir() {
                (home, "~".to_string())
            } else {
                return suggestions;
            }
        } else if path_part.starts_with("~/") {
            // Suggest in home directory with subdirectory
            if let Some(home) = dirs_next::home_dir() {
                let subdir = path_part.trim_start_matches("~/");
                let last_slash = subdir.rfind('/').unwrap_or(0);
                let (dir_part, _file_prefix) = if last_slash == 0 {
                    (subdir, "")
                } else {
                    subdir.split_at(last_slash)
                };

                let search_path = if dir_part.is_empty() {
                    home.clone()
                } else {
                    home.join(dir_part)
                };

                (
                    search_path,
                    format!(
                        "~/{}{}",
                        dir_part,
                        if !dir_part.is_empty() && !dir_part.ends_with('/') {
                            "/"
                        } else {
                            ""
                        }
                    ),
                )
            } else {
                return suggestions;
            }
        } else if path_part.starts_with('/') {
            // Absolute path
            let last_slash = path_part.rfind('/').unwrap_or(0);
            let (dir_part, _file_prefix) = path_part.split_at(last_slash + 1);

            (std::path::PathBuf::from(dir_part), dir_part.to_string())
        } else {
            // Relative path
            let last_slash = path_part.rfind('/').unwrap_or(0);
            let (dir_part, _file_prefix) = if last_slash == 0 {
                ("", path_part)
            } else {
                path_part.split_at(last_slash + 1)
            };

            let search_path = if dir_part.is_empty() {
                self.current_dir.clone()
            } else {
                self.current_dir.join(dir_part)
            };

            (search_path, dir_part.to_string())
        };

        // Get the part after the last slash to match against
        let match_prefix = if let Some(last_slash) = path_part.rfind('/') {
            &path_part[last_slash + 1..]
        } else {
            path_part
        };

        // Read the directory and find matching entries
        if let Ok(entries) = fs::read_dir(&search_dir) {
            for entry in entries.flatten() {
                if let Ok(file_name) = entry.file_name().into_string() {
                    // Check if the file name starts with our prefix
                    if file_name.starts_with(match_prefix) {
                        if let Ok(file_type) = entry.file_type() {
                            let suggestion = if file_type.is_dir() {
                                // Add trailing slash for directories
                                format!("{}{}/", prefix, file_name)
                            } else {
                                // Regular file
                                format!("{}{}", prefix, file_name)
                            };
                            suggestions.push(suggestion);
                        }
                    }
                }
            }
        }

        // Add special directories if they match
        if ".".starts_with(match_prefix) {
            suggestions.push(format!("{}./", prefix));
        }
        if "..".starts_with(match_prefix) {
            suggestions.push(format!("{}../", prefix));
        }

        suggestions
    }

    // Apply autocomplete suggestion
    pub fn apply_autocomplete(&mut self) {
        if let Some(index) = self.autocomplete_index {
            if index < self.autocomplete_suggestions.len() {
                let suggestion = &self.autocomplete_suggestions[index];

                // Replace the input with the suggestion
                self.input = suggestion.clone();

                // Move cursor to end of input
                self.cursor_position = self.input.len();

                // Clear suggestions after applying
                self.autocomplete_suggestions.clear();
                self.autocomplete_index = None;
            }
        } else if !self.autocomplete_suggestions.is_empty() {
            // If we have suggestions but no index, set index to 0
            self.autocomplete_index = Some(0);
        }
    }

    // Cycle through autocomplete suggestions
    pub fn cycle_autocomplete(&mut self, forward: bool) {
        if self.autocomplete_suggestions.is_empty() {
            // Generate suggestions if we don't have any
            self.autocomplete_suggestions = self.get_autocomplete_suggestions();
            if !self.autocomplete_suggestions.is_empty() {
                self.autocomplete_index = Some(0);
            }
        } else if let Some(index) = self.autocomplete_index {
            // Cycle through existing suggestions
            if forward {
                self.autocomplete_index = Some((index + 1) % self.autocomplete_suggestions.len());
            } else {
                self.autocomplete_index = Some(if index == 0 {
                    self.autocomplete_suggestions.len() - 1
                } else {
                    index - 1
                });
            }
        }

        // Apply the current suggestion
        self.apply_autocomplete();
    }
}

// Generate autocomplete suggestions for a command
pub fn generate_suggestions(input: &str, current_dir: &PathBuf) -> Vec<String> {
    if input.is_empty() {
        return Vec::new();
    }

    let parts: Vec<&str> = input.split_whitespace().collect();
    let command = parts[0];
    
    // If we're typing a command (first word)
    if parts.len() == 1 {
        return COMMON_COMMANDS
            .iter()
            .filter(|&cmd| cmd.starts_with(command))
            .map(|&cmd| cmd.to_string())
            .collect();
    }
    
    // If we're typing a path for a command that accepts paths
    if PATH_COMMANDS.contains(&command) && parts.len() > 1 {
        let path_part = parts.last().unwrap();
        return complete_path(path_part, current_dir);
    }
    
    Vec::new()
}

// Complete a path based on the current directory
fn complete_path(path_part: &str, current_dir: &PathBuf) -> Vec<String> {
    let mut suggestions = Vec::new();
    
    // Determine the directory to search in
    let (search_dir, prefix) = if path_part.contains('/') {
        let path = PathBuf::from(path_part);
        let default_path = PathBuf::from(".");
        let parent = path.parent().unwrap_or(&default_path);
        let prefix = path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
        
        let search_path = if path_part.starts_with('/') {
            parent.to_path_buf()
        } else {
            let mut dir = current_dir.clone();
            dir.push(parent);
            dir
        };
        
        (search_path, prefix)
    } else {
        (current_dir.clone(), path_part.to_string())
    };
    
    // Read the directory entries
    if let Ok(entries) = fs::read_dir(&search_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(&prefix) {
                // Add a trailing slash for directories
                let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
                let suggestion = if is_dir {
                    format!("{}/", name)
                } else {
                    name
                };
                
                suggestions.push(suggestion);
            }
        }
    }
    
    suggestions
}

// Get autocomplete suggestions for a command
pub fn get_suggestions(input: &str, current_dir: &PathBuf) -> Vec<String> {
    // If input is empty, return empty suggestions
    if input.is_empty() {
        return Vec::new();
    }

    // Handle cd command specially
    if input.starts_with("cd ") {
        let parts: Vec<&str> = input.split_whitespace().collect();
        if parts.len() >= 2 {
            let path_part = parts[1];
            
            // If the path is just a partial directory name without slashes,
            // we need to find matching directories in the current directory
            if !path_part.contains('/') {
                let mut suggestions = Vec::new();
                
                if let Ok(entries) = fs::read_dir(current_dir) {
                    for entry in entries.filter_map(Result::ok) {
                        if let Ok(file_type) = entry.file_type() {
                            if file_type.is_dir() {
                                let name = entry.file_name().to_string_lossy().to_string();
                                if name.starts_with(path_part) {
                                    suggestions.push(format!("cd {}/", name));
                                }
                            }
                        }
                    }
                }
                
                // Add special directories if they match
                if ".".starts_with(path_part) {
                    suggestions.push("cd ./".to_string());
                }
                if "..".starts_with(path_part) {
                    suggestions.push("cd ../".to_string());
                }
                
                // Sort suggestions
                suggestions.sort();
                return suggestions;
            }
        }
        
        return get_path_suggestions(input, current_dir);
    }

    // Handle other common commands
    if input.contains(' ') {
        // If there's a space, we're completing an argument
        let parts: Vec<&str> = input.split_whitespace().collect();
        let command = parts[0];
        
        match command {
            "ls" | "cat" | "vim" | "nano" | "rm" | "cp" | "mv" | "touch" | "mkdir" => {
                // These commands take file/directory arguments
                return get_path_suggestions(input, current_dir);
            }
            "git" => {
                // Git command completion
                if parts.len() == 2 {
                    let git_commands = vec![
                        "add", "commit", "push", "pull", "checkout", "branch", "status",
                        "log", "diff", "merge", "rebase", "reset", "fetch", "clone",
                    ];
                    return filter_suggestions(git_commands, parts[1]);
                }
            }
            _ => {}
        }
    } else {
        // We're completing the command itself
        let common_commands = vec![
            "ls", "cd", "pwd", "cat", "grep", "find", "git", "vim", "nano",
            "mkdir", "rm", "cp", "mv", "touch", "echo", "clear", "history",
            "ps", "top", "ssh", "scp", "curl", "wget", "tar", "zip", "unzip",
        ];
        
        return filter_suggestions(common_commands, input);
    }

    Vec::new()
}

// Get path suggestions for commands like cd, ls, etc.
fn get_path_suggestions(input: &str, current_dir: &PathBuf) -> Vec<String> {
    let parts: Vec<&str> = input.split_whitespace().collect();
    if parts.len() < 2 {
        return Vec::new();
    }
    
    // Get the command and partial path
    let command = parts[0];
    let mut partial_path = parts[1..].join(" ");
    
    // Handle tilde expansion
    if partial_path.starts_with("~/") {
        if let Some(home) = dirs_next::home_dir() {
            let home_str = home.to_string_lossy();
            partial_path = partial_path.replace("~/", &format!("{}/", home_str));
        }
    } else if partial_path == "~" {
        if let Some(home) = dirs_next::home_dir() {
            partial_path = home.to_string_lossy().to_string();
        }
    }
    
    // Determine the directory to search in and the prefix to match
    let (search_dir, prefix) = if partial_path.contains('/') {
        let rindex = partial_path.rfind('/').unwrap();
        let (dir_part, file_part) = partial_path.split_at(rindex + 1);
        
        let search_path = if dir_part.starts_with('/') {
            // Absolute path
            PathBuf::from(dir_part)
        } else {
            // Relative path
            current_dir.join(dir_part)
        };
        
        (search_path, file_part.to_string())
    } else {
        // No directory specified, search in current directory
        (current_dir.clone(), partial_path.clone())  // Clone here to avoid the move
    };
    
    // Get matching entries
    let mut suggestions = Vec::new();
    
    if let Ok(entries) = fs::read_dir(&search_dir) {
        for entry in entries.filter_map(Result::ok) {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            
            if name_str.starts_with(&prefix) {
                let path = entry.path();
                let is_dir = path.is_dir();
                
                // Format the suggestion
                let mut suggestion = format!("{} ", command);
                
                // Build the path part of the suggestion
                let path_suggestion = if partial_path.contains('/') {
                    let dir_part = &partial_path[..partial_path.rfind('/').unwrap() + 1];
                    format!("{}{}", dir_part, name_str)
                } else {
                    name_str.to_string()
                };
                
                // Add trailing slash for directories
                if is_dir {
                    suggestion.push_str(&format!("{}/", path_suggestion));
                } else {
                    suggestion.push_str(&path_suggestion);
                }
                
                suggestions.push(suggestion);
            }
        }
    }
    
    // If we're looking for directories that start with a specific prefix,
    // add special directories if they match
    if prefix.is_empty() || ".".starts_with(&prefix) {
        let mut suggestion = format!("{} ", command);
        if partial_path.contains('/') {
            let dir_part = &partial_path[..partial_path.rfind('/').unwrap() + 1];
            suggestion.push_str(&format!("{}./", dir_part));
        } else {
            suggestion.push_str("./");
        }
        suggestions.push(suggestion);
    }
    
    if prefix.is_empty() || "..".starts_with(&prefix) {
        let mut suggestion = format!("{} ", command);
        if partial_path.contains('/') {
            let dir_part = &partial_path[..partial_path.rfind('/').unwrap() + 1];
            suggestion.push_str(&format!("{}../", dir_part));
        } else {
            suggestion.push_str("../");
        }
        suggestions.push(suggestion);
    }
    
    // Sort suggestions alphabetically
    suggestions.sort();
    
    suggestions
}

// Filter suggestions based on prefix
fn filter_suggestions(options: Vec<&str>, prefix: &str) -> Vec<String> {
    options
        .into_iter()
        .filter(|opt| opt.starts_with(prefix))
        .map(|opt| opt.to_string())
        .collect()
}

// Get executable commands in PATH
pub fn get_executable_commands() -> Vec<String> {
    let mut commands = Vec::new();
    
    // Try to get PATH environment variable
    if let Ok(path) = std::env::var("PATH") {
        // Split PATH by colon
        for path_dir in path.split(':') {
            if let Ok(entries) = fs::read_dir(path_dir) {
                for entry in entries.filter_map(Result::ok) {
                    if let Ok(metadata) = entry.metadata() {
                        // Check if the entry is a file and is executable
                        if metadata.is_file() && is_executable(&entry.path()) {
                            if let Some(name) = entry.file_name().to_str() {
                                commands.push(name.to_string());
                            }
                        }
                    }
                }
            }
        }
    }
    
    commands
}

// Check if a file is executable
fn is_executable(path: &Path) -> bool {
    if let Ok(output) = Command::new("test")
        .arg("-x")
        .arg(path)
        .output()
    {
        output.status.success()
    } else {
        false
    }
}
