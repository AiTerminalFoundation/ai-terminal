use std::error::Error;
use reqwest::blocking::Client;
use crate::config::{OLLAMA_API_URL, OLLAMA_LIST_MODELS_URL, SYSTEM_PROMPT};
use crate::model::{OllamaRequest, OllamaResponse, OllamaModelList};

/// Function to send a message to Ollama and get a response
pub fn ask_ollama(message: &str, model: &str) -> Result<String, Box<dyn Error>> {
    let client = Client::new();
    
    let request = OllamaRequest {
        model: model.to_string(),
        prompt: message.to_string(),
        stream: false,
        system: Some(SYSTEM_PROMPT.to_string()),
    };
    
    let response = client.post(OLLAMA_API_URL)
        .json(&request)
        .send()?
        .json::<OllamaResponse>()?;
    
    Ok(response.response)
}

/// Function to list available Ollama models
pub fn list_ollama_models() -> Result<Vec<String>, Box<dyn Error>> {
    let client = Client::new();
    
    let response = client.get(OLLAMA_LIST_MODELS_URL)
        .send()?
        .json::<OllamaModelList>()?;
    
    Ok(response.models.into_iter().map(|m| m.name).collect())
} 