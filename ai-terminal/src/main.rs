use std::env;
use std::fs;
use std::io;
use std::panic;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
        MouseButton, MouseEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use tui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
    Terminal,
};

struct App {
    input: String,
    output: Vec<String>,
    cursor_position: usize,
    current_dir: PathBuf,
    // AI assistant fieldsg
    ai_input: String,
    ai_output: Vec<String>,
    ai_cursor_position: usize,
    active_panel: Panel,
    // Panel sizing (percentage of width for the terminal panel)
    panel_ratio: u16,
    // Mouse drag state
    is_dragging: bool,
    // Store layout information for mouse interaction
    terminal_area: Option<Rect>,
    assistant_area: Option<Rect>,
    divider_x: Option<u16>,
    // Scroll state
    terminal_scroll: usize,
    assistant_scroll: usize,
    // Command status tracking
    command_status: Vec<CommandStatus>,
    // Command history
    command_history: Vec<String>,
    command_history_index: Option<usize>,
    // Autocomplete suggestions
    autocomplete_suggestions: Vec<String>,
    autocomplete_index: Option<usize>,
}

enum CommandStatus {
    Success,
    Failure,
    Running,
}

#[derive(PartialEq)]
enum Panel {
    Terminal,
    Assistant,
}

impl App {
    fn new() -> Self {
        // Always start at the root directory
        let current_dir = PathBuf::from("/");
        
        // Set the current working directory to the root
        let _ = env::set_current_dir(&current_dir);

        // Initial output messages
        let initial_output = vec![
            "Welcome to AI Terminal! Type commands below.".to_string(),
            format!("Current directory: {}", current_dir.display()),
            "Use Alt+Left/Right to resize panels.".to_string(),
            "Click on a panel to focus it.".to_string(),
            "Drag the divider between panels to resize them.".to_string(),
            "Use PageUp/PageDown or mouse wheel to scroll through output.".to_string(),
            "Use Alt+Up/Down to scroll through output.".to_string(),
            "Use Up/Down arrow keys to navigate through command history.".to_string(),
            "Use Tab key for command and path autocompletion.".to_string(),
        ];

        // Initial AI output messages
        let initial_ai_output = vec![
            "AI Assistant ready. Type your message below.".to_string(),
            "Use Alt+Left/Right to resize panels.".to_string(),
            "Click on a panel to focus it.".to_string(),
            "Drag the divider between panels to resize them.".to_string(),
            "Use PageUp/PageDown or mouse wheel to scroll through output.".to_string(),
            "Use Alt+Up/Down to scroll through output.".to_string(),
        ];

        // Initialize command status for any commands in the initial output
        let mut command_status = Vec::new();
        for line in &initial_output {
            if line.starts_with("> ") {
                command_status.push(CommandStatus::Success);
            }
        }

        App {
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
            panel_ratio: 50,
            // Mouse drag state
            is_dragging: false,
            // Store layout information for mouse interaction
            terminal_area: None,
            assistant_area: None,
            divider_x: None,
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
        }
    }

    fn execute_command(&mut self) {
        let command = self.input.clone();
        let command = command.trim();
        if command.is_empty() {
            return;
        }

        // Add command to history (only if it's not empty and not the same as the last command)
        if !command.is_empty() && (self.command_history.is_empty() || self.command_history.last().unwrap() != command) {
            // Add to history
            self.command_history.push(command.to_string());
            
            // Limit history to 30 commands
            if self.command_history.len() > 30 {
                self.command_history.remove(0);
            }
        }
        
        // Reset history index
        self.command_history_index = None;

        // Add command to output
        self.output.push(format!("> {}", command));
        
        // Add a placeholder for command status
        self.command_status.push(CommandStatus::Running);
        let command_index = self.command_status.len() - 1;

        // Handle cd command specially
        if command.starts_with("cd ") {
            let path = command.trim_start_matches("cd ").trim();
            let success = self.change_directory(path);
            
            // Update command status
            if success {
                self.command_status[command_index] = CommandStatus::Success;
            } else {
                self.command_status[command_index] = CommandStatus::Failure;
            }
            
            // Add a separator after the command output
            self.output.push("─".repeat(40));
        } else {
            // Execute the command
            let (output, success) = self.run_command(command);
            self.output.extend(output);
            
            // Update command status
            if success {
                self.command_status[command_index] = CommandStatus::Success;
            } else {
                self.command_status[command_index] = CommandStatus::Failure;
            }
            
            // Add a separator after the command output
            self.output.push("─".repeat(40));
        }

        self.input.clear();
        self.cursor_position = 0;
        
        // Set scroll to 0 to always show the most recent output at the bottom
        // In the Paragraph widget, scroll is applied from the bottom when using negative values
        self.terminal_scroll = 0;
    }

    fn run_command(&self, command: &str) -> (Vec<String>, bool) {
        let mut result = Vec::new();
        let mut success = true;
        
        // Split the command into program and arguments
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() {
            return (result, success);
        }

        let program = parts[0];
        let args = &parts[1..];

        // Execute the command
        match Command::new(program)
            .args(args)
            .current_dir(&self.current_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
        {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);

                // Add stdout to output
                if !stdout.is_empty() {
                    for line in stdout.lines() {
                        result.push(line.to_string());
                    }
                }

                // Add stderr to output
                if !stderr.is_empty() {
                    for line in stderr.lines() {
                        result.push(format!("Error: {}", line));
                    }
                }

                // Add exit status
                if !output.status.success() {
                    result.push(format!("Command exited with status: {}", output.status));
                    success = false;
                }
            }
            Err(e) => {
                result.push(format!("Failed to execute command: {}", e));
                success = false;
            }
        }

        (result, success)
    }

    fn change_directory(&mut self, path: &str) -> bool {
        let new_dir = if path.starts_with('/') {
            // Absolute path
            PathBuf::from(path)
        } else if path == "~" || path.starts_with("~/") {
            // Home directory
            if let Some(home) = dirs_next::home_dir() {
                if path == "~" {
                    home
                } else {
                    home.join(path.trim_start_matches("~/"))
                }
            } else {
                self.output
                    .push("Error: Could not determine home directory".to_string());
                return false;
            }
        } else if path == ".." {
            // Parent directory
            if let Some(parent) = self.current_dir.parent() {
                PathBuf::from(parent)
            } else {
                self.output
                    .push("Error: Already at root directory".to_string());
                return false;
            }
        } else {
            // Relative path
            self.current_dir.join(path)
        };

        // Try to change to the new directory
        match env::set_current_dir(&new_dir) {
            Ok(_) => {
                self.current_dir = new_dir;
                self.output.push(format!(
                    "Changed directory to: {}",
                    self.current_dir.display()
                ));
                true
            }
            Err(e) => {
                self.output.push(format!("Error changing directory: {}", e));
                false
            }
        }
    }

    // Get autocomplete suggestions based on current input
    fn get_autocomplete_suggestions(&mut self) -> Vec<String> {
        let input = self.input.clone();
        let mut suggestions = Vec::new();

        // If input is empty, return empty suggestions
        if input.is_empty() {
            return suggestions;
        }

        // Split input into parts
        let parts: Vec<&str> = input.split_whitespace().collect();
        
        // Check if we're trying to autocomplete a path (for cd, ls, etc.)
        if parts.len() >= 2 && ["cd", "ls", "cat", "rm", "cp", "mv", "mkdir", "touch"].contains(&parts[0]) {
            let command = parts[0];
            let path_part = if parts.len() > 1 {
                // Get the last part which is being typed
                parts.last().unwrap()
            } else {
                ""
            };
            
            // For cd command, only suggest directories
            if command == "cd" {
                suggestions = self.get_path_suggestions(path_part).into_iter()
                    .filter(|s| s.ends_with('/'))
                    .collect();
            } else {
                // For other commands, suggest both files and directories
                suggestions = self.get_path_suggestions(path_part);
            }
            
            // Format suggestions to include the command and any intermediate arguments
            if parts.len() > 2 {
                let prefix = parts[..parts.len()-1].join(" ") + " ";
                suggestions = suggestions.into_iter()
                    .map(|s| format!("{}{}", prefix, s))
                    .collect();
            } else if parts.len() == 2 {
                let prefix = format!("{} ", command);
                suggestions = suggestions.into_iter()
                    .map(|s| format!("{}{}", prefix, s))
                    .collect();
            }
        } else if !input.contains(' ') {
            // We're at the beginning of a command (no space yet)
            // Common Unix commands for autocompletion
            let common_commands = vec![
                "ls", "cd", "pwd", "mkdir", "rmdir", "touch", "rm", "cp", "mv",
                "cat", "less", "grep", "find", "echo", "ps", "kill", "chmod",
                "chown", "df", "du", "tar", "gzip", "gunzip", "zip", "unzip",
                "ssh", "scp", "curl", "wget", "ping", "ifconfig", "netstat",
                "top", "htop", "man", "history", "clear", "exit",
            ];

            for cmd in common_commands {
                if cmd.starts_with(&input) {
                    suggestions.push(cmd.to_string());
                }
            }

            // Also add commands from history
            for cmd in &self.command_history {
                let cmd_part = cmd.split_whitespace().next().unwrap_or("");
                if cmd_part.starts_with(&input) && !suggestions.contains(&cmd_part.to_string()) {
                    suggestions.push(cmd_part.to_string());
                }
            }
        }

        suggestions.sort();
        suggestions
    }

    // Get path suggestions for cd command
    fn get_path_suggestions(&self, path_part: &str) -> Vec<String> {
        let mut suggestions = Vec::new();
        
        // Determine the directory to search in and the prefix to match
        let (search_dir, prefix) = if path_part.is_empty() {
            // If no path specified, suggest directories in current directory
            (self.current_dir.clone(), "".to_string())
        } else if path_part == "~" {
            // Suggest home directory
            if let Some(home) = dirs_next::home_dir() {
                (home, "~".to_string())
            } else {
                return suggestions;
            }
        } else if path_part.starts_with("~/") {
            // Suggest in home directory with subdirectory
            if let Some(home) = dirs_next::home_dir() {
                let subdir = path_part.trim_start_matches("~/");
                let last_slash = subdir.rfind('/').unwrap_or(0);
                let (dir_part, _file_prefix) = if last_slash == 0 {
                    (subdir, "")
                } else {
                    subdir.split_at(last_slash)
                };
                
                let search_path = if dir_part.is_empty() {
                    home.clone()
                } else {
                    home.join(dir_part)
                };
                
                (search_path, format!("~/{}{}", dir_part, if !dir_part.is_empty() && !dir_part.ends_with('/') { "/" } else { "" }))
            } else {
                return suggestions;
            }
        } else if path_part.starts_with('/') {
            // Absolute path
            let last_slash = path_part.rfind('/').unwrap_or(0);
            let (dir_part, _file_prefix) = path_part.split_at(last_slash + 1);
            
            (PathBuf::from(dir_part), dir_part.to_string())
        } else {
            // Relative path
            let last_slash = path_part.rfind('/').unwrap_or(0);
            let (dir_part, _file_prefix) = if last_slash == 0 {
                ("", path_part)
            } else {
                path_part.split_at(last_slash + 1)
            };
            
            let search_path = if dir_part.is_empty() {
                self.current_dir.clone()
            } else {
                self.current_dir.join(dir_part)
            };
            
            (search_path, dir_part.to_string())
        };
        
        // Get the part after the last slash to match against
        let match_prefix = if let Some(last_slash) = path_part.rfind('/') {
            &path_part[last_slash + 1..]
        } else {
            path_part
        };
        
        // Read the directory and find matching entries
        if let Ok(entries) = fs::read_dir(&search_dir) {
            for entry in entries.flatten() {
                if let Ok(file_name) = entry.file_name().into_string() {
                    // Check if the file name starts with our prefix
                    if file_name.starts_with(match_prefix) {
                        if let Ok(file_type) = entry.file_type() {
                            let suggestion = if file_type.is_dir() {
                                // Add trailing slash for directories
                                format!("{}{}/", prefix, file_name)
                            } else {
                                // Regular file
                                format!("{}{}", prefix, file_name)
                            };
                            suggestions.push(suggestion);
                        }
                    }
                }
            }
        }
        
        // Add special directories if they match
        if ".".starts_with(match_prefix) {
            suggestions.push(format!("{}./", prefix));
        }
        if "..".starts_with(match_prefix) {
            suggestions.push(format!("{}../", prefix));
        }
        
        suggestions
    }

    // Apply autocomplete suggestion
    fn apply_autocomplete(&mut self) {
        if let Some(index) = self.autocomplete_index {
            if index < self.autocomplete_suggestions.len() {
                let suggestion = &self.autocomplete_suggestions[index];
                
                // Replace the input with the suggestion
                self.input = suggestion.clone();
                
                // Move cursor to end of input
                self.cursor_position = self.input.len();
                
                // Clear suggestions after applying
                self.autocomplete_suggestions.clear();
                self.autocomplete_index = None;
            }
        } else if !self.autocomplete_suggestions.is_empty() {
            // If we have suggestions but no index, set index to 0
            self.autocomplete_index = Some(0);
        }
    }

    // Cycle through autocomplete suggestions
    fn cycle_autocomplete(&mut self, forward: bool) {
        if self.autocomplete_suggestions.is_empty() {
            // Generate suggestions if we don't have any
            self.autocomplete_suggestions = self.get_autocomplete_suggestions();
            if !self.autocomplete_suggestions.is_empty() {
                self.autocomplete_index = Some(0);
            }
        } else if let Some(index) = self.autocomplete_index {
            // Cycle through existing suggestions
            if forward {
                self.autocomplete_index = Some((index + 1) % self.autocomplete_suggestions.len());
            } else {
                self.autocomplete_index = Some(
                    if index == 0 {
                        self.autocomplete_suggestions.len() - 1
                    } else {
                        index - 1
                    }
                );
            }
        }
        
        // Apply the current suggestion
        self.apply_autocomplete();
    }
}

// Function to restore terminal state in case of panic
fn restore_terminal() -> Result<(), io::Error> {
    disable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, LeaveAlternateScreen, DisableMouseCapture)?;
    Ok(())
}

fn main() -> Result<(), io::Error> {
    // Set up panic hook to restore terminal state on panic
    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        let _ = restore_terminal();
        original_hook(panic_info);
    }));

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        EnterAlternateScreen,
        EnableMouseCapture,
        // Enable focus reporting
        event::EnableFocusChange
    )?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut app = App::new();

    // Main loop
    let result = run_app(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture,
        // Disable focus reporting
        event::DisableFocusChange
    )?;
    terminal.show_cursor()?;

    // Handle any errors from the app
    if let Err(err) = result {
        println!("Error: {}", err);
    }

    Ok(())
}

fn run_app<B: tui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> Result<(), io::Error> {
    loop {
        // Draw UI
        terminal.draw(|f| {
            let size = f.size();

            // Create main horizontal layout (terminal and assistant)
            let main_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .margin(1)
                .constraints(
                    [
                        Constraint::Percentage(app.panel_ratio),
                        Constraint::Percentage(100 - app.panel_ratio),
                    ]
                    .as_ref(),
                )
                .split(size);

            // Store layout information for mouse interaction
            app.terminal_area = Some(main_chunks[0]);
            app.assistant_area = Some(main_chunks[1]);
            app.divider_x = Some(main_chunks[0].x + main_chunks[0].width);

            // Terminal panel (left side)
            let terminal_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(1), Constraint::Length(3)].as_ref())
                .split(main_chunks[0]);

            // Output area
            let output_text = Text::from(
                app.output
                    .iter()
                    .enumerate()
                    .flat_map(|(i, line)| {
                        let mut lines = Vec::new();
                        
                        // Now add the line itself with appropriate styling
                        if line.starts_with("> ") {
                            // Find the corresponding command status if available
                            let command_index = app.output
                                .iter()
                                .take(i + 1)
                                .filter(|l| l.starts_with("> "))
                                .count() - 1;
                            
                            // Choose color based on command status
                            let command_color = if command_index < app.command_status.len() {
                                match app.command_status[command_index] {
                                    CommandStatus::Success => Color::Green,
                                    CommandStatus::Failure => Color::Red,
                                    CommandStatus::Running => Color::Yellow,
                                }
                            } else {
                                Color::Yellow // Default color if status not found
                            };
                            
                            // Add the command with appropriate color
                            lines.push(Line::from(vec![
                                Span::styled(line.clone(), Style::default().fg(command_color))
                            ]));
                        } else if line.starts_with("─") {
                            // This is a separator line, style it appropriately
                            lines.push(Line::from(vec![
                                Span::styled(
                                    "─".repeat(terminal_chunks[0].width as usize - 2),
                                    Style::default().fg(Color::DarkGray)
                                )
                            ]));
                        } else {
                            // Regular output line
                            lines.push(Line::from(line.clone()));
                        }
                        
                        lines
                    })
                    .collect::<Vec<Line>>(),
            );
            
            // Remove the divider at the very end of all output
            let output_text = Text::from(output_text.lines);

            // Calculate the total height of the output content
            let actual_line_count = app.output.len();
            
            // Calculate the visible height of the terminal area (minus borders)
            let visible_height = terminal_chunks[0].height.saturating_sub(2);
            
            // If auto-scrolling is enabled (terminal_scroll is 0), show the last line
            if app.terminal_scroll == 0 {
                // Calculate the scroll position to show the last line
                let scroll_position = if actual_line_count > visible_height as usize {
                    (actual_line_count - visible_height as usize + 1) as u16
                } else {
                    0
                };
                
                // Create the paragraph with the calculated scroll position
                let output_paragraph = Paragraph::new(output_text)
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .border_type(BorderType::Rounded)
                            .title("Terminal Output"),
                    )
                    .wrap(Wrap { trim: true })
                    .scroll((scroll_position, 0));
                
                f.render_widget(output_paragraph, terminal_chunks[0]);
            } else {
                // Manual scrolling - use the user-specified scroll position
                let output_paragraph = Paragraph::new(output_text)
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .border_type(BorderType::Rounded)
                            .title("Terminal Output"),
                    )
                    .wrap(Wrap { trim: true })
                    .scroll((app.terminal_scroll as u16, 0));
                
                f.render_widget(output_paragraph, terminal_chunks[0]);
            }

            // Input area with current directory as title
            let input_text = Text::from(app.input.as_str());
            let input_block_style = match app.active_panel {
                Panel::Terminal => Style::default().fg(Color::Yellow),
                Panel::Assistant => Style::default(),
            };
            let input = Paragraph::new(input_text).style(input_block_style).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title(format!("{}", app.current_dir.display())),
            );

            f.render_widget(input, terminal_chunks[1]);

            // Render autocomplete suggestions if available
            if app.active_panel == Panel::Terminal && !app.autocomplete_suggestions.is_empty() {
                // Calculate the position for the suggestions popup
                // It should appear below the input area
                let max_suggestions = 5;
                let suggestions_count = app.autocomplete_suggestions.len().min(max_suggestions);
                let suggestions_height = suggestions_count as u16 + 2; // +2 for borders
                
                // Calculate width based on the longest suggestion
                let suggestions_width = app.autocomplete_suggestions
                    .iter()
                    .take(max_suggestions)
                    .map(|s| s.len())
                    .max()
                    .unwrap_or(20)
                    .min(terminal_chunks[1].width.saturating_sub(4) as usize) as u16 + 4; // +4 for padding
                
                let suggestions_x = terminal_chunks[1].x + 1;
                let suggestions_y = terminal_chunks[1].y + 3;
                
                // Make sure the popup doesn't go off-screen
                let suggestions_y = if suggestions_y + suggestions_height > size.height {
                    terminal_chunks[1].y.saturating_sub(suggestions_height)
                } else {
                    suggestions_y
                };
                
                let suggestions_area = Rect::new(
                    suggestions_x,
                    suggestions_y,
                    suggestions_width,
                    suggestions_height,
                );
                
                // Create the suggestions text
                let suggestions_text = Text::from(
                    app.autocomplete_suggestions
                        .iter()
                        .enumerate()
                        .take(max_suggestions) // Limit to max_suggestions visible suggestions
                        .map(|(i, suggestion)| {
                            // For display purposes, we might want to show a shortened version
                            let display_text = if suggestion.len() > suggestions_width as usize - 4 {
                                // Truncate and add ellipsis
                                format!("{}...", &suggestion[..suggestions_width as usize - 7])
                            } else {
                                suggestion.clone()
                            };
                            
                            if Some(i) == app.autocomplete_index {
                                // Highlight the selected suggestion
                                Line::from(vec![
                                    Span::styled(
                                        format!(" {} ", display_text),
                                        Style::default().fg(Color::Black).bg(Color::White),
                                    )
                                ])
                            } else {
                                Line::from(vec![
                                    Span::styled(
                                        format!(" {} ", display_text),
                                        Style::default().fg(Color::White),
                                    )
                                ])
                            }
                        })
                        .collect::<Vec<Line>>(),
                );
                
                // Add count indicator if there are more suggestions than shown
                let title = if app.autocomplete_suggestions.len() > max_suggestions {
                    format!("Suggestions ({}/{})", max_suggestions, app.autocomplete_suggestions.len())
                } else {
                    "Suggestions".to_string()
                };
                
                let suggestions_widget = Paragraph::new(suggestions_text)
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .border_type(BorderType::Rounded)
                            .title(title),
                    );
                
                f.render_widget(suggestions_widget, suggestions_area);
            }

            // AI Assistant panel (right side)
            let assistant_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(1), Constraint::Length(3)].as_ref())
                .split(main_chunks[1]);

            // AI output area
            let ai_output_text = Text::from(
                app.ai_output
                    .iter()
                    .enumerate()
                    .flat_map(|(i, line)| {
                        let mut lines = Vec::new();
                        
                        // Now add the line itself
                        if line.starts_with("> ") {
                            // Add the user message with a distinct color
                            lines.push(Line::from(vec![
                                Span::styled(line.clone(), Style::default().fg(Color::Cyan))
                            ]));
                        } else if line.starts_with("─") {
                            // This is a separator line, style it appropriately
                            lines.push(Line::from(vec![
                                Span::styled(
                                    "─".repeat(assistant_chunks[0].width as usize - 2),
                                    Style::default().fg(Color::DarkGray),
                                )
                            ]));
                        } else {
                            lines.push(Line::from(line.clone()));
                        }
                        
                        lines
                    })
                    .collect::<Vec<Line>>(),
            );
            
            // Remove the divider at the very end of all AI output
            let ai_output_text = Text::from(ai_output_text.lines);

            // Calculate the total height of the AI output content
            let actual_ai_line_count = app.ai_output.len();
            
            // Calculate the visible height of the assistant area (minus borders)
            let ai_visible_height = assistant_chunks[0].height.saturating_sub(2);
            
            // If auto-scrolling is enabled (assistant_scroll is 0), show the last line
            if app.assistant_scroll == 0 {
                // Calculate the scroll position to show the last line
                let ai_scroll_position = if actual_ai_line_count > ai_visible_height as usize {
                    (actual_ai_line_count - ai_visible_height as usize + 1) as u16
                } else {
                    0
                };
                
                // Create the paragraph with the calculated scroll position
                let ai_output_paragraph = Paragraph::new(ai_output_text)
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .border_type(BorderType::Rounded)
                            .title("AI Assistant"),
                    )
                    .wrap(Wrap { trim: true })
                    .scroll((ai_scroll_position, 0));
                
                f.render_widget(ai_output_paragraph, assistant_chunks[0]);
            } else {
                // Manual scrolling - use the user-specified scroll position
                let ai_output_paragraph = Paragraph::new(ai_output_text)
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .border_type(BorderType::Rounded)
                            .title("AI Assistant"),
                    )
                    .wrap(Wrap { trim: true })
                    .scroll((app.assistant_scroll as u16, 0));
                
                f.render_widget(ai_output_paragraph, assistant_chunks[0]);
            }

            // AI input area
            let ai_input_text = Text::from(app.ai_input.as_str());
            let ai_input_block_style = match app.active_panel {
                Panel::Terminal => Style::default(),
                Panel::Assistant => Style::default().fg(Color::Yellow),
            };
            let ai_input = Paragraph::new(ai_input_text)
                .style(ai_input_block_style)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .title("Message to AI"),
                );

            f.render_widget(ai_input, assistant_chunks[1]);

            // Set cursor position based on active panel
            match app.active_panel {
                Panel::Terminal => {
                    f.set_cursor(
                        terminal_chunks[1].x + app.cursor_position as u16 + 1,
                        terminal_chunks[1].y + 1,
                    );
                }
                Panel::Assistant => {
                    f.set_cursor(
                        assistant_chunks[1].x + app.ai_cursor_position as u16 + 1,
                        assistant_chunks[1].y + 1,
                    );
                }
            }
        })?;

        // Handle input
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        // Resize panels with Alt+Left and Alt+Right
                        KeyCode::Left => {
                            if key.modifiers == KeyModifiers::ALT {
                                // Decrease terminal panel size (min 10%)
                                if app.panel_ratio > 10 {
                                    app.panel_ratio -= 5;
                                }
                            } else {
                                match app.active_panel {
                                    Panel::Terminal => {
                                        if app.cursor_position > 0 {
                                            app.cursor_position -= 1;
                                        }
                                    }
                                    Panel::Assistant => {
                                        if app.ai_cursor_position > 0 {
                                            app.ai_cursor_position -= 1;
                                        }
                                    }
                                }
                            }
                        }
                        KeyCode::Right => {
                            if key.modifiers == KeyModifiers::ALT {
                                // Increase terminal panel size (max 90%)
                                if app.panel_ratio < 90 {
                                    app.panel_ratio += 5;
                                }
                            } else {
                                match app.active_panel {
                                    Panel::Terminal => {
                                        if app.cursor_position < app.input.len() {
                                            app.cursor_position += 1;
                                        }
                                    }
                                    Panel::Assistant => {
                                        if app.ai_cursor_position < app.ai_input.len() {
                                            app.ai_cursor_position += 1;
                                        }
                                    }
                                }
                            }
                        }
                        KeyCode::Up => {
                            if key.modifiers == KeyModifiers::ALT {
                                // Scroll up based on active panel
                                match app.active_panel {
                                    Panel::Terminal => {
                                        if app.terminal_scroll < app.output.len().saturating_sub(1) {
                                            app.terminal_scroll += 1;
                                        }
                                    }
                                    Panel::Assistant => {
                                        if app.assistant_scroll < app.ai_output.len().saturating_sub(1) {
                                            app.assistant_scroll += 1;
                                        }
                                    }
                                }
                            } else {
                                // Navigate command history (up = older commands)
                                match app.active_panel {
                                    Panel::Terminal => {
                                        if !app.command_history.is_empty() {
                                            let new_index = match app.command_history_index {
                                                Some(idx) if idx > 0 => Some(idx - 1),
                                                None => Some(app.command_history.len() - 1),
                                                Some(idx) => Some(idx),
                                            };
                                            
                                            app.command_history_index = new_index;
                                            if let Some(idx) = new_index {
                                                app.input = app.command_history[idx].clone();
                                                app.cursor_position = app.input.len();
                                            }
                                        }
                                    }
                                    Panel::Assistant => {
                                        // No history navigation for assistant panel
                                    }
                                }
                            }
                        }
                        KeyCode::Down => {
                            if key.modifiers == KeyModifiers::ALT {
                                // Scroll down based on active panel
                                match app.active_panel {
                                    Panel::Terminal => {
                                        if app.terminal_scroll > 0 {
                                            app.terminal_scroll -= 1;
                                        }
                                    }
                                    Panel::Assistant => {
                                        if app.assistant_scroll > 0 {
                                            app.assistant_scroll -= 1;
                                        }
                                    }
                                }
                            } else {
                                // Navigate command history (down = newer commands)
                                match app.active_panel {
                                    Panel::Terminal => {
                                        if let Some(idx) = app.command_history_index {
                                            if idx < app.command_history.len() - 1 {
                                                // Move to newer command
                                                let new_idx = idx + 1;
                                                app.command_history_index = Some(new_idx);
                                                app.input = app.command_history[new_idx].clone();
                                            } else {
                                                // At the newest command, clear input
                                                app.command_history_index = None;
                                                app.input.clear();
                                            }
                                            app.cursor_position = app.input.len();
                                        }
                                    }
                                    Panel::Assistant => {
                                        // No history navigation for assistant panel
                                    }
                                }
                            }
                        }
                        KeyCode::Enter => {
                            match app.active_panel {
                                Panel::Terminal => app.execute_command(),
                                Panel::Assistant => {
                                    // For now, just echo the input to the output
                                    if !app.ai_input.is_empty() {
                                        app.ai_output.push(format!("> {}", app.ai_input));
                                        app.ai_output
                                            .push("AI response would be sent here.".to_string());
                                        
                                        // Add a separator after the message
                                        app.ai_output.push("─".repeat(40));
                                        
                                        app.ai_input.clear();
                                        app.ai_cursor_position = 0;
                                        
                                        // Set scroll to 0 to always show the most recent output at the bottom
                                        app.assistant_scroll = 0;
                                    }
                                }
                            }
                        }
                        KeyCode::PageUp => {
                            // Scroll up based on active panel
                            match app.active_panel {
                                Panel::Terminal => {
                                    if app.terminal_scroll < app.output.len().saturating_sub(1) {
                                        app.terminal_scroll += 1;
                                    }
                                }
                                Panel::Assistant => {
                                    if app.assistant_scroll < app.ai_output.len().saturating_sub(1) {
                                        app.assistant_scroll += 1;
                                    }
                                }
                            }
                        }
                        KeyCode::PageDown => {
                            // Scroll down based on active panel
                            match app.active_panel {
                                Panel::Terminal => {
                                    if app.terminal_scroll > 0 {
                                        app.terminal_scroll -= 1;
                                    }
                                }
                                Panel::Assistant => {
                                    if app.assistant_scroll > 0 {
                                        app.assistant_scroll -= 1;
                                    }
                                }
                            }
                        }
                        KeyCode::Tab => {
                            // Handle tab for autocomplete
                            match app.active_panel {
                                Panel::Terminal => {
                                    // Shift+Tab cycles backwards through suggestions
                                    let forward = key.modifiers != KeyModifiers::SHIFT;
                                    app.cycle_autocomplete(forward);
                                }
                                Panel::Assistant => {
                                    // No autocomplete for assistant panel
                                }
                            }
                        }
                        KeyCode::Char(c) => match app.active_panel {
                            Panel::Terminal => {
                                app.input.insert(app.cursor_position, c);
                                app.cursor_position += 1;
                                
                                // Clear autocomplete suggestions when typing
                                app.autocomplete_suggestions.clear();
                                app.autocomplete_index = None;
                                
                                // Set scroll to 0 to always show the most recent output
                                app.terminal_scroll = 0;
                            }
                            Panel::Assistant => {
                                app.ai_input.insert(app.ai_cursor_position, c);
                                app.ai_cursor_position += 1;
                                
                                // Set scroll to 0 to always show the most recent output
                                app.assistant_scroll = 0;
                            }
                        },
                        KeyCode::Backspace => match app.active_panel {
                            Panel::Terminal => {
                                if app.cursor_position > 0 {
                                    app.cursor_position -= 1;
                                    app.input.remove(app.cursor_position);
                                    
                                    // Clear autocomplete suggestions when editing
                                    app.autocomplete_suggestions.clear();
                                    app.autocomplete_index = None;
                                }
                            }
                            Panel::Assistant => {
                                if app.ai_cursor_position > 0 {
                                    app.ai_cursor_position -= 1;
                                    app.ai_input.remove(app.ai_cursor_position);
                                }
                            }
                        },
                        KeyCode::Delete => match app.active_panel {
                            Panel::Terminal => {
                                if app.cursor_position < app.input.len() {
                                    app.input.remove(app.cursor_position);
                                }
                            }
                            Panel::Assistant => {
                                if app.ai_cursor_position < app.ai_input.len() {
                                    app.ai_input.remove(app.ai_cursor_position);
                                }
                            }
                        },
                        KeyCode::Esc => {
                            return Ok(());
                        }
                        _ => {}
                    }
                }
            } else if let Event::Mouse(mouse_event) = event::read()? {
                match mouse_event.kind {
                    MouseEventKind::Down(MouseButton::Left) => {
                        // Check if click is near the divider (within 2 cells)
                        if let Some(divider_x) = app.divider_x {
                            if (mouse_event.column as i32 - divider_x as i32).abs() <= 2 {
                                app.is_dragging = true;
                            } else {
                                // Check which panel was clicked and set focus
                                if let Some(terminal_area) = app.terminal_area {
                                    if mouse_event.column >= terminal_area.x
                                        && mouse_event.column
                                            < terminal_area.x + terminal_area.width
                                    {
                                        app.active_panel = Panel::Terminal;
                                    }
                                }

                                if let Some(assistant_area) = app.assistant_area {
                                    if mouse_event.column >= assistant_area.x
                                        && mouse_event.column
                                            < assistant_area.x + assistant_area.width
                                    {
                                        app.active_panel = Panel::Assistant;
                                    }
                                }
                            }
                        }
                    }
                    MouseEventKind::Drag(MouseButton::Left) => {
                        if app.is_dragging {
                            if let (Some(terminal_area), Some(assistant_area)) =
                                (app.terminal_area, app.assistant_area)
                            {
                                // Calculate total width (excluding margins)
                                let total_width = terminal_area.width + assistant_area.width;

                                // Calculate new ratio based on mouse position
                                let new_x = mouse_event.column.saturating_sub(terminal_area.x);
                                let new_ratio =
                                    ((new_x as f32 / total_width as f32) * 100.0) as u16;

                                // Clamp ratio between 10% and 90%
                                app.panel_ratio = new_ratio.clamp(10, 90);
                            }
                        }
                    }
                    MouseEventKind::Up(MouseButton::Left) => {
                        app.is_dragging = false;
                    }
                    MouseEventKind::ScrollDown => {
                        // Determine which panel to scroll based on mouse position
                        if let (Some(terminal_area), Some(assistant_area)) = (app.terminal_area, app.assistant_area) {
                            if mouse_event.column >= terminal_area.x && mouse_event.column < terminal_area.x + terminal_area.width {
                                // Mouse is over terminal panel
                                if app.terminal_scroll > 0 {
                                    app.terminal_scroll -= 1;
                                }
                            } else if mouse_event.column >= assistant_area.x && mouse_event.column < assistant_area.x + assistant_area.width {
                                // Mouse is over assistant panel
                                if app.assistant_scroll > 0 {
                                    app.assistant_scroll -= 1;
                                }
                            }
                        }
                    }
                    MouseEventKind::ScrollUp => {
                        // Determine which panel to scroll based on mouse position
                        if let (Some(terminal_area), Some(assistant_area)) = (app.terminal_area, app.assistant_area) {
                            if mouse_event.column >= terminal_area.x && mouse_event.column < terminal_area.x + terminal_area.width {
                                // Mouse is over terminal panel
                                if app.terminal_scroll < app.output.len().saturating_sub(1) {
                                    app.terminal_scroll += 1;
                                }
                            } else if mouse_event.column >= assistant_area.x && mouse_event.column < assistant_area.x + assistant_area.width {
                                // Mouse is over assistant panel
                                if app.assistant_scroll < app.ai_output.len().saturating_sub(1) {
                                    app.assistant_scroll += 1;
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}
