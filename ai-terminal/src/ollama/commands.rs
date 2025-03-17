use crate::config::{HELP_COMMANDS,
};
use crate::model::App;
use crate::ollama::api;
use crate::terminal::utils;

// Process AI input and update the AI output
pub fn process_ai_input(app: &mut App) -> String {
    let input = app.ai_input.clone();
    app.ai_input.clear();
    app.ai_cursor_position = 0;
    
    // Add the user input to the AI output
    app.ai_output.push(format!("> {}", input));
    
    // Check if the input is a command
    if input.starts_with('/') {
        process_ai_command(app, &input);
        "".to_string()
    } else {

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
                os_info, terminal_output, chat_history, input, app.current_dir.display().to_string()
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
                // Add the response to the AI output
                for line in response.lines() {
                    app.ai_output.push(line.to_string());
                }

                // Extract commands from the response - in this case, the entire response is the command
                process_extracted_commands(&response)
            }
            Err(e) => {
                app.ai_output.push(format!("Error: {}", e));
                "".to_string()
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
fn process_extracted_commands(response: &str) -> String {
    utils::extract_commands(response)
}
