use crate::model::{App, CommandStatus};
use std::env;
use std::path::PathBuf;
use std::process::{Command, Stdio};

impl App {
    pub fn execute_command(&mut self) {
        let command = self.input.clone();
        let command = command.trim();
        if command.is_empty() {
            return;
        }

        // Add command to history (only if it's not empty and not the same as the last command)
        if !command.is_empty()
            && (self.command_history.is_empty() || self.command_history.last().unwrap() != command)
        {
            // Add to history
            self.command_history.push(command.to_string());

            // Limit history to 30 commands
            if self.command_history.len() > crate::config::MAX_COMMAND_HISTORY {
                self.command_history.remove(0);
            }
        }

        // Add command to output
        self.output.push(format!("> {}", command));

        // Add a placeholder for command status
        self.command_status.push(CommandStatus::Running);
        let command_index = self.command_status.len() - 1;

        // Store the command for context
        let mut command_output = Vec::new();

        // Handle cd command specially
        if command.starts_with("cd ") || command.eq_ignore_ascii_case("cd") {

            let mut path = "~";
            if !command.eq_ignore_ascii_case("cd") {
                path = command.trim_start_matches("cd ").trim();
            }

            let success = self.change_directory(path);

            // Update command status
            if success {
                self.command_status[command_index] = CommandStatus::Success;
                command_output.push(format!(
                    "Changed directory to: {}",
                    self.current_dir.display()
                ));
            } else {
                self.command_status[command_index] = CommandStatus::Failure;
                command_output.push("Error changing directory".to_string());
            }
        } else if command.eq_ignore_ascii_case("clear") || command.eq_ignore_ascii_case("cls") {
            // handling command to clear terminal output
            self.output.clear();
            self.command_status[command_index] = CommandStatus::Success;
            self.output.push(format!("> {}", command));
        } else {
            // Execute the command
            let (output, success) = self.run_command(command);
            self.output.extend(output.clone());
            command_output = output;

            // Update command status
            if success {
                self.command_status[command_index] = CommandStatus::Success;
            } else {
                self.command_status[command_index] = CommandStatus::Failure;
            }
        }

        // Store the command and its output for context
        self.last_terminal_context = Some((command.to_string(), command_output));

        self.input.clear();
        self.cursor_position = 0;

        // Set scroll to maximum to show the most recent output
        self.terminal_scroll = usize::MAX;
    }

    pub fn run_command(&self, command: &str) -> (Vec<String>, bool) {
        let mut result = Vec::new();
        let mut success = true;

        // Split the command into program and arguments
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() {
            return (result, success);
        }

        let program = parts[0];
        let args: Vec<String> = parts[1..]
            .iter()
            .map(|&arg| {
                if arg == "~" || arg.starts_with("~/") {
                    if let Some(home) = dirs_next::home_dir() {
                        if arg == "~" {
                            home.to_string_lossy().to_string()
                        } else {
                            home.join(arg.trim_start_matches("~/"))
                                .to_string_lossy()
                                .to_string()
                        }
                    } else {
                        arg.to_string()
                    }
                } else {
                    arg.to_string()
                }
            })
            .collect();

        // Execute the command
        match Command::new(program)
            .args(&args)
            .current_dir(&self.current_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
        {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);

                // Add stdout to output
                if !stdout.is_empty() {
                    for line in stdout.lines() {
                        result.push(line.to_string());
                    }
                }

                // Add stderr to output
                if !stderr.is_empty() {
                    for line in stderr.lines() {
                        result.push(format!("Error: {}", line));
                    }
                }

                // Add exit status
                if !output.status.success() {
                    result.push(format!("Command exited with status: {}", output.status));
                    success = false;
                }
            }
            Err(e) => {
                result.push(format!("Failed to execute command: {}", e));
                success = false;
            }
        }

        (result, success)
    }

    pub fn change_directory(&mut self, path: &str) -> bool {
        let new_dir = if path.starts_with('/') {
            // Absolute path
            PathBuf::from(path)
        } else if path == "~" || path.starts_with("~/") {
            // Home directory
            if let Some(home) = dirs_next::home_dir() {
                if path == "~" {
                    home
                } else {
                    home.join(path.trim_start_matches("~/"))
                }
            } else {
                self.output
                    .push("Error: Could not determine home directory".to_string());
                return false;
            }
        } else if path == ".." {
            // Parent directory
            if let Some(parent) = self.current_dir.parent() {
                PathBuf::from(parent)
            } else {
                self.output
                    .push("Error: Already at root directory".to_string());
                return false;
            }
        } else {
            // Relative path
            self.current_dir.join(path)
        };

        // Try to change to the new directory
        match env::set_current_dir(&new_dir) {
            Ok(_) => {
                self.current_dir = new_dir;
                self.output.push(format!(
                    "Changed directory to: {}",
                    self.current_dir.display()
                ));
                true
            }
            Err(e) => {
                self.output.push(format!("Error changing directory: {}", e));
                false
            }
        }
    }
}

// Execute a command and return the output
pub fn execute_command(command: &str, current_dir: &PathBuf) -> (Vec<String>, bool) {
    // Handle built-in commands
    if command.starts_with("cd ") {
        return handle_cd_command(command, current_dir);
    }

    // Expand tilde in command arguments
    let expanded_command = command.split_whitespace()
        .enumerate()
        .map(|(i, arg)| {
            if i == 0 {
                arg.to_string()
            } else if arg == "~" || arg.starts_with("~/") {
                if let Some(home) = dirs_next::home_dir() {
                    if arg == "~" {
                        home.to_string_lossy().to_string()
                    } else {
                        home.join(arg.trim_start_matches("~/"))
                            .to_string_lossy()
                            .to_string()
                    }
                } else {
                    arg.to_string()
                }
            } else {
                arg.to_string()
            }
        })
        .collect::<Vec<String>>()
        .join(" ");

    // Execute external command
    let output = Command::new("sh")
        .arg("-c")
        .arg(expanded_command)
        .current_dir(current_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();

    match output {
        Ok(output) => {
            let success = output.status.success();
            
            // Convert stdout to string
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            
            // Split output into lines
            let mut result = Vec::new();
            
            // Add stdout lines
            if !stdout.is_empty() {
                result.extend(stdout.lines().map(|s| s.to_string()));
            }
            
            // Add stderr lines
            if !stderr.is_empty() {
                result.extend(stderr.lines().map(|s| s.to_string()));
            }
            
            (result, success)
        }
        Err(e) => {
            (vec![format!("Error executing command: {}", e)], false)
        }
    }
}

// Handle the cd command
fn handle_cd_command(command: &str, current_dir: &PathBuf) -> (Vec<String>, bool) {
    let parts: Vec<&str> = command.split_whitespace().collect();
    if parts.len() < 2 {
        return (vec!["cd: missing argument".to_string()], false);
    }

    let path = parts[1];
    let new_dir = if path.starts_with('/') {
        PathBuf::from(path)
    } else {
        let mut dir = current_dir.clone();
        dir.push(path);
        dir
    };

    if new_dir.exists() && new_dir.is_dir() {
        std::env::set_current_dir(&new_dir).unwrap_or_else(|_| {
            // If we can't set the current directory, return an error
            return;
        });
        (vec![format!("Changed directory to {}", new_dir.display())], true)
    } else {
        (vec![format!("cd: {}: No such directory", path)], false)
    }
}

pub fn navigate_history_up(app: &mut App) {
    if let Some(current_index) = app.command_history_index {
        if current_index > 0 {
            app.command_history_index = Some(current_index - 1);
            if let Some(command) = app.command_history.get(current_index - 1) {
                app.input = command.clone();
            }
        }
    } else if !app.command_history.is_empty() {
        app.command_history_index = Some(app.command_history.len() - 1);
        if let Some(command) = app.command_history.last() {
            app.input = command.clone();
        }
    }
}

pub fn navigate_history_down(app: &mut App) {
    if let Some(current_index) = app.command_history_index {
        if current_index < app.command_history.len() - 1 {
            app.command_history_index = Some(current_index + 1);
            if let Some(command) = app.command_history.get(current_index + 1) {
                app.input = command.clone();
            }
        } else {
            app.command_history_index = None;
            app.input.clear();
        }
    }
}
