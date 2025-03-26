use reqwest::Client;
use crate::config::{OLLAMA_API_URL, OLLAMA_LIST_MODELS_URL};
use crate::model::{OllamaModelList, OllamaRequest, OllamaResponse};
use crate::ollama::prompt_eng::{trim_context, extract_user_query};
use std::sync::atomic::{AtomicBool, Ordering};

// Global flag to track if a request is in progress
pub static IS_THINKING: AtomicBool = AtomicBool::new(false);
// Track if we're using reduced context
static USING_REDUCED_CONTEXT: AtomicBool = AtomicBool::new(false);

// Send a prompt to Ollama and get the response
pub async fn send_prompt(model: &str, prompt: &str) -> Result<String, String> {
    println!("send_prompt: Sending prompt to model {}", model);
    
    // Reset flag - we'll attempt with full context first unless explicitly in reduced mode
    let using_reduced = USING_REDUCED_CONTEXT.load(Ordering::SeqCst);
    let actual_prompt = if using_reduced {
        println!("send_prompt: Using reduced context due to previous failure");
        extract_user_query(prompt)
    } else {
        // Even for "full context" mode, trim the context to a reasonable size
        println!("send_prompt: Using standard context");
        trim_context(prompt)
    };
    
    let result = {
        let client = Client::new();
        
        let request = OllamaRequest {
            model: model.to_string(),
            prompt: actual_prompt,
            stream: false,
            system: None, // add here the system prompt
        };
        
        println!("send_prompt: Sending request to {}", OLLAMA_API_URL);
        match client.post(OLLAMA_API_URL).json(&request).send().await {
            Ok(response) => {
                println!("send_prompt: Got response with status {}", response.status());
                if response.status().is_success() {
                    match response.json::<OllamaResponse>().await {
                        Ok(ollama_response) => {
                            println!("send_prompt: Successfully parsed response");
                            
                            // Check if we received an empty response
                            if ollama_response.response.trim().is_empty() {
                                println!("send_prompt: Empty response received, trying with simplified context");
                                
                                // Try with a simplified request - just the user's query
                                let simplified_request = OllamaRequest {
                                    model: model.to_string(),
                                    prompt: extract_user_query(prompt),
                                    stream: false,
                                    system: None,
                                };
                                
                                match client.post(OLLAMA_API_URL).json(&simplified_request).send().await {
                                    Ok(simplified_response) => {
                                        if simplified_response.status().is_success() {
                                            match simplified_response.json::<OllamaResponse>().await {
                                                Ok(simplified_ollama_response) => {
                                                    if simplified_ollama_response.response.trim().is_empty() {
                                                        // Both attempts failed - keep reduced context for next time
                                                        USING_REDUCED_CONTEXT.store(true, Ordering::SeqCst);
                                                        Ok("I'm sorry, I couldn't generate a response. Please try again with a simpler query.".to_string())
                                                    } else {
                                                        // Simplified prompt worked - keep reduced context
                                                        USING_REDUCED_CONTEXT.store(true, Ordering::SeqCst);
                                                        Ok(simplified_ollama_response.response)
                                                    }
                                                },
                                                Err(e) => Err(format!("Failed to parse simplified response: {}", e))
                                            }
                                        } else {
                                            Err(format!("API error with simplified request: {}", simplified_response.status()))
                                        }
                                    },
                                    Err(e) => Err(format!("Simplified request error: {}", e))
                                }
                            } else {
                                // Full context response succeeded - reset to full context
                                USING_REDUCED_CONTEXT.store(false, Ordering::SeqCst);
                                Ok(ollama_response.response)
                            }
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