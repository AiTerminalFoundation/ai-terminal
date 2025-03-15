use std::env;
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
    text::{Line, Text, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
    Terminal,
};

struct App {
    input: String,
    output: Vec<String>,
    cursor_position: usize,
    current_dir: PathBuf,
    // AI assistant fields
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
}

enum Panel {
    Terminal,
    Assistant,
}

impl App {
    fn new() -> Self {
        let current_dir = env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));

        App {
            input: String::new(),
            output: vec![
                "Welcome to AI Terminal! Type commands below.".to_string(),
                format!("Current directory: {}", current_dir.display()),
                "Use Alt+Left/Right to resize panels.".to_string(),
                "Click on a panel to focus it.".to_string(),
                "Drag the divider between panels to resize them.".to_string(),
                "Use PageUp/PageDown or mouse wheel to scroll through output.".to_string(),
                "Use Alt+Up/Down to scroll through output.".to_string(),
            ],
            cursor_position: 0,
            current_dir,
            // Initialize AI assistant fields
            ai_input: String::new(),
            ai_output: vec![
                "AI Assistant ready. Type your message below.".to_string(),
                "Use Alt+Left/Right to resize panels.".to_string(),
                "Click on a panel to focus it.".to_string(),
                "Drag the divider between panels to resize them.".to_string(),
                "Use PageUp/PageDown or mouse wheel to scroll through output.".to_string(),
                "Use Alt+Up/Down to scroll through output.".to_string(),
            ],
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
        }
    }

    fn execute_command(&mut self) {
        let command = self.input.clone();
        let command = command.trim();
        if command.is_empty() {
            return;
        }

        self.output.push(format!("> {}", command));

        // Handle cd command specially
        if command.starts_with("cd ") {
            let path = command.trim_start_matches("cd ").trim();
            self.change_directory(path);
        } else {
            // Execute the command
            let output = self.run_command(command);
            self.output.extend(output);
        }

        self.input.clear();
        self.cursor_position = 0;
        
        // Auto-scroll to the bottom when new content is added
        self.terminal_scroll = 0;
    }

    fn run_command(&self, command: &str) -> Vec<String> {
        let mut result = Vec::new();
        
        // Split the command into program and arguments
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() {
            return result;
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
                }
            }
            Err(e) => {
                result.push(format!("Failed to execute command: {}", e));
            }
        }

        result
    }

    fn change_directory(&mut self, path: &str) {
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
                return;
            }
        } else if path == ".." {
            // Parent directory
            if let Some(parent) = self.current_dir.parent() {
                PathBuf::from(parent)
            } else {
                self.output
                    .push("Error: Already at root directory".to_string());
                return;
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
            }
            Err(e) => {
                self.output.push(format!("Error changing directory: {}", e));
            }
        }
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
                    .map(|(i, line)| {
                        if line.starts_with("> ") {
                            // Command line - add a separator before it
                            if i > 0 {
                                Line::from(vec![
                                    Span::styled(
                                        "─".repeat(terminal_chunks[0].width as usize - 2),
                                        Style::default().fg(Color::DarkGray),
                                    )
                                ])
                            } else {
                                Line::from(line.clone())
                            }
                        } else {
                            Line::from(line.clone())
                        }
                    })
                    .collect::<Vec<Line>>(),
            );

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
                    .map(|(i, line)| {
                        if line.starts_with("> ") {
                            // User message - add a separator before it
                            if i > 0 {
                                Line::from(vec![
                                    Span::styled(
                                        "─".repeat(assistant_chunks[0].width as usize - 2),
                                        Style::default().fg(Color::DarkGray),
                                    )
                                ])
                            } else {
                                Line::from(line.clone())
                            }
                        } else {
                            Line::from(line.clone())
                        }
                    })
                    .collect::<Vec<Line>>(),
            );

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
                                        app.ai_input.clear();
                                        app.ai_cursor_position = 0;
                                        
                                        // Auto-scroll to the bottom when new content is added
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
                        KeyCode::Char(c) => match app.active_panel {
                            Panel::Terminal => {
                                app.input.insert(app.cursor_position, c);
                                app.cursor_position += 1;
                            }
                            Panel::Assistant => {
                                app.ai_input.insert(app.ai_cursor_position, c);
                                app.ai_cursor_position += 1;
                            }
                        },
                        KeyCode::Backspace => match app.active_panel {
                            Panel::Terminal => {
                                if app.cursor_position > 0 {
                                    app.cursor_position -= 1;
                                    app.input.remove(app.cursor_position);
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
