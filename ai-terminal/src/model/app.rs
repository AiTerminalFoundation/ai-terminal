use std::env;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};

use crate::config::{
    AI_INSTRUCTIONS, AI_WELCOME_MESSAGE, DEFAULT_OLLAMA_MODEL, DEFAULT_PANEL_RATIO,
    TERMINAL_INSTRUCTIONS, WINDOW_WIDTH, WINDOW_HEIGHT, FocusTarget,
};
use crate::model::{CommandStatus, Panel};

#[derive(Clone, Debug)]
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

    // Change the command_receiver to use Arc to make it cloneable
    pub command_receiver: Option<(Arc<Mutex<mpsc::Receiver<String>>>, usize, String, Vec<String>)>,

    // Password mode
    pub password_mode: bool,
}

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
            // Panel management
            panel_ratio: DEFAULT_PANEL_RATIO,
            is_resizing: false,
            window_width: WINDOW_WIDTH as f32,
            window_height: WINDOW_HEIGHT as f32,
            // Initialize scroll state
            terminal_scroll: 0,
            assistant_scroll: 0,
            // Initialize command status tracking
            command_status,
            // Initialize command history
            command_history: Vec::new(),
            command_history_index: None,
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
            // Focus target
            focus: FocusTarget::Terminal,
            // Change the command_receiver to use Arc to make it cloneable
            command_receiver: None,
            // Password mode
            password_mode: false,
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
