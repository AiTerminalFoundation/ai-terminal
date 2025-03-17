// Extract commands from a response string
pub fn extract_commands(response: &str) -> String {
    // Strip any backticks from the response
    let cleaned_response = response
        .trim()
        .replace("```", "")
        .trim()
        .to_string();
    println!("cleaned: {}", cleaned_response);
    cleaned_response
}