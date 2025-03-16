use std::env;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use crate::config::SEPARATOR_LINE;
use crate::model::{App, CommandStatus};

impl App {
    pub fn execute_command(&mut self) {
        let command = self.input.clone();
        let command = command.trim();
        if command.is_empty() {
            return;
        }

        // Add command to history (only if it's not empty and not the same as the last command)
        if !command.is_empty() && (self.command_history.is_empty() || self.command_history.last().unwrap() != command) {
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
        if command.starts_with("cd ") {
            let path = command.trim_start_matches("cd ").trim();
            let success = self.change_directory(path);
            
            // Update command status
            if success {
                self.command_status[command_index] = CommandStatus::Success;
                command_output.push(format!("Changed directory to: {}", self.current_dir.display()));
            } else {
                self.command_status[command_index] = CommandStatus::Failure;
                command_output.push("Error changing directory".to_string());
            }
            
            // Add a separator after the command output
            self.output.push(SEPARATOR_LINE.repeat(40));
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
            
            // Add a separator after the command output
            self.output.push(SEPARATOR_LINE.repeat(40));
        }

        // Store the command and its output for context
        self.last_terminal_context = Some((command.to_string(), command_output));

        self.input.clear();
        self.cursor_position = 0;
        
        // Set scroll to 0 to always show the most recent output at the bottom
        // In the Paragraph widget, scroll is applied from the bottom when using negative values
        self.terminal_scroll = 0;
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
        let args = &parts[1..];

        // Execute the command
        match Command::new(program)
            .args(args)
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