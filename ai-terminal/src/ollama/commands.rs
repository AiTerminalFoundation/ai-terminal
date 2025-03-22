use crate::config::{HELP_COMMANDS};
use crate::model::App;
use crate::ollama::api;

pub fn process_ai_command(app: &mut App, command: &str) {
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
                app.ai_output.push("Usage: /model <n>".to_string());
            } else {
                let model_name = parts[1];
                app.ollama_model = model_name.to_string();
                app.ai_output.push(format!("Model changed to: {}", model_name));
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