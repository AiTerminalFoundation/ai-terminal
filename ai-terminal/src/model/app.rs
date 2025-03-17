use std::env;
use std::path::PathBuf;

use crate::config::{
    AI_INSTRUCTIONS, AI_WELCOME_MESSAGE, DEFAULT_OLLAMA_MODEL, DEFAULT_PANEL_RATIO,
    TERMINAL_INSTRUCTIONS,
};
use crate::model::{CommandStatus, Panel};

impl crate::model::App {
    pub fn new() -> Self {
        // Always start at the root directory
        let current_dir = PathBuf::from("/");

        // Set the current working directory to the root
        let _ = env::set_current_dir(&current_dir);

        // Detect OS information
        let os_info = detect_os();

        // Initial output messages
        let mut initial_output = vec![
            format!("Operating System: {}", os_info),
        ];

        // Add terminal instructions
        for instruction in TERMINAL_INSTRUCTIONS.iter() {
            initial_output.push(instruction.to_string());
        }

        // Initial AI output messages
        let mut initial_ai_output = vec![AI_WELCOME_MESSAGE.to_string()];

        // Add AI instructions
        for instruction in AI_INSTRUCTIONS.iter() {
            initial_ai_output.push(instruction.to_string());
        }

        // Initialize command status for any commands in the initial output
        let mut command_status = Vec::new();
        for line in &initial_output {
            if line.starts_with("> ") {
                command_status.push(CommandStatus::Success);
            }
        }

        crate::model::App {
            input: String::new(),
            output: initial_output,
            cursor_position: 0,
            current_dir,
            // Initialize AI assistant fields
            ai_input: String::new(),
            ai_output: initial_ai_output,
            ai_cursor_position: 0,
            active_panel: Panel::Terminal,
            // Default to 50% split
            panel_ratio: DEFAULT_PANEL_RATIO,
            is_resizing: false,
            // Initialize scroll state
            terminal_scroll: 0,
            assistant_scroll: 0,
            // Initialize command status tracking
            command_status,
            // Initialize command history
            command_history: Vec::new(),
            history_position: None,
            // Initialize autocomplete
            autocomplete_suggestions: Vec::new(),
            autocomplete_index: None,
            // Ollama integration
            ollama_model: DEFAULT_OLLAMA_MODEL.to_string(),
            ollama_thinking: false,
            // Extracted commands from AI responses
            extracted_commands: Vec::new(),
            // Most recent command from AI assistant
            last_ai_command: None,
            // Last terminal command and output for context
            last_terminal_context: None,
            // System information
            os_info,
            // Auto-execute commands (disabled by default)
            auto_execute_commands: false,
        }
    }
}

// Helper function to detect OS information
fn detect_os() -> String {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    let family = std::env::consts::FAMILY;
    
    format!("{} ({}, {})", os, arch, family)
}
