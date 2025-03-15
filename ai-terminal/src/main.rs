use std::io;
use std::process::{Command, Stdio};
use std::time::Duration;
use std::panic;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use tui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Terminal,
};

struct App {
    input: String,
    output: Vec<String>,
    cursor_position: usize,
}

impl App {
    fn new() -> Self {
        App {
            input: String::new(),
            output: vec!["Welcome to AI Terminal! Type commands below.".to_string()],
            cursor_position: 0,
        }
    }

    fn execute_command(&mut self) {
        let command = self.input.trim();
        if command.is_empty() {
            return;
        }

        self.output.push(format!("> {}", command));

        // Split the command into program and arguments
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() {
            return;
        }

        let program = parts[0];
        let args = &parts[1..];

        // Execute the command
        match Command::new(program)
            .args(args)
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
                        self.output.push(line.to_string());
                    }
                }

                // Add stderr to output
                if !stderr.is_empty() {
                    for line in stderr.lines() {
                        self.output.push(format!("Error: {}", line));
                    }
                }

                // Add exit status
                if !output.status.success() {
                    self.output.push(format!("Command exited with status: {}", output.status));
                }
            }
            Err(e) => {
                self.output.push(format!("Failed to execute command: {}", e));
            }
        }

        // Clear input
        self.input.clear();
        self.cursor_position = 0;
    }
}

// Function to restore terminal state in case of panic
fn restore_terminal() -> Result<(), io::Error> {
    disable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
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
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
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
        DisableMouseCapture
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
            
            // Create layout
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([
                    Constraint::Min(1),
                    Constraint::Length(3),
                ].as_ref())
                .split(size);

            // Output area
            let output_text = Text::from(
                app.output
                    .iter()
                    .map(|line| Line::from(line.clone()))
                    .collect::<Vec<Line>>()
            );
            
            let output_paragraph = Paragraph::new(output_text)
                .block(Block::default().borders(Borders::ALL).title("Output"))
                .wrap(Wrap { trim: true });
            
            f.render_widget(output_paragraph, chunks[0]);

            // Input area
            let input_text = Text::from(app.input.as_str());
            let input = Paragraph::new(input_text)
                .style(Style::default().fg(Color::Yellow))
                .block(Block::default().borders(Borders::ALL).title("Input"));
            
            f.render_widget(input, chunks[1]);
            
            // Cursor position
            f.set_cursor(
                chunks[1].x + app.cursor_position as u16 + 1,
                chunks[1].y + 1,
            );
        })?;

        // Handle input
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Enter => {
                            app.execute_command();
                        }
                        KeyCode::Char(c) => {
                            app.input.insert(app.cursor_position, c);
                            app.cursor_position += 1;
                        }
                        KeyCode::Backspace => {
                            if app.cursor_position > 0 {
                                app.cursor_position -= 1;
                                app.input.remove(app.cursor_position);
                            }
                        }
                        KeyCode::Delete => {
                            if app.cursor_position < app.input.len() {
                                app.input.remove(app.cursor_position);
                            }
                        }
                        KeyCode::Left => {
                            if app.cursor_position > 0 {
                                app.cursor_position -= 1;
                            }
                        }
                        KeyCode::Right => {
                            if app.cursor_position < app.input.len() {
                                app.cursor_position += 1;
                            }
                        }
                        KeyCode::Esc => {
                            return Ok(());
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}
