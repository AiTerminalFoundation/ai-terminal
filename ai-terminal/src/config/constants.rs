// Ollama API endpoints
pub const OLLAMA_API_URL: &str = "http://localhost:11434/api/generate";
pub const OLLAMA_LIST_MODELS_URL: &str = "http://localhost:11434/api/tags";

// Default values
pub const DEFAULT_OLLAMA_MODEL: &str = "macsdeve/BetterBash3:latest";
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
pub const COMMON_COMMANDS: &[&str] = &[
    "ls", "cd", "pwd", "cat", "grep", "find", "git", "vim", "nano",
    "mkdir", "rm", "cp", "mv", "touch", "echo", "clear", "history",
    "ps", "top", "ssh", "scp", "curl", "wget", "tar", "zip", "unzip",
];

// Path-related commands that might need autocompletion
pub const PATH_COMMANDS: &[&str] = &[
    "cd", "ls", "cat", "vim", "nano", "rm", "cp", "mv", "touch", "mkdir",
];
