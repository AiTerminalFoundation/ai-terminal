// Welcome messages
pub const WELCOME_MESSAGE: &str = "Welcome to AI Terminal!";

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
Always respond with valid terminal commands that solve the user's request. \
Format your response with a brief explanation followed by the command in a code block like this: \
```\ncommand\n```\n \
If multiple commands are needed, list them in sequence with explanations for each. \
If you're unsure or the request doesn't require a terminal command, explain why. \
\
You will receive system information about the user's operating system. \
Use this information to provide commands that are compatible with their OS. \
\
You may also receive context about the last terminal command and its output. \
Use this context to provide more relevant and accurate responses. \
When you see one operating system name, and 'Last terminal command:' followed by 'Output:', \
this is providing you with the context of what the user just did in their terminal. \
The actual user query follows after 'User query.'.";

// Help messages
pub const HELP_MESSAGES: [&str; 2] = ["Available commands:", "Features:"];

pub const HELP_COMMANDS: [&str; 5] = [
    "  /model <model_name> - Change the Ollama model",
    "  /help - Show this help message",
    "  /clear - Clear the chat history",
    "  /models - List available models (requires Ollama to be running)",
    "  /autoexec - Toggle automatic execution of commands",
];

pub const HELP_FEATURES: [&str; 1] =
    ["  - System information is provided to the AI for better command compatibility"];

// Error messages
pub const ERROR_FETCHING_MODELS: &str = "Error fetching models: ";
pub const OLLAMA_NOT_RUNNING: &str = "Make sure Ollama is running (http://localhost:11434)";
pub const OLLAMA_INSTALL_INSTRUCTIONS: &str = "You can install Ollama from https://ollama.ai";
pub const NO_MODELS_FOUND: &str = "No models found. You need to pull models first.";
pub const OLLAMA_PULL_INSTRUCTIONS: &str =
    "Run 'ollama pull llama3.2:latest' in the terminal to get started.";
