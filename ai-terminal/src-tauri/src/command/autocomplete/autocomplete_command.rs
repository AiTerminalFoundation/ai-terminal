use crate::command::constants::COMMON_COMMANDS;
use crate::command::types::command_manager::CommandManager;
use crate::utils::file_system_utils::split_path_prefix;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{command, State};

#[command]
pub fn autocomplete(
    input: String,
    session_id: String,
    command_manager: State<'_, CommandManager>,
) -> Result<Vec<String>, String> {
    let states = command_manager.commands.lock().map_err(|e| e.to_string())?;
    let key = session_id;

    let current_dir = if let Some(state) = states.get(&key) {
        &state.current_dir
    } else {
        return Err("Could not determine current directory".to_string());
    };

    let input_parts: Vec<&str> = input.split_whitespace().collect();

    // Autocomplete commands if it's the first word
    if input_parts.len() <= 1 && input_parts.first() != Some(&"cd") {
        let input_prefix = input_parts.first().unwrap_or(&"");

        let matches: Vec<String> = autocomplete_base_command(input_prefix);

        if !matches.is_empty() {
            return Ok(matches);
        }
    }

    // If we have a cd command, autocomplete directories
    let path_to_complete = if input_parts.first() == Some(&"cd") {
        if input_parts.len() > 1 {
            // Handle cd command with argument
            input_parts.last().unwrap_or(&"")
        } else {
            // Handle cd with no argument - show all directories in current folder
            ""
        }
    } else if !input_parts.is_empty() && input_parts[0].contains('/') {
        // Handle path directly
        input_parts[0]
    } else if input_parts.len() > 1 {
        // Handle second argument as path for any command
        input_parts.last().unwrap_or(&"")
    } else {
        // Default to empty string if no path found
        ""
    };

    // If input starts with cd, or we have a potential path to complete
    if input_parts.first() == Some(&"cd") || !path_to_complete.is_empty() {
        let (dir_to_search, prefix) = split_path_prefix(path_to_complete);

        // Create a Path for the directory to search
        let search_path = if dir_to_search.starts_with('/') || dir_to_search.starts_with('~') {
            if dir_to_search.starts_with('~') {
                let home = dirs::home_dir().ok_or("Could not determine home directory")?;
                let without_tilde = dir_to_search.trim_start_matches('~');
                let rel_path = without_tilde.trim_start_matches('/');
                if rel_path.is_empty() {
                    home
                } else {
                    home.join(rel_path)
                }
            } else {
                PathBuf::from(dir_to_search)
            }
        } else {
            Path::new(current_dir).join(dir_to_search)
        };

        if search_path.exists() && search_path.is_dir() {
            let entries = fs::read_dir(search_path).map_err(|e| e.to_string())?;

            let mut matches = Vec::new();
            for entry in entries.flatten() {
                let file_name = entry.file_name();
                let file_name_str = file_name.to_string_lossy();

                // Include all entries for empty prefix, otherwise filter by prefix (case-insensitive)
                if prefix.is_empty()
                    || file_name_str
                        .to_lowercase()
                        .starts_with(&prefix.to_lowercase())
                {
                    let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);

                    // For the 'cd' command, only show directories
                    if !input_parts.is_empty() && input_parts[0] == "cd" && !is_dir {
                        continue;
                    }

                    // Add trailing slash for directories
                    let suggestion = if is_dir {
                        format!("{}/", file_name_str)
                    } else {
                        file_name_str.to_string()
                    };

                    // Construct the full path suggestion for the command
                    let base_path = if dir_to_search.is_empty() {
                        "".to_string()
                    } else {
                        format!("{}/", dir_to_search.trim_end_matches('/'))
                    };

                    matches.push(format!("{}{}", base_path, suggestion));
                }
            }

            if !matches.is_empty() {
                // Sort matches alphabetically, case-insensitive
                matches.sort_by_key(|a| a.to_lowercase());
                return Ok(matches);
            }
        }
    }

    Ok(Vec::new())
}

fn autocomplete_base_command(input_prefix: &str) -> Vec<String> {
    COMMON_COMMANDS
        .iter()
        .filter(|&command| command.starts_with(input_prefix))
        .map(|&command| command.to_string())
        .collect()
}
