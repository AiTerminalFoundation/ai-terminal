use crate::command::types::command_manager::CommandManager;
use crate::ollama::types::ai_provider::{AIProvider, ChatMessage, LocalAIRequest, LocalAIResponse};
use crate::ollama::types::ollama_model_list::OllamaModelList;
use crate::ollama::types::ollama_request::OllamaRequest;
use crate::ollama::types::ollama_response::OllamaResponse;
use crate::utils::command::handle_special_command;
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

    // Get AI configuration
    let (model, api_host, provider, temperature, max_tokens) = {
        let ollama_state = command_manager.ollama.lock().map_err(|e| e.to_string())?;
        (
            model_override.unwrap_or_else(|| ollama_state.current_model.clone()),
            ollama_state.api_host.clone(),
            ollama_state.provider.clone(),
            ollama_state.temperature,
            ollama_state.max_tokens,
        )
    };

    let client = reqwest::Client::new();
    let res = client
        .post(format!("{}/api/generate", api_host))
        .json(&OllamaRequest {
            model,
            prompt: question,
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

// LocalAI/OpenAI-compatible API request
async fn ask_local_ai(
    api_host: String,
    model: String,
    system_prompt: String,
    question: String,
    temperature: Option<f32>,
    max_tokens: Option<u32>,
) -> Result<String, String> {
    let messages = vec![
        ChatMessage {
            role: "system".to_string(),
            content: system_prompt,
        },
        ChatMessage {
            role: "user".to_string(),
            content: question,
        },
    ];

    let client = reqwest::Client::new();
    let endpoint = if api_host.ends_with("/v1/chat/completions") {
        api_host
    } else if api_host.ends_with("/v1") {
        format!("{}/chat/completions", api_host)
    } else {
        format!("{}/v1/chat/completions", api_host)
    };

    let res = client
        .post(&endpoint)
        .json(&LocalAIRequest {
            model,
            messages,
            temperature,
            max_tokens,
            stream: Some(false),
        })
        .send()
        .await
        .map_err(|e| format!("Failed to send request to LocalAI API: {}", e))?;

    if !res.status().is_success() {
        let status = res.status();
        let error_text = res
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(format!("LocalAI API error {}: {}", status, error_text));
    }

    let response: LocalAIResponse = res
        .json()
        .await
        .map_err(|e| format!("Failed to parse LocalAI response: {}", e))?;

    if let Some(choice) = response.choices.first() {
        if let Some(message) = &choice.message {
            return Ok(message.content.clone());
        }
    }

    Err("No valid response from LocalAI".to_string())
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
    Ok(format!("Changed AI API host to: {}", host))
}

// Add function to set AI provider
#[command]
pub fn set_provider(
    provider_name: String,
    command_manager: State<'_, CommandManager>,
) -> Result<String, String> {
    let provider = match provider_name.to_lowercase().as_str() {
        "ollama" => AIProvider::Ollama,
        "local" | "localai" => AIProvider::LocalAI,
        "openai" => AIProvider::OpenAI,
        _ => return Err(format!("Unknown provider: {}. Available: ollama, localai, openai", provider_name)),
    };

    let mut ollama_state = command_manager.ollama.lock().map_err(|e| e.to_string())?;
    ollama_state.provider = provider.clone();
    Ok(format!("Switched to AI provider: {}", provider))
}

// Get current provider
#[command]
pub fn get_provider(command_manager: State<'_, CommandManager>) -> Result<String, String> {
    let ollama_state = command_manager.ollama.lock().map_err(|e| e.to_string())?;
    Ok(format!("Current AI provider: {}", ollama_state.provider))
}

// Quick setup for localhost:8000
#[command]
pub fn setup_local_ai(
    model_name: Option<String>,
    command_manager: State<'_, CommandManager>,
) -> Result<String, String> {
    let mut ollama_state = command_manager.ollama.lock().map_err(|e| e.to_string())?;
    ollama_state.provider = AIProvider::LocalAI;
    ollama_state.api_host = "http://localhost:8000".to_string();
    if let Some(model) = model_name {
        ollama_state.current_model = model;
    }
    Ok("Configured to use LocalAI on localhost:8000".to_string())
}

// Set AI parameters
#[command]
pub fn set_ai_params(
    temperature: Option<f32>,
    max_tokens: Option<u32>,
    command_manager: State<'_, CommandManager>,
) -> Result<String, String> {
    let mut ollama_state = command_manager.ollama.lock().map_err(|e| e.to_string())?;
    
    if let Some(temp) = temperature {
        ollama_state.temperature = Some(temp);
    }
    if let Some(tokens) = max_tokens {
        ollama_state.max_tokens = Some(tokens);
    }
    
    Ok(format!(
        "AI parameters updated - Temperature: {:?}, Max tokens: {:?}", 
        ollama_state.temperature, 
        ollama_state.max_tokens
    ))
}
