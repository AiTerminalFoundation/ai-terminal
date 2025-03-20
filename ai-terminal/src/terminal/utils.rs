use regex::Regex;

// Extract commands from a response string
pub fn extract_commands(text: &str) -> String {
    let re = Regex::new(r"```(?:bash|sh)?\s*([\s\S]*?)\n?```").unwrap();
    if let Some(captures) = re.captures(text) {
        if let Some(command_match) = captures.get(1) {
            return command_match.as_str().trim().to_string();
        }
    } 
    String::new()
}