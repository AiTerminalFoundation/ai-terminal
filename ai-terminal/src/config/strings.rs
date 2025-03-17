pub const APP_TITLE: &str = "AI Terminal!";

// Terminal instructions
pub const TERMINAL_INSTRUCTIONS: [&str; 3] = [
    "Type commands to execute them in the terminal.",
    "Press Tab to switch between terminal and AI assistant panels.",
    "Press Ctrl+C to exit the application.",
];

// AI welcome message
pub const AI_WELCOME_MESSAGE: &str = "Hello! I'm your AI assistant. How can I help you today?";

// AI instructions
pub const AI_INSTRUCTIONS: [&str; 2] = [
    "Type your questions or requests in the input box below.",
    "I can help you with terminal commands and provide explanations.",
];

// AI System prompt
pub const SYSTEM_PROMPT: &str =
    "You are a helpful AI assistant integrated into a terminal application. \
Your primary role is to suggest terminal commands that solve the user's requests. \
\
IMPORTANT FORMATTING INSTRUCTIONS: \
Always format your commands in a code block using triple backticks, with each command on its own line: \
```command``` if you have more commands use & like in the following example: ```command1 & command2 & command3``` \
When responding: answer with only one command, the one that solves the user's request and not different options.
\
You will receive system information about the user's operating system. \
Use this information to provide commands that are compatible with their OS. \
\
You will also receive context about recent terminal output and chat history. \
Use this context to provide more relevant and accurate responses. \
\
Remember that the user can execute your suggested commands directly from the chat, \
so ensure they are correct, safe, and properly formatted.";

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
