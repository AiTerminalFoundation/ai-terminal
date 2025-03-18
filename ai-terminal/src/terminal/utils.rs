// Extract commands from a response string
pub fn extract_commands(response: &str) -> String {
    // Strip any backticks from the response
    let start = response.find("```").unwrap_or_default();
    let end = response[start + 3..].find("```").unwrap_or_default();

    response[start + 3..start + 3 + end].trim().to_string()
}