// Remove the unused import

// Function to detect OS information
pub fn detect_os() -> String {
    std::env::consts::OS.to_string()
}

// Extract commands from a response string
pub fn extract_commands(response: &str) -> Vec<String> {
    // Strip any backticks from the response
    let cleaned_response = response
        .trim()
        .replace("```", "")
        .trim()
        .to_string();
    
    // Return the cleaned response as a command
    if !cleaned_response.is_empty() {
        vec![cleaned_response]
    } else {
        Vec::new()
    }
}