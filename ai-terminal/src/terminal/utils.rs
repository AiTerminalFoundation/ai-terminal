use std::env;
use std::process::Command;

// Function to detect OS information
pub fn detect_os_info() -> String {
    let mut os_info = String::new();
    
    // Get OS name and version
    if let Ok(os_release) = Command::new("uname").arg("-a").output() {
        if os_release.status.success() {
            let output = String::from_utf8_lossy(&os_release.stdout).trim().to_string();
            os_info = output;
        }
    }
    
    // If uname failed (e.g., on Windows), try alternative methods
    if os_info.is_empty() {
        if cfg!(target_os = "windows") {
            os_info = "Windows".to_string();
            // Try to get Windows version
            if let Ok(ver) = Command::new("cmd").args(["/C", "ver"]).output() {
                if ver.status.success() {
                    let output = String::from_utf8_lossy(&ver.stdout).trim().to_string();
                    os_info = output;
                }
            }
        } else if cfg!(target_os = "macos") {
            os_info = "macOS".to_string();
            // Try to get macOS version
            if let Ok(ver) = Command::new("sw_vers").output() {
                if ver.status.success() {
                    let output = String::from_utf8_lossy(&ver.stdout).trim().to_string();
                    os_info = output;
                }
            }
        } else if cfg!(target_os = "linux") {
            os_info = "Linux".to_string();
            // Try to get Linux distribution
            if let Ok(ver) = Command::new("cat").arg("/etc/os-release").output() {
                if ver.status.success() {
                    let output = String::from_utf8_lossy(&ver.stdout).trim().to_string();
                    if let Some(name_line) = output.lines().find(|l| l.starts_with("PRETTY_NAME=")) {
                        if let Some(name) = name_line.strip_prefix("PRETTY_NAME=") {
                            os_info = name.trim_matches('"').to_string();
                        }
                    }
                }
            }
        }
    }
    
    // If all else fails, use Rust's built-in OS detection
    if os_info.is_empty() {
        os_info = format!("OS: {}", env::consts::OS);
    }
    
    os_info
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