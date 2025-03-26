use iced::widget::{container, row, text_input};
use iced::{Application, Command, Element, Length, Theme};
use iced::keyboard::Event as KeyEvent;

use crate::model::{App as AppState, Panel};
use crate::ui::components::{drag_handle, ShortcutsModal};
use crate::ui::theme::DraculaTheme;
use crate::config::keyboard::{FocusTarget};
use crate::ui::messages::Message;
use crate::ui::panel_state::{TerminalPanelState, AiPanelState, PanelViews};
use crate::ui::subscriptions;
use crate::ui::handlers;

// Add these constants at the top of the file
const TERMINAL_INPUT_ID: &str = "terminal_input";
const AI_INPUT_ID: &str = "ai_input";

pub struct TerminalApp {
    state: AppState,
    terminal_input: String,
    ai_input: String,
    focus: FocusTarget,
    current_suggestions: Vec<String>,
    suggestion_index: usize,
    password_buffer: String,
    password_mode: bool,
    show_shortcuts_modal: bool,
    terminal_panel: TerminalPanelState,
    ai_panel: AiPanelState,
}

impl Application for TerminalApp {
    type Executor = iced::executor::Default;
    type Message = Message;
    type Theme = Theme;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
        println!("[app.rs] Creating new TerminalApp");
        let app_state = AppState::new();
        
        // Create the initial panel states
        let terminal_panel = TerminalPanelState::new(
            app_state.clone(), 
            String::new(), 
            FocusTarget::Terminal,
            false
        );
        
        let ai_panel = AiPanelState::new(
            app_state.clone(),
            String::new(),
            FocusTarget::Terminal
        );
        
        // Create a batch of commands to initialize the app
        let init_commands = Command::batch(vec![
            // Force focus on terminal input at startup
            text_input::focus(text_input::Id::new(handlers::TERMINAL_INPUT_ID)),
            // Move cursor to end to ensure visibility
            text_input::move_cursor_to_end(text_input::Id::new(handlers::TERMINAL_INPUT_ID))
        ]);
        
        println!("[app.rs] Initializing with focus on terminal input");
        
        (
            Self {
                state: app_state,
                terminal_input: String::new(),
                ai_input: String::new(),
                focus: FocusTarget::Terminal,
                current_suggestions: Vec::new(),
                suggestion_index: 0,
                password_buffer: String::new(),
                password_mode: false,
                show_shortcuts_modal: false,
                terminal_panel,
                ai_panel,
            },
            // Initialize focus at startup
            init_commands
        )
    }

    fn title(&self) -> String {
        String::from("AI Terminal")
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        if let Some(command) = self.state.poll_command_output() {
            return command;
        }

        let cmd = match message {
            Message::TerminalInput(value) => {
                handlers::handle_terminal_input(
                    &mut self.terminal_panel,
                    &mut self.terminal_input,
                    &mut self.focus,
                    value,
                    &mut self.current_suggestions,
                    &mut self.suggestion_index,
                )
            }
            Message::AIInput(value) => {
                // Update the input string
                self.ai_input = value.clone();
                // Update the AI panel state
                self.ai_panel.update_input(value);
                Command::none()
            }
            Message::ExecuteCommand => {
                let search_mode = self.terminal_panel.search_mode;
                handlers::handle_execute_command(
                    &mut self.state,
                    &mut self.terminal_input,
                    &mut self.terminal_panel,
                    &mut self.current_suggestions,
                    &mut self.suggestion_index,
                    &self.focus,
                    search_mode,
                )
            }
            Message::ProcessAIQuery => {
                handlers::handle_process_ai_query(
                    &mut self.state,
                    &mut self.ai_input,
                )
            }
            Message::OllamaResponse(result) => {
                handlers::handle_ollama_response(
                    &mut self.state,
                    &mut self.terminal_input,
                    &mut self.terminal_panel,
                    &self.focus,
                    result,
                )
            }
            Message::PollCommandOutput => {
                handlers::handle_poll_command_output(
                    &mut self.state,
                    &mut self.terminal_panel,
                    &self.terminal_input,
                    &self.focus,
                )
            }
            Message::CheckCommandOutput => {
                handlers::handle_check_command_output(
                    &mut self.state,
                    &mut self.terminal_panel,
                    &self.terminal_input,
                    &self.focus,
                )
            }
            Message::SearchInput(input) => {
                handlers::handle_search_input(
                    &self.state,
                    &mut self.terminal_panel,
                    &self.terminal_input,
                    &self.focus,
                    input,
                )
            }
            Message::ToggleSearch => {
                handlers::handle_toggle_search(
                    &self.state,
                    &mut self.terminal_panel,
                    &self.terminal_input,
                    &self.focus,
                )
            }
            Message::SwitchPanel => {
                self.state.active_panel = match self.state.active_panel {
                    Panel::Terminal => Panel::Assistant,
                    Panel::Assistant => Panel::Terminal,
                };
                Command::none()
            }
            Message::ResizeLeft => {
                let new_ratio = (self.state.panel_ratio - 5).max(20);
                self.state.panel_ratio = new_ratio;
                Command::none()
            }
            Message::ResizeRight => {
                let new_ratio = (self.state.panel_ratio + 5).min(80);
                self.state.panel_ratio = new_ratio;
                Command::none()
            }
            Message::HistoryUp => {
                if self.focus == FocusTarget::Terminal {
                    let need_update = if let Some(current_index) = self.state.command_history_index {
                        // Already navigating history, try to go to older command
                        if current_index > 0 {
                            self.state.command_history_index = Some(current_index - 1);
                            if let Some(command) = self.state.command_history.get(current_index - 1) {
                                self.terminal_input = command.clone();
                                true
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    } else if !self.state.command_history.is_empty() {
                        // Start from the newest command (last in the vector)
                        let last_idx = self.state.command_history.len() - 1;
                        self.state.command_history_index = Some(last_idx);
                        if let Some(command) = self.state.command_history.last() {
                            self.terminal_input = command.clone();
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    };
                    
                    if need_update {
                        println!("[app.rs] HistoryUp: Updated terminal input to: '{}'", self.terminal_input);
                        
                        // Recreate terminal panel with updated input
                        self.terminal_panel.recreate(
                            self.state.clone(),
                            self.terminal_input.clone(),
                            self.focus.clone()
                        );
                        
                        // Return a command to focus the terminal input and move cursor to end
                        return Command::batch(vec![
                            text_input::focus(text_input::Id::new(handlers::TERMINAL_INPUT_ID)),
                            text_input::move_cursor_to_end(text_input::Id::new(handlers::TERMINAL_INPUT_ID))
                        ]);
                    }
                }
                Command::none()
            }
            Message::HistoryDown => {
                if self.focus == FocusTarget::Terminal {
                    let mut need_update = false;
                    
                    if let Some(current_index) = self.state.command_history_index {
                        // Move to newer command
                        if current_index < self.state.command_history.len() - 1 {
                            self.state.command_history_index = Some(current_index + 1);
                            if let Some(command) = self.state.command_history.get(current_index + 1) {
                                self.terminal_input = command.clone();
                                need_update = true;
                            }
                        } else {
                            // At newest command, clear input
                            self.state.command_history_index = None;
                            self.terminal_input.clear();
                            need_update = true;
                        }
                    }
                    
                    if need_update {
                        println!("[app.rs] HistoryDown: Updated terminal input to: '{}'", self.terminal_input);
                        
                        // Recreate terminal panel with updated input
                        self.terminal_panel.recreate(
                            self.state.clone(),
                            self.terminal_input.clone(),
                            self.focus.clone()
                        );
                        
                        // Return a command to focus the terminal input and move cursor to end
                        return Command::batch(vec![
                            text_input::focus(text_input::Id::new(handlers::TERMINAL_INPUT_ID)),
                            text_input::move_cursor_to_end(text_input::Id::new(handlers::TERMINAL_INPUT_ID))
                        ]);
                    }
                }
                Command::none()
            }
            Message::TildePressed => {
                if self.state.active_panel == Panel::Terminal {
                    self.terminal_input.push('~');
                } else {
                    self.ai_input.push('~');
                }
                Command::none()
            }
            Message::TerminalScroll(viewport) => {
                // Only update the scroll position if we're not actively trying to scroll to the bottom
                self.state.terminal_scroll = viewport.relative_offset().y as usize;
                Command::none()
            }
            Message::ToggleFocus => {
                self.focus = match self.focus {
                    FocusTarget::Terminal => FocusTarget::AiChat,
                    FocusTarget::AiChat => FocusTarget::Terminal,
                };
                // Return a command to focus the correct input
                match self.focus {
                    FocusTarget::Terminal => text_input::focus(text_input::Id::new(handlers::TERMINAL_INPUT_ID)),
                    FocusTarget::AiChat => text_input::focus(text_input::Id::new(handlers::AI_INPUT_ID)),
                }
            }
            Message::ScrollToBottom => {
                // Only scroll to bottom when explicitly requested
                crate::ui::components::scrollable_container::scroll_to_bottom()
            }
            Message::UpdateTerminalOutput(line) => {
                self.state.output.push(line);
                crate::ui::components::scrollable_container::scroll_to_bottom()
            }
            Message::SendInput(input) => {
                if self.state.password_mode {
                    self.state.send_input(input);
                    self.terminal_input.clear();  // Clear the input after sending password
                }
                Command::none()
            }
            Message::TabPressed => {
                println!("[app.rs] Tab pressed message received for autocomplete");
                
                // Tab should now only handle autocomplete, not context switching
                if self.focus == FocusTarget::Terminal {
                    println!("[app.rs] Getting autocomplete suggestions");

                    // If we don't have any suggestions yet, get them
                    if self.current_suggestions.is_empty() {
                        println!("[app.rs] Getting new suggestions");
                        self.state.input = self.terminal_input.clone();
                        self.current_suggestions = self.state.get_autocomplete_suggestions();
                        println!("[app.rs] Got suggestions: {:?}", self.current_suggestions);
                    } 
                    
                    // Apply suggestions if available
                    if !self.current_suggestions.is_empty() {
                        // We have suggestions, move to the next one if there are multiple
                        if self.current_suggestions.len() > 1 {
                            self.suggestion_index = (self.suggestion_index + 1) % self.current_suggestions.len();
                            println!("[app.rs] Moving to suggestion {}/{}", 
                                self.suggestion_index + 1, self.current_suggestions.len());
                        }

                        // Apply the current suggestion
                        let suggestion = self.current_suggestions[self.suggestion_index].clone();
                        println!("[app.rs] Using suggestion: {}", suggestion);
                        self.terminal_input = suggestion;
                        
                        // Recreate terminal panel with updated input
                        self.terminal_panel.recreate(
                            self.state.clone(),
                            self.terminal_input.clone(),
                            self.focus.clone()
                        );

                        // Move cursor to end after applying suggestion
                        return Command::batch(vec![
                            text_input::focus(text_input::Id::new(handlers::TERMINAL_INPUT_ID)),
                            text_input::move_cursor_to_end(text_input::Id::new(handlers::TERMINAL_INPUT_ID))
                        ]);
                    }
                    
                    // Even if no suggestions, ensure focus is on terminal input
                    return text_input::focus(text_input::Id::new(handlers::TERMINAL_INPUT_ID));
                }
                
                // If not on terminal, do nothing for Tab
                Command::none()
            }
            Message::NoOp => {
                Command::none()
            }
            Message::PasswordInput(password) => {
                // Store password temporarily (don't display it)
                self.password_buffer = password;
                Command::none()
            }
            Message::SubmitPassword => {
                // Send the password to the running command
                let password = std::mem::take(&mut self.password_buffer);
                self.state.send_input(password);
                Command::none()
            }
            Message::TerminateCommand => {
                if let Some(cmd) = self.state.terminate_running_command() {
                    cmd
                } else {
                    Command::none()
                }
            }
            Message::ToggleShortcutsModal => {
                self.show_shortcuts_modal = !self.show_shortcuts_modal;
                Command::none()
            }
            Message::CopyToClipboard(content, _show_feedback) => {
                // Just copy to clipboard without feedback mechanism
                iced::clipboard::write(content)
            }
            Message::HandleCtrlC => {
                // Check if there's a running command first
                if self.state.command_receiver.is_some() {
                    // There's a running command, terminate it
                    if let Some(cmd) = self.state.terminate_running_command() {
                        cmd
                    } else {
                        Command::none()
                    }
                } else {
                    // No running command, try to get selected text from OS clipboard
                    if let Ok(mut clipboard) = arboard::Clipboard::new() {
                        if let Ok(text) = clipboard.get_text() {
                            // Text was copied via OS selection mechanisms, we don't need to do anything
                            // The OS clipboard already has the text
                            println!("Text copied: {}", if text.len() > 20 { 
                                format!("{}...", &text[..20]) 
                            } else { 
                                text.clone() 
                            });
                        }
                    }
                    Command::none()
                }
            }
            Message::SearchNext => {
                if let Some(index) = self.terminal_panel.search_matches.get(self.terminal_panel.search_index) {
                    let visible_output = if self.state.output.len() > 2000 {
                        self.state.output.iter().skip(self.state.output.len() - 2000).cloned().collect()
                    } else {
                        self.state.output.clone()
                    };
                    self.terminal_input = visible_output[*index].clone();
                    self.terminal_panel.search_index = (self.terminal_panel.search_index + 1) % self.terminal_panel.search_matches.len();
                    // Update search count in terminal panel
                    self.terminal_panel.update_search_count(self.terminal_panel.search_index, self.terminal_panel.search_matches.len());
                }
                Command::none()
            }
            Message::SearchPrev => {
                if let Some(index) = self.terminal_panel.search_matches.get(self.terminal_panel.search_index) {
                    let visible_output = if self.state.output.len() > 2000 {
                        self.state.output.iter().skip(self.state.output.len() - 2000).cloned().collect()
                    } else {
                        self.state.output.clone()
                    };
                    self.terminal_input = visible_output[*index].clone();
                    self.terminal_panel.search_index = if self.terminal_panel.search_index == 0 { 
                        self.terminal_panel.search_matches.len() - 1 
                    } else { 
                        self.terminal_panel.search_index - 1 
                    };
                    // Update search count in terminal panel
                    self.terminal_panel.update_search_count(self.terminal_panel.search_index, self.terminal_panel.search_matches.len());
                }
                Command::none()
            }
            Message::ClearSearch => {
                self.terminal_panel.search_input.clear();
                self.terminal_panel.search_matches.clear();
                self.terminal_panel.search_index = 0;
                
                // Recreate terminal panel with cleared search
                self.terminal_panel.recreate(
                    self.state.clone(),
                    self.terminal_input.clone(), 
                    self.focus.clone()
                );
                
                // Update search count in terminal panel
                self.terminal_panel.update_search_count(0, 0);
                
                // Focus remains on search but terminal_focus should be false
                self.terminal_panel.set_terminal_focus(false);
                // Focus back on the search input
                text_input::focus(text_input::Id::new("search_input"))
            }
            Message::ToggleTerminalSearchFocus => {
                // This is now triggered by Ctrl+Tab or Escape
                println!("[app.rs] ToggleTerminalSearchFocus triggered (Ctrl+Tab)");
                
                // Only toggle focus between terminal and search input when search is active
                if self.terminal_panel.search_mode {
                    // Toggle terminal focus - if currently on search, switch to terminal and vice versa
                    self.terminal_panel.focus = !self.terminal_panel.focus;
                    // Sync the focus state to the panel
                    self.terminal_panel.set_terminal_focus(self.terminal_panel.focus);
                    
                    if self.terminal_panel.focus {
                        // Focus the terminal input and ensure global focus is terminal
                        self.focus = FocusTarget::Terminal;
                        println!("[app.rs] Context switch: Focusing terminal input");
                        return text_input::focus(text_input::Id::new(handlers::TERMINAL_INPUT_ID));
                    } else {
                        // Focus the search input
                        println!("[app.rs] Context switch: Focusing search input");
                        return text_input::focus(text_input::Id::new("search_input"));
                    }
                } else {
                    // If search is not active, focus the terminal input
                    self.terminal_panel.set_terminal_focus(true);
                    self.focus = FocusTarget::Terminal;
                    println!("[app.rs] Context switch: Focusing terminal input (search inactive)");
                    return text_input::focus(text_input::Id::new(handlers::TERMINAL_INPUT_ID));
                }
            }
        };

        // Update AI panel state after handling messages
        self.ai_panel.recreate(
            self.state.clone(),
            self.focus.clone(),
        );

        cmd
    }

    fn view(&self) -> Element<Message> {
        let views = self.create_panel_views();

        // Build the main content using the stored views
        let content = row![
            container(views.terminal)
                .width(Length::FillPortion(self.state.panel_ratio as u16))
                .height(Length::Fill)
                .style(DraculaTheme::container_style()),
            drag_handle(),
            container(views.ai)
                .width(Length::FillPortion((100 - self.state.panel_ratio) as u16))
                .height(Length::Fill)
                .style(DraculaTheme::container_style()),
        ]
        .height(Length::Fill);

        // If modal is visible, show it centered without a backdrop
        if self.show_shortcuts_modal {
            // Create a floating container for the modal
            container(
                container(ShortcutsModal::view())
                    .width(Length::Fixed(450.0))
                    .padding(20)
                    .style(DraculaTheme::modal_style())
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x()
            .center_y()
            .style(DraculaTheme::transparent_container_style())
            .into()
        } else {
            // Just show the normal content
            content.into()
        }
    }

    fn subscription(&self) -> iced::Subscription<Message> {
        // Create subscriptions for keyboard events and terminal polling
        subscriptions::create_subscriptions(self.state.command_receiver.is_some())
    }
}

impl TerminalApp {
    pub fn handle_input(&mut self, key_event: KeyEvent) {
        // Event handler for direct keyboard input
        // This is used for advanced terminal control
        let action = crate::config::keyboard::handle_keyboard_event(key_event);
        
        match action {
            crate::config::keyboard::ShortcutAction::TabAutocomplete => {
                println!("[app.rs] Tab key pressed in handle_input");
                if self.focus == FocusTarget::Terminal {
                    println!("[app.rs] Focus is on terminal, getting suggestions");
                    self.state.input = self.terminal_input.clone();
                    println!("[app.rs] Current input: {}", self.terminal_input);
                    let suggestions = self.state.get_autocomplete_suggestions();
                    println!("[app.rs] Got suggestions: {:?}", suggestions);
                    if !suggestions.is_empty() {
                        println!("[app.rs] Using first suggestion: {}", suggestions[0]);
                        self.terminal_input = suggestions[0].clone();
                    } else {
                        println!("[app.rs] No suggestions found");
                    }
                } else {
                    println!("[app.rs] Focus is not on terminal");
                }
                return;
            },
            crate::config::keyboard::ShortcutAction::ToggleFocus => {
                if crate::config::keyboard::handle_keyboard_shortcuts(key_event, &mut self.focus) {
                    return;
                }
            },
            _ => {}
        }
    }

    fn create_panel_views(&self) -> PanelViews<'_> {
        PanelViews {
            terminal: self.terminal_panel.view(),
            ai: self.ai_panel.view(),
        }
    }
}

#[derive(Debug, Clone)]
enum State {
    Ready,
    Waiting,
}
