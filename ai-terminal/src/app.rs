use iced::widget::{column, container, row, text_input, scrollable};
use iced::{Application, Command, Element, Length, Theme, Font};
use iced::keyboard::{self, Event as KeyEvent};
use iced::event::Event;

use crate::model::{App as AppState, Panel, CommandStatus};
use crate::ollama::{api, commands};
use crate::ui::components::{styled_text, drag_handle};
use crate::ui::theme::DraculaTheme;
use crate::terminal::utils;

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
}

pub struct TerminalApp {
    state: AppState,
    terminal_input: String,
    ai_input: String,
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
            },
            Command::none(),
        )
    }

    fn title(&self) -> String {
        String::from("AI Terminal")
    }

    fn subscription(&self) -> iced::Subscription<Message> {
        iced::subscription::events_with(|event, _status| {
            match event {
                Event::Keyboard(KeyEvent::KeyPressed { key_code, modifiers }) => {
                    match key_code {
                        keyboard::KeyCode::Tab => Some(Message::SwitchPanel),
                        keyboard::KeyCode::Up => Some(Message::HistoryUp),
                        keyboard::KeyCode::Down => Some(Message::HistoryDown),
                        keyboard::KeyCode::Left if modifiers.alt() => {
                            Some(Message::ResizeLeft)
                        }
                        keyboard::KeyCode::Right if modifiers.alt() => {
                            Some(Message::ResizeRight)
                        }
                        keyboard::KeyCode::Grave if modifiers.contains(keyboard::Modifiers::SHIFT) => {
                            Some(Message::TildePressed)
                        }
                        _ => None,
                    }
                }
                _ => None,
            }
        })
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::TerminalInput(value) => {
                self.terminal_input = value;
                Command::none()
            }
            Message::AIInput(value) => {
                self.ai_input = value;
                Command::none()
            }
            Message::ExecuteCommand => {
                if !self.terminal_input.is_empty() {
                    self.state.input = self.terminal_input.clone();
                    self.state.execute_command();
                    self.terminal_input.clear();
                }
                Command::none()
            }
            Message::ProcessAIQuery => {
                if !self.ai_input.is_empty() {
                    let query = self.ai_input.clone();
                    self.ai_input.clear();
                    
                    // Add query to output
                    self.state.ai_output.push(format!("> {}", query));

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
                            self.state.last_ai_command = Some(extracted_command);
                        }

                        // Add the AI response to output
                        println!("Adding response to output: {}", response);
                        self.state.ai_output.push(response);
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
                    }
                }
                Command::none()
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
                // Handle history navigation up
                if let Some(index) = self.state.command_history_index {
                    if index > 0 {
                        self.state.command_history_index = Some(index - 1);
                        if let Some(command) = self.state.command_history.get(index - 1) {
                            self.terminal_input = command.clone();
                        }
                    }
                } else if !self.state.command_history.is_empty() {
                    self.state.command_history_index = Some(self.state.command_history.len() - 1);
                    if let Some(command) = self.state.command_history.last() {
                        self.terminal_input = command.clone();
                    }
                }
                Command::none()
            }
            Message::HistoryDown => {
                // Handle history navigation down
                if let Some(index) = self.state.command_history_index {
                    if index < self.state.command_history.len() - 1 {
                        self.state.command_history_index = Some(index + 1);
                        if let Some(command) = self.state.command_history.get(index + 1) {
                            self.terminal_input = command.clone();
                        }
                    } else {
                        self.state.command_history_index = None;
                        self.terminal_input.clear();
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
                self.state.terminal_scroll = viewport.relative_offset().y as usize;
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
        for line in &self.state.output {
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
                let style = if i < block_status.len() {
                    match block_status[i] {
                        CommandStatus::Success => DraculaTheme::success_command_block_style(),
                        CommandStatus::Failure => DraculaTheme::failure_command_block_style(),
                        _ => DraculaTheme::command_block_style(),
                    }
                } else {
                    DraculaTheme::command_block_style()
                };

                container(
                    column(
                        block.iter().map(|line| {
                            styled_text(
                                line,
                                line.starts_with("> ")
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

        let terminal_output = scrollable(output_elements)
            .height(Length::Fill)
            .on_scroll(Message::TerminalScroll)
            .id(scrollable::Id::new("terminal-output"));

        let current_dir = container(
            styled_text(
                &format!("{}",
                    if let Some(home) = dirs_next::home_dir() {
                        if let Ok(path) = self.state.current_dir.strip_prefix(&home) {
                            format!("~/{}", path.display())
                        } else {
                            self.state.current_dir.display().to_string()
                        }
                    } else {
                        self.state.current_dir.display().to_string()
                    }
                ),
                false
            )
        )
        .padding(5)
        .width(Length::Fill)
        .style(DraculaTheme::current_dir_style());

        let input = text_input("Enter command...", &self.terminal_input)
            .on_input(Message::TerminalInput)
            .on_submit(Message::ExecuteCommand)
            .padding(5)
            .font(Font::MONOSPACE)
            .size(12)
            .style(DraculaTheme::text_input_style());

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
        for line in &self.state.ai_output {
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
                                line.starts_with("> ")
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

        let ai_output = scrollable(output_elements)
            .height(Length::Fill)
            .id(scrollable::Id::new("ai-output"));

        let input = text_input("Ask AI...", &self.ai_input)
            .on_input(Message::AIInput)
            .on_submit(Message::ProcessAIQuery)
            .padding(5)
            .font(Font::MONOSPACE)
            .size(12)
            .style(DraculaTheme::text_input_style());

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
}
