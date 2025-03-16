use reqwest::blocking::Client;
use std::error::Error;

use crate::config::{OLLAMA_API_URL, OLLAMA_LIST_MODELS_URL};
use crate::model::{OllamaModelList, OllamaRequest, OllamaResponse};

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
