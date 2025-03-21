use crate::config::{HELP_COMMANDS,
};
use crate::model::App;
use crate::ollama::api;
use crate::terminal::utils;
use std::thread;
use std::time::Duration;
use std::sync::atomic::Ordering;

// Process AI input and update the AI output
pub fn process_ai_input(app: &mut App, query: String) -> String {
    app.ai_input.clear();
    app.ai_cursor_position = 0;
    
    // Add the user input to the AI output
    app.ai_output.push(format!("> {}", query));
    
    // Check if the input is a command
    if query.starts_with('/') {
        process_ai_command(app, &query);
        api::IS_THINKING.store(false, Ordering::SeqCst);
        return "".to_string();
    } else {
        app.ai_output.push("🤔 Thinking...".to_string());

        // Prepare the message with context
        let message_with_context = {
            // Include OS information
            let os_info = &app.os_info;

            // Include terminal output (limited to last 20 lines to avoid context overflow)
            let terminal_output = app.output.iter()
                .rev()
                .take(20)
                .rev()
                .map(|s| s.as_str())
                .collect::<Vec<&str>>()
                .join("\n");

            // Include recent chat history (limited to last 10 exchanges)
            let chat_history = app.ai_output.iter()
                .rev()
                .take(10)
                .rev()
                .filter(|line| !line.is_empty())
                .map(|s| s.as_str())
                .collect::<Vec<&str>>()
                .join("\n");

            // Format the context
            format!(
                "System Info: {}\n\nRecent Terminal Output:\n{}\n\nRecent Chat History:\n{}\n\nUser query: {}\n\nCurrent directory: {}",
                os_info, terminal_output, chat_history, query, app.current_dir.display().to_string()
            )
        };

        // Custom system prompt that instructs the LLM to format commands properly
        let system_prompt = format!(
            "{}",
            crate::config::SYSTEM_PROMPT
        );

        // Send the prompt to Ollama with the system prompt
        match api::send_prompt(&app.ollama_model, &message_with_context, Some(&system_prompt)) {
            Ok(response) => {
                // Remove the "thinking" indicator
                if let Some(last) = app.ai_output.last() {
                    if last.contains("🤔 Thinking...") {
                        app.ai_output.pop();
                    }
                }
                
                // Extract commands from the response first
                let extracted_command = utils::extract_commands(&response);
                
                // Add the AI response to output
                app.ai_output.push(response.clone());

                println!("Extracted command: {} end-1", extracted_command);

                extracted_command
            }
            Err(e) => {
                // Remove the "thinking" indicator
                if let Some(last) = app.ai_output.last() {
                    if last.contains("🤔 Thinking...") {
                        app.ai_output.pop();
                    }
                }
                
                let error_msg = format!("Error: {}", e);
                app.ai_output.push(error_msg.clone());
                error_msg
            }
        }
    }
}

// Process AI commands (starting with /)
fn process_ai_command(app: &mut App, command: &str) {
    let parts: Vec<&str> = command.split_whitespace().collect();
    let cmd = parts[0];
    
    match cmd {
        "/help" => {
            for help_command in HELP_COMMANDS {
                app.ai_output.push(help_command.to_string());
            }
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
            app.ai_output.push("🔍 Fetching models...".to_string());
            
            match api::list_models() {
                Ok(models) => {
                    // Remove the "thinking" indicator
                    if let Some(last) = app.ai_output.last() {
                        if last.contains("🔍 Fetching models...") {
                            app.ai_output.pop();
                        }
                    }
                    
                    app.ai_output.push("Available models:".to_string());
                    for model in models {
                        app.ai_output.push(format!("- {}", model));
                    }
                }
                Err(e) => {
                    if let Some(last) = app.ai_output.last() {
                        if last.contains("🔍 Fetching models...") {
                            app.ai_output.pop();
                        }
                    }
                    
                    app.ai_output.push(format!("Error listing models: {}", e));
                }
            }
        }
        "/clear" => {
            app.ai_output.clear();
            app.ai_output.push("AI output cleared.".to_string());
        }
        "/autoexec" => {
            app.auto_execute_commands = !app.auto_execute_commands;
            app.ai_output.push(format!("Auto-execute commands: {}", if app.auto_execute_commands { "on" } else { "off" }));
        }
        _ => {
            app.ai_output.push(format!("Unknown command: {}", cmd));
            app.ai_output.push("Type /help for available commands.".to_string());
        }
    }
}

// Add these new functions to handle command history navigation
pub fn navigate_history_up(app: &mut App) {
    if app.command_history.is_empty() {
        return;
    }
    
    let new_index = match app.command_history_index {
        Some(idx) if idx > 0 => Some(idx - 1),
        None => Some(app.command_history.len() - 1),
        Some(idx) => Some(idx),
    };
    
    app.command_history_index = new_index;
    
    if let Some(idx) = new_index {
        app.ai_input = app.command_history[idx].clone();
        app.ai_cursor_position = app.ai_input.len();
    }
}

pub fn navigate_history_down(app: &mut App) {
    if app.command_history.is_empty() {
        return;
    }
    
    let new_index = match app.command_history_index {
        Some(idx) if idx < app.command_history.len() - 1 => Some(idx + 1),
        Some(_) => None,
        None => None,
    };
    
    app.command_history_index = new_index;
    
    if let Some(idx) = new_index {
        app.ai_input = app.command_history[idx].clone();
    } else {
        app.ai_input.clear();
    }
    
    app.ai_cursor_position = app.ai_input.len();
}