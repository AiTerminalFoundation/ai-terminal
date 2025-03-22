use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use crate::config::FocusTarget;

pub mod app;

// Ollama API models
#[derive(Serialize)]
pub struct OllamaRequest {
    pub model: String,
    pub prompt: String,
    pub stream: bool,
    pub system: Option<String>,
}

#[derive(Deserialize)]
pub struct OllamaResponse {
    pub response: String,
}

#[derive(Deserialize)]
pub struct OllamaModel {
    pub name: String,
}

#[derive(Deserialize)]
pub struct OllamaModelList {
    pub models: Vec<OllamaModel>,
}

// Application state models
#[derive(PartialEq, Clone, Copy, Debug)]
pub enum Panel {
    Terminal,
    Assistant,
}

#[derive(Clone, Debug)]
pub enum CommandStatus {
    Success,
    Failure,
    Running,
}

// Main application state
#[derive(Clone)]
pub struct App {
    // Terminal panel state
    pub input: String,
    pub output: Vec<String>,
    pub cursor_position: usize,
    pub current_dir: PathBuf,

    // AI assistant panel state
    pub ai_input: String,
    pub ai_output: Vec<String>,
    pub ai_cursor_position: usize,

    // Panel management
    pub active_panel: Panel,
    pub panel_ratio: u32,
    pub is_resizing: bool,
    pub window_width: f32,
    pub window_height: f32,

    // Scroll state
    pub terminal_scroll: usize,
    pub assistant_scroll: usize,

    // Command status tracking
    pub command_status: Vec<CommandStatus>,

    // Command history
    pub command_history: Vec<String>,
    pub command_history_index: Option<usize>,

    // Autocomplete suggestions
    pub autocomplete_suggestions: Vec<String>,
    pub autocomplete_index: Option<usize>,

    // Ollama integration
    pub ollama_model: String,
    pub ollama_thinking: bool,

    // Extracted commands from AI responses
    pub extracted_commands: Vec<(usize, String)>, // (line_index, command)

    // Most recent command from AI assistant
    pub last_ai_command: Option<String>,

    // Last terminal command and output for context
    pub last_terminal_context: Option<(String, Vec<String>)>, // (command, output)

    // System information
    pub os_info: String,

    // Auto-execute commands (disabled by default)
    pub auto_execute_commands: bool,

    // Focus target
    pub focus: FocusTarget,
}
