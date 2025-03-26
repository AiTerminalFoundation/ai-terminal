use crate::model::{App, CommandStatus};
use std::env;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::io::{BufRead, BufReader, Write};
use iced::Command as IcedCommand;
use crate::ui::messages::Message;
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
            // Use streaming for all commands except for built-in commands
            // that we've already handled (cd, clear)
            self.spawn_streaming_command(command.to_string(), command_index);
            return;
        }

        // Store the command and its output for context
        self.last_terminal_context = Some((command.to_string(), command_output));

        self.input.clear();
        self.cursor_position = 0;

        // Set scroll to maximum to show the most recent output
        self.terminal_scroll = usize::MAX;
    }

    // New method to handle streaming command execution
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
                
                // Check if this is a git repository and get branch info
                let (is_git_repo, branch) = crate::terminal::utils::get_git_info(&self.current_dir);
                self.is_git_repo = is_git_repo;
                self.git_branch = branch;
                
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
        
        // Send an initial output to force display refresh
        // This line helps ensure the UI updates even if command takes time to produce output
        tx.send("".to_string()).ok();
        
        // Detect if this is a directory listing command
        let is_ls_command = command.trim() == "ls" || command.trim().starts_with("ls ");
        // Increase buffer size to handle large directories (especially for root)
        let buffer_size = if is_ls_command { 2000 } else { 1 };
        
        // Check if this is a sudo command, but don't immediately enable password mode
        thread::spawn(move || {
            let parts: Vec<&str> = command_clone.split_whitespace().collect();
            
            let mut cmd = if parts[0] == "sudo" {
                println!("DEBUG: Creating sudo command");
                let mut cmd = Command::new("sudo");
                
                // First check if sudo needs a password with -n flag
                let needs_password = {
                    let mut check_cmd = Command::new("sudo");
                    check_cmd.arg("-n"); // Non-interactive - will fail if password is needed
                    check_cmd.arg("true");
                    !check_cmd.status().map(|s| s.success()).unwrap_or(false)
                };
                
                println!("DEBUG: Sudo needs password: {}", needs_password);
                
                // If password is needed, send a message to enable password mode
                if needs_password {
                    tx.send("[sudo] password required:".to_string()).ok();
                }
                
                // Configure sudo command
                cmd.arg("-S"); // Read from stdin
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
               
            // For ls commands, ensure we're using the absolute path
            if is_ls_command {
                // Print the working directory for debugging
                let current_path = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
                println!("DEBUG: Working directory for ls: {:?}", current_path);

                // Ensure we are in the correct directory
                if let Err(e) = std::env::set_current_dir(&current_dir) {
                    tx.send(format!("Error setting directory: {}", e)).ok();
                }
            }
               
            match cmd.spawn() {
                Ok(mut child) => {
                    let stdout = child.stdout.take().expect("Failed to open stdout");
                    let stderr = child.stderr.take().expect("Failed to open stderr");
                    let stdin = child.stdin.take().expect("Failed to open stdin");
                    
                    // Thread to handle user input
                    thread::spawn(move || {
                        let mut stdin = stdin;
                        while let Ok(input) = input_rx.recv() {
                            writeln!(stdin, "{}", input).ok();
                            stdin.flush().ok();
                        }
                    });

                    // Thread for stdout - optimize for directory listings
                    let stdout_tx = tx.clone();
                    thread::spawn(move || {
                        handle_stream(BufReader::new(stdout), stdout_tx, is_ls_command, buffer_size);
                    });

                    // Thread for stderr
                    let stderr_tx = tx.clone();
                    thread::spawn(move || {
                        handle_stream(BufReader::new(stderr), stderr_tx, false, 1);
                    });

                    // Wait for the command to finish
                    let status_tx = tx.clone();
                    thread::spawn(move || {
                        // Wait for the process to complete
                        match child.wait() {
                            Ok(status) => {
                                // Send completion message
                                status_tx.send(format!("__COMMAND_COMPLETE__{}", status.success())).ok();
                            },
                            Err(_) => {
                                // Error waiting for process
                                status_tx.send("__COMMAND_COMPLETE__false".to_string()).ok();
                            }
                        }
                    });
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
        // Check if there's an active command
        if let Some((rx, command_index, command, output_lines, _input_tx)) = &self.command_receiver {
            // Try to receive a message without taking ownership
            let result = {
                let rx_lock = rx.lock().unwrap();
                rx_lock.try_recv()
            };
            
            match result {
                Ok(line) => {
                    // Add debug print
                    println!("DEBUG: Received line from command: '{}'", line);
                    
                    // Check for password prompts
                    if line.contains("[sudo] password for") || line.contains("Password:") || 
                       line.contains("password:") || line.contains("password for") || 
                       line.contains("password di") || line.contains("password per") ||
                       line.contains("[sudo]") {
                        println!("DEBUG: Password prompt detected!");
                        self.password_mode = true;
                        self.output.push(line.clone());  // Add the password prompt to output
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
                        
                        // Clone command data before clearing the command_receiver
                        let cmd_clone = command.clone();
                        let output_clone = output_lines.clone();
                        
                        // Store context and clean up
                        self.last_terminal_context = Some((cmd_clone.clone(), output_clone));
                        self.password_mode = false;
                        self.command_receiver = None;
                        
                        // Check if command was a directory listing (ls) and ensure it's all processed at once
                        let is_directory_listing = cmd_clone.trim() == "ls" || cmd_clone.trim().starts_with("ls ");
                        
                        // For directory listings, wait a brief moment to collect all output before refreshing UI
                        if is_directory_listing {
                            std::thread::sleep(std::time::Duration::from_millis(50));
                        }
                        
                        // Force UI update
                        return Some(scrollable_container::scroll_to_bottom());
                    } else if !line.is_empty() {
                        // Regular output, add to terminal
                        self.output.push(line.clone());
                        
                        // Update our stored output lines
                        if let Some((_, _, _, lines, _)) = &mut self.command_receiver {
                            lines.push(line);
                        }
                        
                        // Force UI update - ensure the display refreshes with every output line
                        return Some(scrollable_container::scroll_to_bottom());
                    } else {
                        // Handle empty lines
                        return None;
                    }
                }
                Err(mpsc::TryRecvError::Empty) => None,
                Err(_) => {
                    // Channel closed unexpectedly
                    if *command_index < self.command_status.len() {
                        self.command_status[*command_index] = CommandStatus::Failure;
                    }
                    
                    self.output.push("Error: Command execution terminated unexpectedly".to_string());
                    self.command_receiver = None;
                    self.password_mode = false;
                    
                    // Force UI update
                    return Some(scrollable_container::scroll_to_bottom());
                }
            }
        } else {
            None
        }
    }

    // Add this method to handle sending input to the command
    pub fn send_input(&mut self, input: String) {
        if let Some((_, _, _, _, input_tx)) = &self.command_receiver {
            if input_tx.send(input).is_ok() {
                // Don't echo the actual password, just show asterisks
                self.output.push("*****".to_string());
                self.password_mode = false;  // Disable password mode after sending
            }
        }
    }

    // Add this new method to the App impl
    pub fn terminate_running_command(&mut self) -> Option<IcedCommand<Message>> {
        if let Some((_, command_index, command, output_lines, _)) = &self.command_receiver {
            // Make copies of the values we need
            let command_index = *command_index;
            let command = command.clone();
            let output_lines = output_lines.clone();
            
            // Set command status to indicate interruption
            if command_index < self.command_status.len() {
                self.command_status[command_index] = CommandStatus::Interrupted;
            }
            
            // Add message to output
            self.output.push("^C Command interrupted".to_string());
            
            // Store the command and its partial output for context
            self.last_terminal_context = Some((command, output_lines));
            
            // Clear command receiver and reset password mode
            self.command_receiver = None;
            self.password_mode = false;
            
            // Return command to scroll to bottom
            return Some(scrollable_container::scroll_to_bottom());
        }
        None
    }
}

// Helper function to handle stdout/stderr streams with proper buffering
fn handle_stream(stream: impl BufRead, tx: mpsc::Sender<String>, is_ls_command: bool, buffer_size: usize) {
    let mut buffer = Vec::with_capacity(buffer_size);
    let mut all_output = String::new();
    
    for line in stream.lines() {
        match line {
            Ok(line) => {
                // For ls commands, buffer the output to reduce UI updates
                if is_ls_command && !line.is_empty() {
                    buffer.push(line);
                    
                    if buffer.len() >= buffer_size {
                        // For large directories, join all lines and send at once
                        all_output.push_str(&buffer.join("\n"));
                        buffer.clear();
                    }
                } else if !line.is_empty() {
                    // For other commands, send each line immediately
                    println!("STREAM: [{}] - Forcing UI refresh", line);
                    if tx.send(line).is_err() {
                        break;
                    }
                    // Force UI refresh by using a zero duration sleep
                    std::thread::sleep(std::time::Duration::from_millis(0));
                }
            }
            Err(e) => {
                // Send error information to UI
                if tx.send(format!("Error reading output: {}", e)).is_err() {
                    break;
                }
            }
        }
    }
    
    // Send any remaining buffered content
    if !buffer.is_empty() {
        if all_output.is_empty() {
            // If we haven't sent anything yet, send the buffer directly
            for line in buffer {
                tx.send(line).ok();
            }
        } else {
            // Add remaining buffer to all_output
            all_output.push_str(&buffer.join("\n"));
            tx.send(all_output).ok();
        }
    } else if !all_output.is_empty() {
        // Send any accumulated output
        tx.send(all_output).ok();
    }
}