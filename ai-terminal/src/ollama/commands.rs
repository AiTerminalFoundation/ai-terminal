use crate::config::{
    ERROR_FETCHING_MODELS, HELP_COMMANDS, HELP_FEATURES, HELP_MESSAGES, NO_MODELS_FOUND,
    OLLAMA_INSTALL_INSTRUCTIONS, OLLAMA_NOT_RUNNING, OLLAMA_PULL_INSTRUCTIONS,
};
use crate::model::App;
use crate::ollama::api;
use crate::terminal::utils;

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
                }
                "/model" => {
                    if parts.len() > 1 {
                        let model = parts[1].trim();
                        self.ollama_model = model.to_string();
                        self.ai_output.push(format!("Model changed to: {}", model));
                    } else {
                        self.ai_output
                            .push(format!("Current model: {}", self.ollama_model));
                        self.ai_output
                            .push("Usage: /model <model_name>".to_string());
                    }
                }
                "/clear" => {
                    self.ai_output = vec!["Chat history cleared.".to_string()];
                    self.extracted_commands.clear();
                }
                "/models" => {
                    self.ai_output
                        .push("Fetching available models...".to_string());
                    match api::list_models() {
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
                        }
                        Err(e) => {
                            self.ai_output
                                .push(format!("{}{}", ERROR_FETCHING_MODELS, e));
                            self.ai_output.push(OLLAMA_NOT_RUNNING.to_string());
                            self.ai_output.push(OLLAMA_INSTALL_INSTRUCTIONS.to_string());
                        }
                    }
                }
                "/autoexec" => {
                    // Toggle auto-execution of commands
                    self.auto_execute_commands = !self.auto_execute_commands;
                    if self.auto_execute_commands {
                        self.ai_output
                            .push("Auto-execution of commands is now enabled.".to_string());
                    } else {
                        self.ai_output
                            .push("Auto-execution of commands is now disabled.".to_string());
                    }
                }
                _ => {
                    self.ai_output.push(format!("Unknown command: {}", command));
                }
            }
            return;
        }

        // Send message to Ollama
        self.ollama_thinking = true;

        // Prepare the message with context if available
        let message_with_context = {
            // Include all terminal output
            let all_terminal_output = self.output.join("\n");

            // Include all chat history
            let chat_history = self
                .ai_output
                .iter()
                .filter(|line| !line.is_empty())
                .map(|s| s.as_str())
                .collect::<Vec<&str>>()
                .join("\n");

            format!(
                "System Info: {}\n\nTerminal History:\n{}\n\nChat History:\n{}\n\nUser query: {}",
                self.os_info, all_terminal_output, chat_history, input
            )
        };

        // In a real implementation, this would be done asynchronously
        // For simplicity, we're using blocking requests here
        match api::send_prompt(&self.ollama_model, &message_with_context, None) {
            Ok(response) => {
                // Add the response
                let _start_line_index = self.ai_output.len();
                self.ai_output.push(response.clone());

                // Extract commands from the response
                process_extracted_commands(self, &response);
            }
            Err(e) => {
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

        // Set scroll to 0 to always show the most recent output at the bottom
        self.assistant_scroll = 0;

        // Execute the command
        self.execute_command();
    }
}

// Process AI input and update the AI output
pub fn process_ai_input(app: &mut App) {
    let input = app.ai_input.clone();
    app.ai_input.clear();
    app.ai_cursor_position = 0;
    
    // Add the user input to the AI output
    app.ai_output.push(format!("> {}", input));
    
    // Check if the input is a command
    if input.starts_with('/') {
        process_ai_command(app, &input);
        return;
    }
    
    // Set the thinking flag
    app.ollama_thinking = true;
    
    // Create context from the last terminal command if available
    let context = if let Some((cmd, output)) = &app.last_terminal_context {
        format!(
            "Last terminal command: {}\nOutput:\n{}\n\nUser question: {}",
            cmd,
            output.join("\n"),
            input
        )
    } else {
        input
    };
    
    // Send the prompt to Ollama
    match api::send_prompt(&app.ollama_model, &context, None) {
        Ok(response) => {
            // Add the response to the AI output
            for line in response.lines() {
                app.ai_output.push(line.to_string());
            }
            
            // Extract commands from the response
            process_extracted_commands(app, &response);
        }
        Err(e) => {
            app.ai_output.push(format!("Error: {}", e));
        }
    }
    
    // Clear the thinking flag
    app.ollama_thinking = false;
}

// Process AI commands (starting with /)
fn process_ai_command(app: &mut App, command: &str) {
    let parts: Vec<&str> = command.split_whitespace().collect();
    let cmd = parts[0];
    
    match cmd {
        "/help" => {
            app.ai_output.push("Available commands:".to_string());
            app.ai_output.push("/help - Show this help message".to_string());
            app.ai_output.push("/model <name> - Change the Ollama model".to_string());
            app.ai_output.push("/models - List available Ollama models".to_string());
            app.ai_output.push("/clear - Clear the AI output".to_string());
            app.ai_output.push("/auto <on|off> - Toggle auto-execution of commands".to_string());
        }
        "/model" => {
            if parts.len() < 2 {
                app.ai_output.push("Current model: ".to_string() + &app.ollama_model);
                app.ai_output.push("Usage: /model <name>".to_string());
            } else {
                let model_name = parts[1];
                app.ollama_model = model_name.to_string();
                app.ai_output.push(format!("Model changed to: {}", model_name));
            }
        }
        "/models" => {
            match api::list_models() {
                Ok(models) => {
                    app.ai_output.push("Available models:".to_string());
                    for model in models {
                        app.ai_output.push(format!("- {}", model));
                    }
                }
                Err(e) => {
                    app.ai_output.push(format!("Error listing models: {}", e));
                }
            }
        }
        "/clear" => {
            app.ai_output.clear();
            app.ai_output.push("AI output cleared.".to_string());
        }
        "/auto" => {
            if parts.len() < 2 {
                app.ai_output.push(format!("Auto-execute commands: {}", if app.auto_execute_commands { "on" } else { "off" }));
                app.ai_output.push("Usage: /auto <on|off>".to_string());
            } else {
                let setting = parts[1];
                match setting {
                    "on" => {
                        app.auto_execute_commands = true;
                        app.ai_output.push("Auto-execute commands: on".to_string());
                    }
                    "off" => {
                        app.auto_execute_commands = false;
                        app.ai_output.push("Auto-execute commands: off".to_string());
                    }
                    _ => {
                        app.ai_output.push("Invalid setting. Use 'on' or 'off'.".to_string());
                    }
                }
            }
        }
        _ => {
            app.ai_output.push(format!("Unknown command: {}", cmd));
            app.ai_output.push("Type /help for available commands.".to_string());
        }
    }
}

// Process extracted commands from the AI response
fn process_extracted_commands(app: &mut App, response: &str) {
    app.extracted_commands.clear();
    
    // Use the utility function to extract commands
    let commands = utils::extract_commands(response);
    
    // Store the extracted commands with their line indices
    for (i, cmd) in commands.iter().enumerate() {
        app.extracted_commands.push((i, cmd.clone()));
    }
    
    // If there's exactly one command, store it as the last AI command
    if app.extracted_commands.len() == 1 {
        app.last_ai_command = Some(app.extracted_commands[0].1.clone());
    } else {
        app.last_ai_command = None;
    }
}
