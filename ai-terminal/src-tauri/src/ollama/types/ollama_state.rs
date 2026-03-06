pub struct OllamaState {
    pub current_model: String,
    pub api_host: String,
    pub provider: AIProvider,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
}

impl Default for OllamaState {
    fn default() -> Self {
        Self {
            current_model: "llama2".to_string(),
            api_host: "http://localhost:11434".to_string(),
            provider: AIProvider::Ollama,
            temperature: Some(0.7),
            max_tokens: Some(2048),
        }
    }
}
