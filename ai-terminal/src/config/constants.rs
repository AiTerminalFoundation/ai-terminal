// Ollama API endpoints
pub const OLLAMA_API_URL: &str = "http://localhost:11434/api/generate";
pub const OLLAMA_LIST_MODELS_URL: &str = "http://localhost:11434/api/tags";

// Default values
pub const DEFAULT_OLLAMA_MODEL: &str = "llama3.2:latest";
pub const DEFAULT_PANEL_RATIO: u32 = 65;
pub const MAX_COMMAND_HISTORY: usize = 30;
pub const MAX_VISIBLE_SUGGESTIONS: usize = 5;
pub const SEPARATOR_LINE: &str = "─";

// UI constants
pub const WINDOW_WIDTH: i32 = 1200;
pub const WINDOW_HEIGHT: i32 = 800;
pub const TERMINAL_TITLE: &str = "Terminal Output";
pub const ASSISTANT_TITLE: &str = "AI Assistant";
pub const INPUT_TITLE: &str = "Message to AI";
pub const SUGGESTIONS_TITLE: &str = "Suggestions";

// Common Unix commands for autocompletion
pub const COMMON_COMMANDS: [&str; 35] = [
    "ls", "cd", "pwd", "mkdir", "rmdir", "touch", "rm", "cp", "mv", "cat", "grep", "find", "echo",
    "ps", "kill", "chmod", "chown", "df", "du", "tar", "gzip", "gunzip", "zip", "unzip", "ssh",
    "scp", "curl", "wget", "ping", "ifconfig", "netstat", "top", "htop", "ls -a", "ls -l",
];

// Path-related commands that might need autocompletion
pub const PATH_COMMANDS: [&str; 8] = ["cd", "ls", "cat", "rm", "cp", "mv", "mkdir", "touch"];
