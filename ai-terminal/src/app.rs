use iced::widget::{column, container, row, text_input, scrollable, text};
use iced::{Application, Command, Element, Length, Theme, Font};
use iced::keyboard::{self, Event as KeyEvent};
use iced::event::Event;
use iced::subscription;
use std::time::Duration;

use crate::model::{App as AppState, Panel, CommandStatus};
use crate::ollama::{api, commands};
use crate::ui::components::{styled_text, drag_handle};
use crate::ui::theme::DraculaTheme;
use crate::terminal::utils;
use crate::config::keyboard::{FocusTarget, handle_keyboard_shortcuts};
use crate::ui::components;

// Add these constants at the top of the file
const TERMINAL_INPUT_ID: &str = "terminal_input";
const AI_INPUT_ID: &str = "ai_input";

#[derive(Debug, Clone)]
pub enum Message {
    TerminalInput(String),
    AIInput(String),
    ExecuteCommand,
    ProcessAIQuery,
    OllamaResponse(Result<String, String>),
    SwitchPanel,
    ResizeLeft,
    ResizeRight,
    HistoryUp,
    HistoryDown,
    TildePressed,
    TerminalScroll(scrollable::Viewport),
    ToggleFocus,
    ScrollToBottom,
    UpdateTerminalOutput(String),
    SendInput(String),
    PollCommandOutput,
    TabPressed,
    NoOp,
}

pub struct TerminalApp {
    state: AppState,
    terminal_input: String,
    ai_input: String,
    focus: FocusTarget,
    current_suggestions: Vec<String>,
    suggestion_index: usize,
}

impl Application for TerminalApp {
    type Message = Message;
    type Theme = Theme;
    type Executor = iced::executor::Default;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
        (
            Self {
                state: AppState::new(),
                terminal_input: String::new(),
                ai_input: String::new(),
                focus: FocusTarget::Terminal,
                current_suggestions: Vec::new(),
                suggestion_index: 0,
            },
            Command::none(),
        )
    }

    fn title(&self) -> String {
        String::from("AI Terminal")
    }

    fn subscription(&self) -> iced::Subscription<Message> {
        struct EventHandler;
        impl EventHandler {
            fn handle(event: Event, _status: iced::event::Status) -> Option<Message> {
                if let Event::Keyboard(key_event) = event {
                    match key_event {
                        KeyEvent::KeyPressed {
                            key_code: keyboard::KeyCode::Tab,
                            modifiers,
                            ..
                        } if !modifiers.alt() && !modifiers.control() && !modifiers.shift() => {
                            Some(Message::TabPressed)
                        }
                        KeyEvent::KeyPressed {
                            key_code: keyboard::KeyCode::Up,
                            ..
                        } => Some(Message::HistoryUp),
                        KeyEvent::KeyPressed {
                            key_code: keyboard::KeyCode::Down,
                            ..
                        } => Some(Message::HistoryDown),
                        KeyEvent::KeyPressed {
                            key_code: keyboard::KeyCode::Left,
                            modifiers,
                        } if modifiers.alt() => Some(Message::ResizeLeft),
                        KeyEvent::KeyPressed {
                            key_code: keyboard::KeyCode::Right,
                            modifiers,
                        } if modifiers.alt() => Some(Message::ResizeRight),
                        KeyEvent::KeyPressed {
                            key_code: keyboard::KeyCode::Grave,
                            modifiers,
                        } if modifiers.shift() => Some(Message::TildePressed),
                        KeyEvent::KeyPressed {
                            key_code: keyboard::KeyCode::E,
                            modifiers,
                        } if modifiers.control() => Some(Message::ToggleFocus),
                        _ => None,
                    }
                } else {
                    None
                }
            }
        }

        let keyboard_events = iced::subscription::events_with(EventHandler::handle);
        
        // Only create the terminal poll subscription if we have a command running
        let terminal_poll = if self.state.command_receiver.is_some() {
            subscription::unfold(
                "terminal_poll",
                State::Ready,
                move |state| async move {
                    match state {
                        State::Ready => {
                            tokio::time::sleep(Duration::from_millis(10)).await;
                            (Message::PollCommandOutput, State::Waiting)
                        }
                        State::Waiting => {
                            tokio::time::sleep(Duration::from_millis(10)).await;
                            (Message::PollCommandOutput, State::Waiting)
                        }
                    }
                },
            )
        } else {
            subscription::unfold("inactive_poll", (), |_| async {
                tokio::time::sleep(Duration::from_secs(3600)).await;
                (Message::PollCommandOutput, ())
            })
        };
        
        iced::Subscription::batch(vec![
            keyboard_events,
            terminal_poll,
        ])
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        // First, check if we have a streaming command that needs polling
        if let Some(command) = self.state.poll_command_output() {
            return command;
        }
        
        match message {
            Message::PollCommandOutput => {
                if let Some(cmd) = self.state.poll_command_output() {
                    cmd
                } else {
                    Command::none()
                }
            }
            Message::TerminalInput(value) => {
                self.terminal_input = value;
                // Reset suggestions when input changes
                self.current_suggestions.clear();
                self.suggestion_index = 0;
                Command::none()
            }
            Message::AIInput(value) => {
                self.ai_input = value;
                Command::none()
            }
            Message::ExecuteCommand => {
                if !self.terminal_input.is_empty() {
                    self.state.input = self.terminal_input.clone();
                    
                    // Start command execution
                    self.state.execute_command();
                    self.terminal_input.clear();
                    
                    // Add slight delay before scrolling to improve smoothness
                    let scroll_cmd = components::scrollable_container::scroll_to_bottom();
                    return Command::batch(vec![
                        Command::perform(async {}, |_| Message::NoOp),
                        scroll_cmd,
                    ]);
                }
                Command::none()
            }
            Message::ProcessAIQuery => {
                if !self.ai_input.is_empty() {
                    let query = self.ai_input.clone();
                    self.ai_input.clear();
                    
                    // Add query to output
                    let formatted_query = format!("> {}", query);
                    self.state.ai_output.push(formatted_query.clone());

                    // Check if the input is a command
                    if query.starts_with('/') {
                        let parts: Vec<&str> = query.split_whitespace().collect();
                        let cmd = parts[0];
                        
                        match cmd {
                            "/models" => {
                                println!("Processing /models command");
                                self.state.ai_output.push("🔍 Fetching models...".to_string());
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
                                commands::process_ai_command(&mut self.state, &query);
                                Command::none()
                            }
                        }
                    } else {
                        self.state.ai_output.push("Thinking...".to_string());
                        
                        // Create the context for Ollama
                        let message_with_context = self.create_ollama_context(&query);
                        let model = self.state.ollama_model.clone();

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
                } else {
                    Command::none()
                }
            }
            Message::OllamaResponse(result) => {
                println!("Handling OllamaResponse message");
                match result {
                    Ok(response) => {
                        println!("Processing successful response");
                        // Remove thinking message
                        if let Some(last) = self.state.ai_output.last() {
                            if last.contains("Thinking") || last.contains("🔍 Fetching") {
                                println!("Removing thinking/fetching message");
                                self.state.ai_output.pop();
                            }
                        }

                        // Extract commands from the response
                        let extracted_command = utils::extract_commands(&response);
                        if !extracted_command.is_empty() {
                            println!("Extracted command: {}", extracted_command);
                            self.state.last_ai_command = Some(extracted_command.clone());
                            self.terminal_input = extracted_command;
                        }

                        // Add the AI response to output
                        println!("Adding response to output: {}", response);
                        self.state.ai_output.push(response.clone());
                        
                        // Add slight delay before scrolling to improve smoothness
                        let scroll_cmd = components::scrollable_container::scroll_to_bottom();
                        return Command::batch(vec![
                            Command::perform(async {}, |_| Message::NoOp),
                            scroll_cmd,
                        ]);
                    }
                    Err(error) => {
                        println!("Processing error response: {}", error);
                        // Remove thinking message
                        if let Some(last) = self.state.ai_output.last() {
                            if last.contains("Thinking") || last.contains("🔍 Fetching") {
                                println!("Removing thinking/fetching message");
                                self.state.ai_output.pop();
                            }
                        }
                        self.state.ai_output.push(format!("Error: {}", error));
                        
                        // Add slight delay before scrolling to improve smoothness
                        let scroll_cmd = components::scrollable_container::scroll_to_bottom();
                        return Command::batch(vec![
                            Command::perform(async {}, |_| Message::NoOp),
                            scroll_cmd,
                        ]);
                    }
                }
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
                    if let Some(current_index) = self.state.command_history_index {
                        // Already navigating history, try to go to older command
                        if current_index > 0 {
                            self.state.command_history_index = Some(current_index - 1);
                            if let Some(command) = self.state.command_history.get(current_index - 1) {
                                self.terminal_input = command.clone();
                            }
                        }
                    } else if !self.state.command_history.is_empty() {
                        // Start from newest command (last in the vector)
                        let last_idx = self.state.command_history.len() - 1;
                        self.state.command_history_index = Some(last_idx);
                        if let Some(command) = self.state.command_history.last() {
                            self.terminal_input = command.clone();
                        }
                    }
                }
                Command::none()
            }
            Message::HistoryDown => {
                if self.focus == FocusTarget::Terminal {
                    if let Some(current_index) = self.state.command_history_index {
                        // Move to newer command
                        if current_index < self.state.command_history.len() - 1 {
                            self.state.command_history_index = Some(current_index + 1);
                            if let Some(command) = self.state.command_history.get(current_index + 1) {
                                self.terminal_input = command.clone();
                            }
                        } else {
                            // At newest command, clear input
                            self.state.command_history_index = None;
                            self.terminal_input.clear();
                        }
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
                // Only update the scroll position if we're not actively
                // trying to scroll to the bottom
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
                    FocusTarget::Terminal => text_input::focus(text_input::Id::new(TERMINAL_INPUT_ID)),
                    FocusTarget::AiChat => text_input::focus(text_input::Id::new(AI_INPUT_ID)),
                }
            }
            Message::ScrollToBottom => {
                // Only scroll to bottom when explicitly requested, not on every scroll event
                // This prevents scroll stuttering when user is manually scrolling
                components::scrollable_container::scroll_to_bottom()
            }
            Message::UpdateTerminalOutput(line) => {
                self.state.output.push(line);
                components::scrollable_container::scroll_to_bottom()
            }
            Message::SendInput(input) => {
                if self.state.password_mode {
                    self.state.send_input(input);
                    self.terminal_input.clear();  // Clear the input after sending password
                }
                Command::none()
            }
            Message::TabPressed => {
                println!("[app.rs] Tab pressed message received");
                if self.focus == FocusTarget::Terminal {
                    println!("[app.rs] Focus is on terminal");
                    
                    // If we don't have any suggestions yet, get them
                    if self.current_suggestions.is_empty() {
                        println!("[app.rs] Getting new suggestions");
                        self.state.input = self.terminal_input.clone();
                        self.current_suggestions = self.state.get_autocomplete_suggestions();
                        self.suggestion_index = 0;
                        println!("[app.rs] Got suggestions: {:?}", self.current_suggestions);
                    } else {
                        // We already have suggestions, move to the next one
                        println!("[app.rs] Moving to next suggestion");
                        self.suggestion_index = (self.suggestion_index + 1) % self.current_suggestions.len();
                    }

                    // Apply the current suggestion if we have any
                    if !self.current_suggestions.is_empty() {
                        println!("[app.rs] Using suggestion {}: {}", self.suggestion_index, self.current_suggestions[self.suggestion_index]);
                        self.terminal_input = self.current_suggestions[self.suggestion_index].clone();
                        // Move cursor to end after applying suggestion
                        return text_input::move_cursor_to_end(text_input::Id::new(TERMINAL_INPUT_ID));
                    } else {
                        println!("[app.rs] No suggestions found");
                    }
                } else {
                    println!("[app.rs] Focus is not on terminal");
                    self.focus = FocusTarget::Terminal;
                }
                Command::none()
            }
            Message::NoOp => {
                Command::none()
            }
        }
    }

    fn view(&self) -> Element<Message> {
        let terminal_panel = self.view_terminal_panel();
        let ai_panel = self.view_ai_panel();

        row![
            container(terminal_panel)
                .width(Length::FillPortion(self.state.panel_ratio as u16))
                .height(Length::Fill)
                .style(DraculaTheme::container_style()),
            drag_handle(),
            container(ai_panel)
                .width(Length::FillPortion((100 - self.state.panel_ratio) as u16))
                .height(Length::Fill)
                .style(DraculaTheme::container_style()),
        ]
        .height(Length::Fill)
        .into()
    }
}

impl TerminalApp {
    fn view_terminal_panel(&self) -> Element<Message> {
        let mut blocks = Vec::new();
        let mut current_block = Vec::new();

        // Group commands and their outputs into blocks
        // Only render the last N blocks for better performance
        let visible_output = if self.state.output.len() > 100 {
            // Only show the last 100 lines for better performance
            self.state.output.iter().skip(self.state.output.len() - 100).cloned().collect()
        } else {
            self.state.output.clone()
        };

        for line in &visible_output {
            if line.starts_with("> ") && !current_block.is_empty() {
                // If we were building a previous block, add it
                blocks.push(current_block);
                current_block = Vec::new();
            }
            current_block.push(line.clone());
        }
        
        // Add the last block if any
        if !current_block.is_empty() {
            blocks.push(current_block);
        }

        // Get status for each block
        let mut block_status = self.state.command_status.clone();
        // Add a default status for the initial instructions block if needed
        if blocks.len() > block_status.len() {
            block_status.insert(0, CommandStatus::Success);
        }

        // Create styled blocks
        let output_elements: Element<_> = column(
            blocks.iter().enumerate().map(|(i, block)| {
                // Always use the default command block style, regardless of status
                let style = DraculaTheme::command_block_style();
                
                // Check if this block has a failure status for coloring the command line
                let has_failed = i < block_status.len() && block_status[i] == CommandStatus::Failure;

                container(
                    column(
                        block.iter().map(|line| {
                            styled_text(
                                line,
                                line.starts_with("> "),
                                // Only pass true for command lines (starting with >) and when the command failed
                                line.starts_with("> ") && has_failed
                            )
                        }).collect()
                    ).spacing(2)
                    .width(Length::Fill)
                )
                .padding(10)
                .width(Length::Fill)
                .style(style)
                .into()
            }).collect()
        )
        .spacing(10)
        .width(Length::Fill)
        .into();

        let terminal_output = components::scrollable_container::scrollable_container(output_elements);

        let dir_path = if let Some(home) = dirs_next::home_dir() {
            if let Ok(path) = self.state.current_dir.strip_prefix(&home) {
                format!("~/{}", path.display())
            } else {
                self.state.current_dir.display().to_string()
            }
        } else {
            self.state.current_dir.display().to_string()
        };

        // Create directory path display, possibly with git info
        let current_dir_content = if self.state.is_git_repo {
            if let Some(branch) = &self.state.git_branch {
                row![
                    styled_text(&dir_path, false, false),
                    styled_text(" ", false, false),
                    crate::ui::components::git_branch_text(branch)
                ]
            } else {
                row![styled_text(&dir_path, false, false)]
            }
        } else {
            row![styled_text(&dir_path, false, false)]
        };

        let current_dir = container(current_dir_content)
            .padding(5)
            .width(Length::Fill)
            .style(DraculaTheme::current_dir_style());

        let input = if self.state.password_mode {
            // Password input field (hidden text)
            text_input("Enter password...", &self.terminal_input)
                .on_input(Message::TerminalInput)
                .on_submit(Message::SendInput(self.terminal_input.clone()))  // Send password on Enter
                .password()  // This makes the input field hide the text
                .padding(5)
                .font(Font::MONOSPACE)
                .size(12)
                .id(text_input::Id::new(TERMINAL_INPUT_ID))
                .style(if self.focus == FocusTarget::Terminal {
                    DraculaTheme::focused_text_input_style()
                } else {
                    DraculaTheme::text_input_style()
                })
        } else {
            // Normal command input field
            text_input("Enter command...", &self.terminal_input)
                .on_input(Message::TerminalInput)
                .on_submit(Message::ExecuteCommand)
                .padding(5)
                .font(Font::MONOSPACE)
                .size(12)
                .id(text_input::Id::new(TERMINAL_INPUT_ID))
                .style(if self.focus == FocusTarget::Terminal {
                    DraculaTheme::focused_text_input_style()
                } else {
                    DraculaTheme::text_input_style()
                })
        };

        column![
            terminal_output,
            current_dir,
            input,
        ]
        .spacing(10)
        .padding(10)
        .into()
    }

    fn view_ai_panel(&self) -> Element<Message> {
        let mut blocks = Vec::new();
        let mut current_block = Vec::new();

        // Group AI messages and responses into blocks
        // Only render the last N blocks for better performance
        let visible_output = if self.state.ai_output.len() > 50 {
            // Only show the last 50 lines for better performance
            self.state.ai_output.iter().skip(self.state.ai_output.len() - 50).cloned().collect()
        } else {
            self.state.ai_output.clone()
        };

        // Group AI messages and responses into blocks
        for line in &visible_output {
            if line.starts_with("> ") && !current_block.is_empty() {
                // If we were building a previous block, add it
                blocks.push(current_block);
                current_block = Vec::new();
            }
            current_block.push(line.clone());
        }
        
        // Add the last block if any
        if !current_block.is_empty() {
            blocks.push(current_block);
        }

        // Create styled blocks
        let output_elements: Element<_> = column(
            blocks.iter().map(|block| {
                container(
                    column(
                        block.iter().map(|line| {
                            styled_text(
                                line,
                                line.starts_with("> "),
                                false // AI commands don't have failure status
                            )
                        }).collect()
                    ).spacing(2)
                    .width(Length::Fill)
                )
                .padding(10)
                .width(Length::Fill)
                .style(DraculaTheme::command_block_style())
                .into()
            }).collect()
        )
        .spacing(10)
        .width(Length::Fill)
        .into();

        let ai_output = components::scrollable_container::scrollable_container(output_elements);

        let input = text_input("Ask AI...", &self.ai_input)
            .on_input(Message::AIInput)
            .on_submit(Message::ProcessAIQuery)
            .padding(5)
            .font(Font::MONOSPACE)
            .size(12)
            .id(text_input::Id::new(AI_INPUT_ID))
            .style(if self.focus == FocusTarget::AiChat {
                DraculaTheme::focused_text_input_style()
            } else {
                DraculaTheme::text_input_style()
            });

        column![
            ai_output,
            input,
        ]
        .spacing(10)
        .padding(10)
        .into()
    }

    fn create_ollama_context(&self, query: &str) -> String {
        format!(
            "System Info: {}\n\nRecent Terminal Output:\n{}\n\nRecent Chat History:\n{}\n\nUser query: {}\n\nCurrent directory: {}",
            self.state.os_info,
            self.state.output.iter().rev().take(20).map(String::as_str).collect::<Vec<_>>().join("\n"),
            self.state.ai_output.iter().rev().take(10).map(String::as_str).collect::<Vec<_>>().join("\n"),
            query,
            self.state.current_dir.display()
        )
    }

    pub fn handle_input(&mut self, key_event: KeyEvent) {
        match key_event {
            KeyEvent::KeyPressed { 
                key_code: keyboard::KeyCode::Tab,
                modifiers,
                ..
            } if !modifiers.alt() && !modifiers.control() && !modifiers.shift() => {
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
            }
            _ => {
                if handle_keyboard_shortcuts(key_event, &mut self.focus) {
                    return;
                }
            }
        }
        
        // Handle other input based on focus
        match self.focus {
            FocusTarget::Terminal => {
                // ... existing terminal input handling ...
            }
            FocusTarget::AiChat => {
                // ... existing AI chat input handling ...
            }
        }
    }
}

#[derive(Debug, Clone)]
enum State {
    Ready,
    Waiting,
}
