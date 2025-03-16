// Remove the unused import

// Function to detect OS information
pub fn detect_os() -> String {
    std::env::consts::OS.to_string()
}

// Extract commands from a response string
pub fn extract_commands(response: &str) -> Vec<String> {
    let mut commands = Vec::new();
    
    // Look for commands in backticks
    for line in response.lines() {
        if let Some(cmd) = extract_command_from_backticks(line) {
            commands.push(cmd.to_string());
        }
    }
    
    commands
}

// Extract a command from backticks in a line
fn extract_command_from_backticks(line: &str) -> Option<&str> {
    if let Some(start) = line.find('`') {
        if let Some(end) = line[start + 1..].find('`') {
            let cmd = &line[start + 1..start + 1 + end];
            if !cmd.is_empty() {
                return Some(cmd);
            }
        }
    }
    None
}
