use iced::Command;
use iced::widget::text_input;
use std::time::Duration;

use crate::model::{App as AppState};
use crate::config::keyboard::FocusTarget;
use crate::ui::messages::Message;
use crate::ui::panel_state::{TerminalPanelState, AiPanelState};
use crate::ui::ollama_client;
use crate::ollama::{api, commands};
use crate::terminal::utils;
use crate::ui::components;

// Constants for input IDs
pub const TERMINAL_INPUT_ID: &str = "terminal_input";
pub const AI_INPUT_ID: &str = "ai_input";

// Handler for Terminal Input message
pub fn handle_terminal_input(
    terminal_panel: &mut TerminalPanelState,
    terminal_input: &mut String,
    focus: &mut FocusTarget,
    value: String,
    suggestions: &mut Vec<String>,
    suggestion_index: &mut usize,
) -> Command<Message> {
    println!("[handlers.rs] Received TerminalInput message with value: '{}'", value);
    println!("[handlers.rs] Current terminal_input before update: '{}'", terminal_input);
    
    // Update the input string
    *terminal_input = value.clone();
    
    println!("[handlers.rs] Current terminal_input after update: '{}'", terminal_input);
    
    // Clear any existing suggestions when input changes
    suggestions.clear();
    *suggestion_index = 0;
    
    // Update the panel with the new input value
    terminal_panel.input = terminal_input.clone();
    
    // Ensure terminal focus is set
    if *focus == FocusTarget::Terminal {
        println!("[handlers.rs] Setting terminal_focus to true");
        terminal_panel.set_terminal_focus(true);
    }
    
    println!("[handlers.rs] Focus set to {:?}", focus);
    
    terminal_panel.panel.update_input(terminal_input.clone());
    
    Command::none()
}

// Handler for Execute Command message
pub fn handle_execute_command(
    app_state: &mut AppState,
    terminal_input: &mut String,
    terminal_panel: &mut TerminalPanelState,
    suggestions: &mut Vec<String>,
    suggestion_index: &mut usize,
    focus: &FocusTarget,
    _search_mode: bool,
) -> Command<Message> {
    println!("[handlers.rs] Execute command message received: '{}'", terminal_input);
    
    if terminal_input.is_empty() {
        // Even if no command, ensure focus remains on terminal input
        return text_input::focus(text_input::Id::new(TERMINAL_INPUT_ID));
    }
    
    println!("[handlers.rs] Executing command: '{}'", terminal_input);
    app_state.input = terminal_input.clone();

    // Start command execution
    app_state.execute_command();
    terminal_input.clear();
    
    // Force an immediate UI update to show command output right away
    terminal_panel.recreate(
        app_state.clone(), 
        terminal_input.clone(),
        focus.clone()
    );
    
    // Reset suggestion state
    suggestions.clear();
    *suggestion_index = 0;
    
    // Add slight delay before scrolling to improve smoothness
    let scroll_cmd = components::scrollable_container::scroll_to_bottom();
    
    // Keep focus on the terminal input field after execution
    let focus_cmd = text_input::focus(text_input::Id::new(TERMINAL_INPUT_ID));
    
    Command::batch(vec![
        Command::perform(async {}, |_| Message::NoOp),
        scroll_cmd,
        focus_cmd,
        // Add an immediate check for command output to display results faster
        Command::perform(async {}, |_| Message::CheckCommandOutput),
        // Schedule additional checks shortly after
        Command::perform(async {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }, |_| Message::CheckCommandOutput),
        Command::perform(async {
            tokio::time::sleep(Duration::from_millis(30)).await;
        }, |_| Message::CheckCommandOutput),
        Command::perform(async {
            tokio::time::sleep(Duration::from_millis(60)).await;
        }, |_| Message::CheckCommandOutput),
    ])
}

// Handler for Process AI Query message
pub fn handle_process_ai_query(
    app_state: &mut AppState,
    ai_input: &mut String,
) -> Command<Message> {
    if ai_input.is_empty() {
        return Command::none();
    }
    
    let query = ai_input.clone();
    ai_input.clear();

    // Add query to output
    let formatted_query = format!("> {}", query);
    app_state.ai_output.push(formatted_query.clone());

    // Check if the input is a command
    if query.starts_with('/') {
        let parts: Vec<&str> = query.split_whitespace().collect();
        let cmd = parts[0];

        match cmd {
            "/models" => {
                println!("Processing /models command");
                app_state.ai_output.push("🔍 Fetching models...".to_string());
                Command::perform(
                    async move {
                        println!("Fetching models from Ollama...");
                        match api::list_models().await {
                            Ok(models) => {
                                println!("Successfully fetched models: {:?}", models);
                                Ok(models)
                            },
                            Err(e) => {
                                println!("Error fetching models: {}", e);
                                Err(format!("Error listing models: {}", e))
                            }
                        }
                    },
                    |result| {
                        println!("Processing models result: {:?}", result);
                        match result {
                            Ok(models) => {
                                let response = format!(
                                    "Available models:\n{}",
                                    models.iter()
                                        .map(|model| format!("- {}", model))
                                        .collect::<Vec<_>>()
                                        .join("\n")
                                );
                                println!("Formatted response: {}", response);
                                Message::OllamaResponse(Ok(response))
                            },
                            Err(e) => {
                                println!("Error response: {}", e);
                                Message::OllamaResponse(Err(e))
                            }
                        }
                    }
                )
            }
            _ => {
                // Handle other commands synchronously
                commands::process_ai_command(app_state, &query);
                Command::none()
            }
        }
    } else {
        app_state.ai_output.push("Thinking...".to_string());

        // Create the context for Ollama
        let message_with_context = ollama_client::create_ollama_context(app_state, &query);
        let model = app_state.ollama_model.clone();

        println!("Sending chat query to Ollama with model: {}", model);
        // First check if Ollama is running
        Command::perform(
            async move {
                println!("Checking if Ollama is running...");
                match api::list_models().await {
                    Ok(_) => {
                        println!("Ollama is running, sending prompt...");
                        match api::send_prompt(&model, &message_with_context).await {
                            Ok(response) => {
                                println!("Got response from Ollama");
                                Ok(response)
                            },
                            Err(e) => {
                                println!("Error from Ollama: {}", e);
                                Err(e)
                            }
                        }
                    }
                    Err(_) => {
                        println!("Ollama is not running");
                        Err("Error: Ollama is not running. Please start Ollama and try again.".to_string())
                    }
                }
            },
            Message::OllamaResponse
        )
    }
}

// Handler for Ollama Response message
pub fn handle_ollama_response(
    app_state: &mut AppState,
    terminal_input: &mut String,
    terminal_panel: &mut TerminalPanelState,
    focus: &FocusTarget,
    result: Result<String, String>
) -> Command<Message> {
    println!("Handling OllamaResponse message");
    match result {
        Ok(response) => {
            println!("Processing successful response");
            // Remove thinking message
            if let Some(last) = app_state.ai_output.last() {
                if last.contains("Thinking") || last.contains("🔍 Fetching") {
                    println!("Removing thinking/fetching message");
                    app_state.ai_output.pop();
                }
            }

            // Extract commands from the response
            let extracted_command = utils::extract_commands(&response);
            
            // ALWAYS add the AI's full response to the chat output
            println!("Adding response to output: {}", response);
            app_state.ai_output.push(response.clone());
            
            if !extracted_command.is_empty() {
                println!("Extracted command: {}", extracted_command);
                
                // Add the extracted command as a separate message in AI output with an indicator
                app_state.ai_output.push(format!("📋 Command: {}", extracted_command));
                
                // Set the command for execution
                app_state.last_ai_command = Some(extracted_command.clone());
                *terminal_input = extracted_command;
                
                // Recreate the terminal panel to ensure terminal input is visible
                terminal_panel.recreate(
                    app_state.clone(),
                    terminal_input.clone(),
                    focus.clone()
                );
                
                // Make sure terminal focus state is properly set
                terminal_panel.panel.set_terminal_focus(true);
                terminal_panel.focus = true;
                
                // Return commands to focus terminal input and execute UI refresh
                return Command::batch(vec![
                    Command::perform(async {}, |_| Message::NoOp),
                    components::scrollable_container::scroll_to_bottom(),
                    text_input::focus(text_input::Id::new(TERMINAL_INPUT_ID)),
                    text_input::move_cursor_to_end(text_input::Id::new(TERMINAL_INPUT_ID))
                ]);
            }

            // If no command was extracted, just scroll to bottom
            // Add slight delay before scrolling to improve smoothness
            let scroll_cmd = components::scrollable_container::scroll_to_bottom();
            Command::batch(vec![
                Command::perform(async {}, |_| Message::NoOp),
                scroll_cmd,
            ])
        }
        Err(error) => {
            println!("Processing error response: {}", error);
            // Remove thinking message
            if let Some(last) = app_state.ai_output.last() {
                if last.contains("Thinking") || last.contains("🔍 Fetching") {
                    println!("Removing thinking/fetching message");
                    app_state.ai_output.pop();
                }
            }
            app_state.ai_output.push(format!("Error: {}", error));

            // Since we had an error response, reset terminal panel to ensure proper UI state
            terminal_panel.recreate(
                app_state.clone(),
                terminal_input.clone(),
                focus.clone()
            );
            
            // Make sure terminal focus state is properly set
            terminal_panel.panel.set_terminal_focus(true);
            terminal_panel.focus = true;

            // Add slight delay before scrolling to improve smoothness
            let scroll_cmd = components::scrollable_container::scroll_to_bottom();
            Command::batch(vec![
                Command::perform(async {}, |_| Message::NoOp),
                scroll_cmd,
                text_input::focus(text_input::Id::new(TERMINAL_INPUT_ID)),
                text_input::move_cursor_to_end(text_input::Id::new(TERMINAL_INPUT_ID)),
            ])
        }
    }
}

// Handler for Poll Command Output message
pub fn handle_poll_command_output(
    app_state: &mut AppState,
    terminal_panel: &mut TerminalPanelState,
    terminal_input: &String,
    focus: &FocusTarget,
) -> Command<Message> {
    if let Some(cmd) = app_state.poll_command_output() {
        // Always recreate the terminal panel to force a view update
        terminal_panel.recreate(
            app_state.clone(),
            terminal_input.clone(), 
            focus.clone()
        );
        
        // Make sure terminal focus state is preserved
        terminal_panel.panel.set_terminal_focus(terminal_panel.focus);
        
        cmd
    } else {
        Command::none()
    }
}

// Handler for Check Command Output message
pub fn handle_check_command_output(
    app_state: &mut AppState,
    terminal_panel: &mut TerminalPanelState,
    terminal_input: &String,
    focus: &FocusTarget,
) -> Command<Message> {
    // Force an immediate check for command output and ensure UI updates
    if let Some(cmd) = app_state.poll_command_output() {
        // Command produced new output
        // Force terminal panel refresh
        terminal_panel.recreate(
            app_state.clone(),
            terminal_input.clone(), 
            focus.clone()
        );
        
        // Make sure terminal focus state is preserved
        terminal_panel.panel.set_terminal_focus(terminal_panel.focus);
        
        cmd
    } else {
        terminal_panel.recreate(
            app_state.clone(),
            terminal_input.clone(), 
            focus.clone()
        );
        
        // Make sure terminal focus state is preserved
        terminal_panel.panel.set_terminal_focus(terminal_panel.focus);
        
        // Always return a command to force UI refresh for streaming commands
        components::scrollable_container::scroll_to_bottom()
    }
}

// Handler for Search Input message
pub fn handle_search_input(
    app_state: &AppState,
    terminal_panel: &mut TerminalPanelState,
    terminal_input: &String,
    focus: &FocusTarget,
    input: String,
) -> Command<Message> {
    println!("[handlers.rs] SearchInput message received with value: '{}'", input);
    terminal_panel.search_input = input.clone();
    terminal_panel.search_index = 0;
    terminal_panel.search_matches = Vec::new();
    
    // When typing in search, we're focused on search
    terminal_panel.focus = false;
    println!("[handlers.rs] Setting terminal_focus to false (search has focus)");
    terminal_panel.panel.set_terminal_focus(false);
    
    if !input.is_empty() {
        // Find all matches in the terminal output
        let visible_output = if app_state.output.len() > 2000 {
            app_state.output.iter().skip(app_state.output.len() - 2000).cloned().collect()
        } else {
            app_state.output.clone()
        };
        
        // Count all matches in each line
        for (i, line) in visible_output.iter().enumerate() {
            let mut pos = 0;
            while let Some(pos_found) = line[pos..].to_lowercase().find(&input.to_lowercase()) {
                terminal_panel.search_matches.push(i);
                pos += pos_found + 1;
            }
        }
        println!("[handlers.rs] Found {} matches for search query", terminal_panel.search_matches.len());
    }
    
    // Create a new terminal panel with updated search input
    terminal_panel.recreate(
        app_state.clone(),
        terminal_input.clone(), 
        focus.clone()
    );
    
    // Update search count in terminal panel
    terminal_panel.panel.update_search_input(input);
    terminal_panel.panel.update_search_count(terminal_panel.search_index, terminal_panel.search_matches.len());
    
    // Make sure terminal_focus is false since we're in search
    terminal_panel.panel.set_terminal_focus(false);
    
    // Make sure search input keeps focus
    println!("[handlers.rs] Focusing search input after SearchInput message");
    text_input::focus(text_input::Id::new("search_input"))
}

// Handler for Toggle Search message
pub fn handle_toggle_search(
    app_state: &AppState,
    terminal_panel: &mut TerminalPanelState,
    terminal_input: &String,
    focus: &FocusTarget,
) -> Command<Message> {
    // Toggle search mode
    terminal_panel.search_mode = !terminal_panel.search_mode;
    
    if terminal_panel.search_mode {
        // When turning on search mode:
        // 1. Focus should go to search bar
        terminal_panel.focus = false;
        
        // 2. Create a new terminal panel with search mode enabled
        terminal_panel.recreate(
            app_state.clone(),
            terminal_input.clone(), 
            focus.clone()
        );
        
        // Make sure terminal panel has correct focus state
        terminal_panel.panel.set_terminal_focus(false);
        
        // Clear search state
        terminal_panel.search_input.clear();
        terminal_panel.search_matches.clear();
        terminal_panel.search_index = 0;
        
        // Focus the search input when toggling search on
        println!("[handlers.rs] Toggling search ON, focusing search input");
        text_input::focus(text_input::Id::new("search_input"))
    } else {
        // When turning off search mode:
        terminal_panel.focus = true;
        
        // Create a new terminal panel with search mode disabled
        terminal_panel.recreate(
            app_state.clone(),
            terminal_input.clone(), 
            focus.clone()
        );
        
        // Make sure terminal panel has correct focus state
        terminal_panel.panel.set_terminal_focus(true);
        
        // Focus back on terminal input when search is closed
        println!("[handlers.rs] Toggling search OFF, focusing terminal input");
        text_input::focus(text_input::Id::new(TERMINAL_INPUT_ID))
    }
} 