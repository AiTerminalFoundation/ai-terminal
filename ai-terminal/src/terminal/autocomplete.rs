use crate::config::{COMMON_COMMANDS, PATH_COMMANDS};
use crate::model::App;
use std::fs;

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
