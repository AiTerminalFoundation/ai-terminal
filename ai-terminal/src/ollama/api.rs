use reqwest::blocking::Client;
use std::error::Error;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::config::{OLLAMA_API_URL, OLLAMA_LIST_MODELS_URL};
use crate::model::{OllamaModelList, OllamaRequest, OllamaResponse};

// Global flag to track if a request is in progress
pub static IS_THINKING: AtomicBool = AtomicBool::new(false);

/// Function to send a message to Ollama and get a response
pub fn ask_ollama(message: &str, model: &str) -> Result<String, Box<dyn Error>> {
    let client = Client::new();

    let request = OllamaRequest {
        model: model.to_string(),
        prompt: message.to_string(),
        stream: false,
        system: None,
    };

    let response = client
        .post(OLLAMA_API_URL)
        .json(&request)
        .send()?
        .json::<OllamaResponse>()?;

    Ok(response.response)
}

/// Function to list available Ollama models
pub fn list_ollama_models() -> Result<Vec<String>, Box<dyn Error>> {
    let client = Client::new();

    let response = client
        .get(OLLAMA_LIST_MODELS_URL)
        .send()?
        .json::<OllamaModelList>()?;

    Ok(response.models.into_iter().map(|m| m.name).collect())
}

// Send a prompt to Ollama and get the response
pub fn send_prompt(model: &str, prompt: &str, system: Option<&str>) -> Result<String, String> {
    let result = {
        let client = Client::new();
        
        let request = OllamaRequest {
            model: model.to_string(),
            prompt: prompt.to_string(),
            stream: false,
            system: system.map(|s| s.to_string()),
        };
        
        match client.post(OLLAMA_API_URL).json(&request).send() {
            Ok(response) => {
                if response.status().is_success() {
                    match response.json::<OllamaResponse>() {
                        Ok(ollama_response) => Ok(ollama_response.response),
                        Err(e) => Err(format!("Failed to parse response: {}", e)),
                    }
                } else {
                    Err(format!("API error: {}", response.status()))
                }
            }
            Err(e) => Err(format!("Request error: {}", e)),
        }
    };
    
    // Set the thinking flag back to false after getting the response
    IS_THINKING.store(false, Ordering::SeqCst);
    
    result
}

// Get a list of available models from Ollama
pub fn list_models() -> Result<Vec<String>, String> {
    let client = Client::new();
    
    match client.get(OLLAMA_LIST_MODELS_URL).send() {
        Ok(response) => {
            if response.status().is_success() {
                match response.json::<OllamaModelList>() {
                    Ok(model_list) => {
                        let models = model_list.models.into_iter()
                            .map(|model| model.name)
                            .collect();
                        Ok(models)
                    }
                    Err(e) => Err(format!("Failed to parse response: {}", e)),
                }
            } else {
                Err(format!("API error: {}", response.status()))
            }
        }
        Err(e) => Err(format!("Request error: {}", e)),
    }
}

// Check if the LLM is currently thinking
pub fn is_thinking() -> bool {
    IS_THINKING.load(Ordering::SeqCst)
}
