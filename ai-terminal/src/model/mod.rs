use std::path::PathBuf;
use serde::{Deserialize, Serialize};

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
#[derive(PartialEq)]
pub enum Panel {
    Terminal,
    Assistant,
}

pub enum CommandStatus {
    Success,
    Failure,
    Running,
}

// Main application state
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
    pub panel_ratio: u16,
    
    // Mouse drag state
    pub is_dragging: bool,
    
    // Layout information for mouse interaction
    pub terminal_area: Option<ratatui::layout::Rect>,
    pub assistant_area: Option<ratatui::layout::Rect>,
    pub divider_x: Option<u16>,
    
    // Scroll state
    pub terminal_scroll: usize,
    pub assistant_scroll: usize,
    
    // Command status tracking
    pub command_status: Vec<CommandStatus>,
    
    // Command history
    pub command_history: Vec<String>,
    pub history_position: Option<usize>,

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
    
    // Auto-execute commands
    pub auto_execute_commands: bool,
} 