// Function to detect OS information
pub fn detect_os() -> String {
    std::env::consts::OS.to_string()
}

// Function to extract commands from AI response
pub fn extract_commands(response: &str) -> Vec<String> {
    let mut commands = Vec::new();
    let mut in_code_block = false;
    let mut current_command = String::new();
    
    for line in response.lines() {
        let trimmed = line.trim();
        
        // Check for code block markers
        if trimmed.starts_with("```") {
            if !in_code_block {
                // Start of code block
                in_code_block = true;
                // Skip the opening line if it contains a language specifier
                // e.g., ```bash, ```sh, etc.
                continue;
            } else {
                // End of code block
                if !current_command.trim().is_empty() {
                    commands.push(current_command.trim().to_string());
                }
                current_command = String::new();
                in_code_block = false;
            }
        } else if in_code_block {
            // Inside code block, collect command
            current_command.push_str(line);
            current_command.push('\n');
        }
    }
    
    // In case there's an unclosed code block
    if in_code_block && !current_command.trim().is_empty() {
        commands.push(current_command.trim().to_string());
    }
    
    commands
} 