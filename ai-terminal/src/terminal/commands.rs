use crate::model::{App, CommandStatus};
use std::env;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::io::{BufRead, BufReader, Write};
use iced::Command as IcedCommand;
use crate::app::Message;
use crate::ui::components::scrollable_container;
use std::sync::Arc;
use std::sync::Mutex;

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
        if command.starts_with("cd ") {
            let path = command.trim_start_matches("cd ").trim();
            let success = self.change_directory(path);

            // Update command status
            if success {
                self.command_status[command_index] = CommandStatus::Success;
                command_output.push(format!(
                    "Changed directory to: {}",
                    self.current_dir.display()
                ));
                self.output.push(command_output.last().unwrap().clone());
            } else {
                self.command_status[command_index] = CommandStatus::Failure;
                command_output.push("Error changing directory".to_string());
                self.output.push(command_output.last().unwrap().clone());
            }
        } else if command.eq_ignore_ascii_case("clear") || command.eq_ignore_ascii_case("cls") {
            // handling command to clear terminal output
            self.output.clear();
            self.command_status[command_index] = CommandStatus::Success;
            self.output.push(format!("> {}", command));
        } else {
            // Check if this is a sudo command or other long-running command
            let parts: Vec<&str> = command.split_whitespace().collect();
            let is_long_running = !parts.is_empty() && (
                parts[0] == "sudo" || parts[0] == "make" || parts[0] == "cargo" 
                || parts[0] == "npm" || parts[0] == "yarn" || parts[0] == "apt" 
                || parts[0] == "apt-get" || parts[0] == "yum" || parts[0] == "brew"
            );
            
            if is_long_running {
                // For long-running commands, spawn the process and handle output asynchronously
                self.spawn_streaming_command(command.to_string(), command_index);
                return;
            } else {
                // Execute quick commands synchronously
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
        }

        // Store the command and its output for context
        self.last_terminal_context = Some((command.to_string(), command_output));

        self.input.clear();
        self.cursor_position = 0;

        // Set scroll to maximum to show the most recent output
        self.terminal_scroll = usize::MAX;
    }

    pub fn run_command(&mut self, command: &str) -> (Vec<String>, bool) {
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

        // Check if this is likely a long-running command
        let is_long_running = program == "sudo" || program == "make" || program == "cargo" 
            || program == "npm" || program == "yarn" || program == "apt" 
            || program == "apt-get" || program == "yum" || program == "brew";
        
        if is_long_running {
            // Use streaming execution for long-running commands
            return self.run_streaming_command(program, &args);
        }

        // Execute the command synchronously (for quick commands)
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

    // New method to handle streaming command execution
    fn run_streaming_command(&mut self, program: &str, args: &[String]) -> (Vec<String>, bool) {
        let mut result = Vec::new();
        
        // Show a message indicating the command is running
        result.push(format!("Running command: {} {}", program, args.join(" ")));
        self.output.push(result.last().unwrap().clone());
        
        // Create a command that will be executed with streaming IO
        let mut cmd = Command::new(program);
        cmd.args(args)
           .current_dir(&self.current_dir)
           .stdout(Stdio::piped())
           .stderr(Stdio::piped())
           .stdin(Stdio::inherit()); // Allow stdin to be passed through
        
        match cmd.spawn() {
            Ok(mut child) => {
                let stdout = child.stdout.take().expect("Failed to open stdout");
                let stderr = child.stderr.take().expect("Failed to open stderr");
                
                // Use BufReader to read lines from stdout and stderr
                let stdout_reader = std::io::BufReader::new(stdout);
                let stderr_reader = std::io::BufReader::new(stderr);
                
                // Handle stdout in a separate thread
                let stdout_lines = stdout_reader.lines();
                for line in stdout_lines.flatten() {
                    result.push(line.clone());
                    self.output.push(line);
                    
                    // Signal to the UI to refresh
                    crate::app::Message::ScrollToBottom;
                }
                
                // Handle stderr in a separate thread
                let stderr_lines = stderr_reader.lines();
                for line in stderr_lines.flatten() {
                    let error_line = format!("Error: {}", line);
                    result.push(error_line.clone());
                    self.output.push(error_line);
                    
                    // Signal to the UI to refresh
                    crate::app::Message::ScrollToBottom;
                }
                
                // Wait for the command to complete
                match child.wait() {
                    Ok(status) => {
                        let success = status.success();
                        if !success {
                            let status_msg = format!("Command exited with status: {}", status);
                            result.push(status_msg.clone());
                            self.output.push(status_msg);
                        }
                        (result, success)
                    }
                    Err(e) => {
                        let error_msg = format!("Error waiting for command to complete: {}", e);
                        result.push(error_msg.clone());
                        self.output.push(error_msg);
                        (result, false)
                    }
                }
            }
            Err(e) => {
                let error_msg = format!("Failed to execute command: {}", e);
                result.push(error_msg.clone());
                self.output.push(error_msg);
                (result, false)
            }
        }
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

    // New method to spawn a command with streaming output
    fn spawn_streaming_command(&mut self, command: String, command_index: usize) {
        let (tx, rx) = mpsc::channel();
        
        let command_clone = command.clone();
        let current_dir = self.current_dir.clone();
        
        // Create a channel for user input
        let (input_tx, input_rx) = mpsc::channel::<String>();
        let input_tx_clone = input_tx.clone();
        
        thread::spawn(move || {
            let parts: Vec<&str> = command_clone.split_whitespace().collect();
            
            let mut cmd = if parts[0] == "sudo" {
                let mut cmd = Command::new("sudo");
                cmd.arg("-S"); // Force sudo to read password from stdin
                if parts.len() > 1 {
                    cmd.args(&parts[1..]);
                }
                cmd
            } else {
                let mut cmd = Command::new(parts[0]);
                if parts.len() > 1 {
                    cmd.args(&parts[1..]);
                }
                cmd
            };

            cmd.current_dir(&current_dir)
               .stdout(Stdio::piped())
               .stderr(Stdio::piped())
               .stdin(Stdio::piped());

            match cmd.spawn() {
                Ok(mut child) => {
                    let stdout = child.stdout.take().expect("Failed to open stdout");
                    let stderr = child.stderr.take().expect("Failed to open stderr");
                    let stdin = child.stdin.take().expect("Failed to open stdin");
                    
                    // Thread to handle user input
                    let input_thread = thread::spawn(move || {
                        let mut stdin = stdin;
                        while let Ok(input) = input_rx.recv() {
                            writeln!(stdin, "{}", input).ok();
                            stdin.flush().ok();
                        }
                    });

                    // Thread for stdout
                    let stdout_tx = tx.clone();
                    let stdout_thread = thread::spawn(move || {
                        let reader = BufReader::new(stdout);
                        for line in reader.lines().flatten() {
                            stdout_tx.send(format!("{}", line)).ok();  // Remove "Output: " prefix
                        }
                    });

                    // Thread for stderr
                    let stderr_tx = tx.clone();
                    let stderr_thread = thread::spawn(move || {
                        let reader = BufReader::new(stderr);
                        for line in reader.lines().flatten() {
                            // Don't prefix error messages unless they're actual errors
                            if line.to_lowercase().contains("error") {
                                stderr_tx.send(format!("Error: {}", line)).ok();
                            } else {
                                stderr_tx.send(line).ok();
                            }
                        }
                    });

                    // Wait for the command to finish
                    let status = child.wait().expect("Command wasn't running");
                    
                    // Wait for threads to finish
                    stdout_thread.join().ok();
                    stderr_thread.join().ok();
                    input_thread.join().ok();
                    
                    tx.send(format!("__COMMAND_COMPLETE__{}", status.success())).ok();
                }
                Err(e) => {
                    tx.send(format!("Failed to execute command: {}", e)).ok();
                    tx.send("__COMMAND_COMPLETE__false".to_string()).ok();
                }
            }
        });
        
        self.command_receiver = Some((
            Arc::new(Mutex::new(rx)),
            command_index,
            command,
            Vec::new(),
            input_tx_clone
        ));
    }
    
    // New method to poll for command output
    pub fn poll_command_output(&mut self) -> Option<IcedCommand<Message>> {
        if let Some((rx, command_index, command, output_lines, _input_tx)) = &self.command_receiver {
            // Try to receive a message without taking ownership
            let result = {
                let rx_lock = rx.lock().unwrap();
                rx_lock.try_recv()
            };
            
            match result {
                Ok(line) => {
                    // Check for password prompts
                    if line.contains("[sudo] password for") || line.contains("Password:") {
                        self.password_mode = true;
                        // Don't add the password prompt to output to avoid duplicates
                        return Some(scrollable_container::scroll_to_bottom());
                    }
                    
                    // We got a line, process it
                    if line.starts_with("__COMMAND_COMPLETE__") {
                        // Command is done
                        let success = line.strip_prefix("__COMMAND_COMPLETE__").unwrap() == "true";
                        if *command_index < self.command_status.len() {
                            self.command_status[*command_index] = if success {
                                CommandStatus::Success
                            } else {
                                CommandStatus::Failure
                            };
                        }
                        
                        // Store context and clean up
                        self.last_terminal_context = Some((command.clone(), output_lines.clone()));
                        self.password_mode = false;
                        self.command_receiver = None;
                        
                        return Some(scrollable_container::scroll_to_bottom());
                    } else {
                        // Regular output, add to terminal
                        self.output.push(line.clone());
                        
                        // Update our stored output lines
                        if let Some((_, _, _, lines, _)) = &mut self.command_receiver {
                            lines.push(line);
                        }
                        
                        return Some(scrollable_container::scroll_to_bottom());
                    }
                }
                Err(mpsc::TryRecvError::Empty) => {
                    // No data available right now
                    return None;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    // Channel closed unexpectedly
                    if *command_index < self.command_status.len() {
                        self.command_status[*command_index] = CommandStatus::Failure;
                    }
                    
                    self.output.push("Error: Command execution terminated unexpectedly".to_string());
                    self.command_receiver = None;
                    self.password_mode = false;
                    
                    return Some(scrollable_container::scroll_to_bottom());
                }
            }
        }
        
        None
    }

    // Add this method to handle sending input to the command
    pub fn send_input(&mut self, input: String) {
        if let Some((_, _, _, _, input_tx)) = &self.command_receiver {
            if input_tx.send(input).is_ok() {
                self.output.push("*****".to_string());
                self.password_mode = false;  // Disable password mode after sending
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
