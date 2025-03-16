use crate::model::App;
use crate::config::{
    THINKING_MESSAGE, HELP_MESSAGES, HELP_COMMANDS, HELP_FEATURES, 
    COMMAND_COPIED_MESSAGE, COMMAND_EXECUTED_MESSAGE, EXTRACTED_COMMANDS_HEADER,
    COMMAND_BUTTONS_HELP, ERROR_FETCHING_MODELS, OLLAMA_NOT_RUNNING,
    OLLAMA_INSTALL_INSTRUCTIONS, NO_MODELS_FOUND, OLLAMA_PULL_INSTRUCTIONS
};
use crate::terminal::utils::extract_commands;
use crate::ollama::api::{ask_ollama, list_ollama_models};

impl App {
    pub fn send_to_ai_assistant(&mut self) {
        if self.ai_input.is_empty() {
            return;
        }

        let input = self.ai_input.clone();
        self.ai_output.push(format!("> {}", input));
        self.ai_input.clear();
        self.ai_cursor_position = 0;
        
        // Clear previous extracted commands
        self.extracted_commands.clear();

        // Handle special commands
        if input.starts_with("/") {
            let parts: Vec<&str> = input.splitn(2, ' ').collect();
            let command = parts[0];
            
            match command {
                "/help" => {
                    self.ai_output.push(HELP_MESSAGES[0].to_string());
                    for cmd in HELP_COMMANDS.iter() {
                        self.ai_output.push(cmd.to_string());
                    }
                    self.ai_output.push("".to_string());
                    self.ai_output.push(HELP_MESSAGES[1].to_string());
                    for feature in HELP_FEATURES.iter() {
                        self.ai_output.push(feature.to_string());
                    }
                },
                "/model" => {
                    if parts.len() > 1 {
                        let model = parts[1].trim();
                        self.ollama_model = model.to_string();
                        self.ai_output.push(format!("Model changed to: {}", model));
                    } else {
                        self.ai_output.push(format!("Current model: {}", self.ollama_model));
                        self.ai_output.push("Usage: /model <model_name>".to_string());
                    }
                },
                "/clear" => {
                    self.ai_output = vec!["Chat history cleared.".to_string()];
                    self.extracted_commands.clear();
                },
                "/models" => {
                    self.ai_output.push("Fetching available models...".to_string());
                    match list_ollama_models() {
                        Ok(models) => {
                            if models.is_empty() {
                                self.ai_output.push(NO_MODELS_FOUND.to_string());
                                self.ai_output.push(OLLAMA_PULL_INSTRUCTIONS.to_string());
                            } else {
                                self.ai_output.push("Available models:".to_string());
                                for model in models {
                                    self.ai_output.push(format!("  - {}", model));
                                }
                            }
                        },
                        Err(e) => {
                            self.ai_output.push(format!("{}{}", ERROR_FETCHING_MODELS, e));
                            self.ai_output.push(OLLAMA_NOT_RUNNING.to_string());
                            self.ai_output.push(OLLAMA_INSTALL_INSTRUCTIONS.to_string());
                        }
                    }
                },
                "/autoexec" => {
                    // Toggle auto-execution of commands
                    self.auto_execute_commands = !self.auto_execute_commands;
                    if self.auto_execute_commands {
                        self.ai_output.push("Auto-execution of commands is now enabled.".to_string());
                    } else {
                        self.ai_output.push("Auto-execution of commands is now disabled.".to_string());
                    }
                },
                _ => {
                    self.ai_output.push(format!("Unknown command: {}", command));
                }
            }
            return;
        }

        // Send message to Ollama
        self.ollama_thinking = true;
        self.ai_output.push(THINKING_MESSAGE.to_string());
        
        // Prepare the message with context if available
        let message_with_context = {
            // Include all terminal output
            let all_terminal_output = self.output.join("\n");
            
            // Include all chat history
            let chat_history = self.ai_output
                .iter()
                .filter(|line| !line.is_empty() && !line.contains(THINKING_MESSAGE))
                .map(|s| s.as_str())
                .collect::<Vec<&str>>()
                .join("\n");
            
            format!(
                "System Info: {}\n\nTerminal History:\n{}\n\nChat History:\n{}\n\nUser query: {}", 
                self.os_info,
                all_terminal_output,
                chat_history,
                input
            )
        };
        
        // In a real implementation, this would be done asynchronously
        // For simplicity, we're using blocking requests here
        match ask_ollama(&message_with_context, &self.ollama_model) {
            Ok(response) => {
                // Remove the "Thinking..." message
                if let Some(last) = self.ai_output.last() {
                    if last == THINKING_MESSAGE {
                        self.ai_output.pop();
                    }
                }
                
                // Add the response
                let _start_line_index = self.ai_output.len();
                self.ai_output.push(response.clone());
                
                // Extract commands from the response
                let commands = extract_commands(&response);
                if !commands.is_empty() {
                    // Add a separator
                    self.ai_output.push("".to_string());
                    self.ai_output.push(EXTRACTED_COMMANDS_HEADER.to_string());
                    
                    // Store the extracted commands with their line indices
                    for (i, cmd) in commands.iter().enumerate() {
                        let cmd_line_index = self.ai_output.len();
                        self.ai_output.push(format!("[{}] {} [📋] [▶️]", i + 1, cmd));
                        self.extracted_commands.push((cmd_line_index, cmd.clone()));
                    }
                    
                    // Store the first command for quick access (instead of the last)
                    if let Some(first_cmd) = commands.first() {
                        self.last_ai_command = Some(first_cmd.clone());
                        
                        // Automatically place the first command in the terminal input and execute it
                        self.copy_command_to_terminal(first_cmd);
                        
                        // Add a message about the auto filled command
                        self.ai_output.push("".to_string());
                        self.ai_output.push(format!("✅ First command automatically placed in terminal input and executed: {}", first_cmd));
                    }
                    
                    self.ai_output.push("".to_string());
                    self.ai_output.push(COMMAND_BUTTONS_HELP.to_string());
                }
            },
            Err(e) => {
                // Remove the "Thinking..." message
                if let Some(last) = self.ai_output.last() {
                    if last == THINKING_MESSAGE {
                        self.ai_output.pop();
                    }
                }
                
                self.ai_output.push(format!("Error: {}", e));
                self.ai_output.push(OLLAMA_NOT_RUNNING.to_string());
                self.ai_output.push(OLLAMA_INSTALL_INSTRUCTIONS.to_string());
            }
        }
        
        self.ollama_thinking = false;
    }

    // Copy a command to the terminal input
    pub fn copy_command_to_terminal(&mut self, command: &str) {
        // Set the terminal input to the command
        self.input = command.to_string();
        self.cursor_position = self.input.len();
        
        // Switch focus to the terminal panel
        self.active_panel = crate::model::Panel::Terminal;
        
        // Add a message to the AI output with a visual indicator
        self.ai_output.push(format!("{}{}", COMMAND_COPIED_MESSAGE, command));
        
        // Set scroll to 0 to always show the most recent output at the bottom
        self.assistant_scroll = 0;
        
        // Automatically execute the command if requested or if auto-execute is enabled
        if self.auto_execute_commands {
            self.execute_command();
        }
    }

    // Copy a command to the terminal input and execute it
    pub fn copy_and_execute_command(&mut self, command: &str) {
        // Set the terminal input to the command
        self.input = command.to_string();
        self.cursor_position = self.input.len();
        
        // Switch focus to the terminal panel
        self.active_panel = crate::model::Panel::Terminal;
        
        // Add a message to the AI output with a visual indicator
        self.ai_output.push(format!("{}{}", COMMAND_EXECUTED_MESSAGE, command));
        
        // Set scroll to 0 to always show the most recent output at the bottom
        self.assistant_scroll = 0;
        
        // Execute the command
        self.execute_command();
    }
} 