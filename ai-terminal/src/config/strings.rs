pub const APP_TITLE: &str = "AI Terminal!";

// Terminal instructions
pub const TERMINAL_INSTRUCTIONS: [&str; 1] = [
    "Type commands to execute them in the terminal.",
];

// AI welcome message
pub const AI_WELCOME_MESSAGE: &str = "Hello! I'm your AI assistant. How can I help you today?";

// AI instructions
pub const AI_INSTRUCTIONS: [&str; 2] = [
    "Type your questions or requests in the input box below.",
    "I can help you with terminal commands and provide explanations.",
];

// Help messages
pub const HELP_MESSAGES: [&str; 2] = ["Available commands:", "Features:"];

pub const HELP_COMMANDS: [&str; 5] = [
    "  /model <model_name> - Change the Ollama model",
    "  /help - Show this help message",
    "  /clear - Clear the chat history",
    "  /models - List available models (requires Ollama to be running)",
    "  /auto <on|off> - Toggle automatic execution of commands",
];

pub const HELP_FEATURES: [&str; 2] = [
    "  - System information is provided to the AI for better command compatibility",
    "  - AI can suggest and execute terminal commands based on your queries",
];

// Error messages
pub const ERROR_FETCHING_MODELS: &str = "Error fetching models: ";
pub const OLLAMA_NOT_RUNNING: &str = "Make sure Ollama is running (http://localhost:11434)";
pub const OLLAMA_INSTALL_INSTRUCTIONS: &str = "You can install Ollama from https://ollama.ai";
pub const NO_MODELS_FOUND: &str = "No models found. You need to pull models first.";
pub const OLLAMA_PULL_INSTRUCTIONS: &str =
    "Run 'ollama pull llama3.2:latest' in the terminal to get started.";
