use reqwest::Client;
use std::error::Error;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::config::{OLLAMA_API_URL, OLLAMA_LIST_MODELS_URL};
use crate::model::{OllamaModelList, OllamaRequest, OllamaResponse};

// Global flag to track if a request is in progress
pub static IS_THINKING: AtomicBool = AtomicBool::new(false);

/// Function to send a message to Ollama and get a response
pub async fn ask_ollama(message: &str, model: &str) -> Result<String, Box<dyn Error>> {
    println!("ask_ollama: Sending message to model {}", model);
    let client = Client::new();

    let request = OllamaRequest {
        model: model.to_string(),
        prompt: message.to_string(),
        stream: false,
        system: None,
    };

    println!("ask_ollama: Sending request to {}", OLLAMA_API_URL);
    let response = client
        .post(OLLAMA_API_URL)
        .json(&request)
        .send()
        .await?
        .json::<OllamaResponse>()
        .await?;

    println!("ask_ollama: Got response");
    Ok(response.response)
}

/// Function to list available Ollama models
pub async fn list_ollama_models() -> Result<Vec<String>, Box<dyn Error>> {
    println!("list_ollama_models: Fetching models from {}", OLLAMA_LIST_MODELS_URL);
    let client = Client::new();

    let response = client
        .get(OLLAMA_LIST_MODELS_URL)
        .send()
        .await?
        .json::<OllamaModelList>()
        .await?;

    println!("list_ollama_models: Got {} models", response.models.len());
    Ok(response.models.into_iter().map(|m| m.name).collect())
}

// Send a prompt to Ollama and get the response
pub async fn send_prompt(model: &str, prompt: &str) -> Result<String, String> {
    println!("send_prompt: Sending prompt to model {}", model);
    let result = {
        let client = Client::new();
        
        let request = OllamaRequest {
            model: model.to_string(),
            prompt: prompt.to_string(),
            stream: false,
            system: None,
        };
        
        println!("send_prompt: Sending request to {}", OLLAMA_API_URL);
        match client.post(OLLAMA_API_URL).json(&request).send().await {
            Ok(response) => {
                println!("send_prompt: Got response with status {}", response.status());
                if response.status().is_success() {
                    match response.json::<OllamaResponse>().await {
                        Ok(ollama_response) => {
                            println!("send_prompt: Successfully parsed response");
                            Ok(ollama_response.response)
                        },
                        Err(e) => {
                            println!("send_prompt: Failed to parse response: {}", e);
                            Err(format!("Failed to parse response: {}", e))
                        }
                    }
                } else {
                    println!("send_prompt: API error with status {}", response.status());
                    Err(format!("API error: {}", response.status()))
                }
            }
            Err(e) => {
                println!("send_prompt: Request error: {}", e);
                Err(format!("Request error: {}", e))
            }
        }
    };
    
    // Set the thinking flag back to false after getting the response
    IS_THINKING.store(false, Ordering::SeqCst);
    
    result
}

// Get a list of available models from Ollama
pub async fn list_models() -> Result<Vec<String>, String> {
    println!("list_models: Fetching models from {}", OLLAMA_LIST_MODELS_URL);
    let client = Client::new();
    
    match client.get(OLLAMA_LIST_MODELS_URL).send().await {
        Ok(response) => {
            println!("list_models: Got response with status {}", response.status());
            if response.status().is_success() {
                match response.json::<OllamaModelList>().await {
                    Ok(model_list) => {
                        println!("list_models: Successfully parsed {} models", model_list.models.len());
                        let models = model_list.models.into_iter()
                            .map(|model| model.name)
                            .collect();
                        Ok(models)
                    }
                    Err(e) => {
                        println!("list_models: Failed to parse response: {}", e);
                        Err(format!("Failed to parse response: {}", e))
                    }
                }
            } else {
                println!("list_models: API error with status {}", response.status());
                Err(format!("API error: {}", response.status()))
            }
        }
        Err(e) => {
            println!("list_models: Request error: {}", e);
            Err(format!("Request error: {}", e))
        }
    }
}

// Check if the LLM is currently thinking
pub fn is_thinking() -> bool {
    IS_THINKING.load(Ordering::SeqCst)
}
