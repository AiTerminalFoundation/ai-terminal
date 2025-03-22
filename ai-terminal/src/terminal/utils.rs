use regex::Regex;

// Extract commands from a response string
pub fn extract_commands(text: &str) -> String {
    println!("Input text: '{}'", text);  // Debug print
    
    // First try to match complete code blocks
    let re = Regex::new(r"```\s*(?:\w+)?\s*(.+?)```").unwrap();
    if let Some(captures) = re.captures(text) {
        if let Some(command_match) = captures.get(1) {
            let result = command_match.as_str().trim().to_string();
            println!("Matched (complete): '{}'", result);  // Debug print
            return result;
        }
    }

    // If no complete block found, try to match just after opening ```
    let re_open = Regex::new(r"```\s*(.+)").unwrap();
    if let Some(captures) = re_open.captures(text) {
        if let Some(command_match) = captures.get(1) {
            let result = command_match.as_str().trim().to_string();
            println!("Matched (open): '{}'", result);  // Debug print
            return result;
        }
    }
    
    println!("No match found");  // Debug print
    String::new()
}