use crate::command::types::command_manager::CommandManager;
use crate::ollama::types::ollama_model_list::OllamaModelList;
use crate::ollama::types::ollama_request::OllamaRequest;
use crate::ollama::types::ollama_response::OllamaResponse;
use crate::utils::command::handle_special_command;
use crate::utils::operating_system_utils::get_operating_system;
use tauri::{command, State};

#[command]
pub async fn ask_ai(
    question: String,
    model_override: Option<String>,
    command_manager: State<'_, CommandManager>,
) -> Result<String, String> {
    // Check if this is a special command
    if question.starts_with('/') {
        return handle_special_command(question, command_manager).await;
    }

    // Regular message to Ollama
    let model;
    let api_host;

    // Scope the mutex lock to drop it before any async operations
    {
        let ollama_state = command_manager.ollama.lock().map_err(|e| e.to_string())?;
        // Use the model_override if provided, otherwise use the default
        model = model_override.unwrap_or_else(|| ollama_state.current_model.clone());
        api_host = ollama_state.api_host.clone();
        // MutexGuard is dropped here at the end of scope
    }

    // Get the current operating system
    let os = get_operating_system();

    // Create a system prompt that includes OS information and formatting instructions
    let system_prompt = format!(
        "You are a helpful terminal assistant. The user is using a {} operating system. \
        When providing terminal commands, ensure they are compatible with {}. \
        When asked for a command, respond with ONLY the command in this format: ```command```\
        The command should be a single line without any explanation or additional text.",
        os, os
    );

    // Combine the system prompt with the user's question
    let combined_prompt = format!("{}\n\nUser: {}", system_prompt, question);

    let client = reqwest::Client::new();
    let res = client
        .post(format!("{}/api/generate", api_host))
        .json(&OllamaRequest {
            model,
            prompt: combined_prompt,
            stream: false,
        })
        .send()
        .await
        .map_err(|e| format!("Failed to send request to Ollama API: {}", e))?;

    if !res.status().is_success() {
        return Err(format!("Ollama API error: {}", res.status()));
    }

    let response: OllamaResponse = res
        .json()
        .await
        .map_err(|e| format!("Failed to parse Ollama response: {}", e))?;

    Ok(response.response)
}

// Add function to get models from Ollama API
#[command]
pub async fn get_models(command_manager: State<'_, CommandManager>) -> Result<String, String> {
    // Get the API host from the Ollama state
    let api_host;
    {
        let ollama_state = command_manager.ollama.lock().map_err(|e| e.to_string())?;
        api_host = ollama_state.api_host.clone();
    }

    // Request the list of models from Ollama
    let client = reqwest::Client::new();
    let res = client
        .get(format!("{}/api/tags", api_host))
        .send()
        .await
        .map_err(|e| format!("Failed to get models from Ollama API: {}", e))?;

    if !res.status().is_success() {
        return Err(format!("Ollama API error: {}", res.status()));
    }

    // Parse the response
    let models: OllamaModelList = res
        .json()
        .await
        .map_err(|e| format!("Failed to parse models list: {}", e))?;

    // Format the response
    let mut result = String::from("Available models:\n");
    for model in models.models {
        result.push_str(&format!("- {} ({} bytes)\n", model.name, model.size));
    }
    Ok(result)
}

// Add function to switch model
#[command]
pub fn switch_model(
    model: String,
    command_manager: State<'_, CommandManager>,
) -> Result<String, String> {
    let mut ollama_state = command_manager.ollama.lock().map_err(|e| e.to_string())?;
    ollama_state.current_model = model.clone();
    Ok(format!("Switched to model: {}", model))
}

// Add function to get current API host
#[command]
pub fn get_host(command_manager: State<'_, CommandManager>) -> Result<String, String> {
    let ollama_state = command_manager.ollama.lock().map_err(|e| e.to_string())?;
    Ok(format!(
        "Current Ollama API host: {}",
        ollama_state.api_host
    ))
}

// Add function to set API host
#[command]
pub fn set_host(
    host: String,
    command_manager: State<'_, CommandManager>,
) -> Result<String, String> {
    let mut ollama_state = command_manager.ollama.lock().map_err(|e| e.to_string())?;
    ollama_state.api_host = host.clone();
    Ok(format!("Changed Ollama API host to: {}", host))
}
