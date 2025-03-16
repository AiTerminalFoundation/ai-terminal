use std::env;
use std::io;
use std::panic;
use std::process::Command;
use std::time::Duration;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    Terminal,
};

// Import our modules
mod config;
mod model;
mod terminal;
mod ollama;
mod ui;

use model::App;

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

    // Check if we're running as a macOS app bundle
    let is_app_bundle = cfg!(target_os = "macos") && env::var("APP_BUNDLE").is_ok();
    
    // If running as a macOS app bundle, set the current directory to the user's home directory
    if is_app_bundle {
        if let Some(home_dir) = dirs_next::home_dir() {
            let _ = env::set_current_dir(&home_dir);
        }
    }

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        EnterAlternateScreen,
        EnableMouseCapture,
        // Enable focus reporting
        event::EnableFocusChange,
        // Set cursor to a thin line
        crossterm::cursor::SetCursorStyle::SteadyBar
    )?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut app = App::new();
    
    // If running as a macOS app, update the initial output
    if is_app_bundle {
        app.output.push("Running as a macOS application bundle.".to_string());
        app.output.push("Current directory set to your home directory.".to_string());
        
        // Update the current directory in the app state
        if let Some(home_dir) = dirs_next::home_dir() {
            app.current_dir = home_dir;
        }
    }

    // Main loop
    let result = run_app(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture,
        // Disable focus reporting
        event::DisableFocusChange,
        // Reset cursor style to default
        crossterm::cursor::SetCursorStyle::DefaultUserShape
    )?;
    terminal.show_cursor()?;

    // Handle any errors from the app
    if let Err(err) = result {
        println!("Error: {}", err);
    }

    Ok(())
}

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> Result<(), io::Error> {
    loop {
        // Draw UI
        terminal.draw(|f| ui::draw::draw_ui::<B>(f, app))?;

        // Handle input with a timeout
        if event::poll(Duration::from_millis(100))? {
            if let Some(should_quit) = ui::events::handle_event(app)? {
                if should_quit {
                    return Ok(());
                }
            }
        }
    }
}

// Function to extract commands from AI response
fn extract_commands(response: &str) -> Vec<String> {
    let mut commands = Vec::new();
    let mut in_code_block = false;
    let mut current_command = String::new();
    
    for line in response.lines() {
        let trimmed = line.trim();
        
        // Check for code block markers
        if trimmed.starts_with("```") {
            if !in_code_block {
                // Start of code block
                in_code_block = true;
                // Skip the opening line if it contains a language specifier
                // e.g., ```bash, ```sh, etc.
                continue;
            } else {
                // End of code block
                if !current_command.trim().is_empty() {
                    commands.push(current_command.trim().to_string());
                }
                current_command = String::new();
                in_code_block = false;
            }
        } else if in_code_block {
            // Inside code block, collect command
            current_command.push_str(line);
            current_command.push('\n');
        }
    }
    
    // In case there's an unclosed code block
    if in_code_block && !current_command.trim().is_empty() {
        commands.push(current_command.trim().to_string());
    }
    
    commands
}

// Function to detect OS information
fn detect_os_info() -> String {
    let mut os_info = String::new();
    
    // Get OS name and version
    if let Ok(os_release) = Command::new("uname").arg("-a").output() {
        if os_release.status.success() {
            let output = String::from_utf8_lossy(&os_release.stdout).trim().to_string();
            os_info = output;
        }
    }
    
    // If uname failed (e.g., on Windows), try alternative methods
    if os_info.is_empty() {
        if cfg!(target_os = "windows") {
            os_info = "Windows".to_string();
            // Try to get Windows version
            if let Ok(ver) = Command::new("cmd").args(["/C", "ver"]).output() {
                if ver.status.success() {
                    let output = String::from_utf8_lossy(&ver.stdout).trim().to_string();
                    os_info = output;
                }
            }
        } else if cfg!(target_os = "macos") {
            os_info = "macOS".to_string();
            // Try to get macOS version
            if let Ok(ver) = Command::new("sw_vers").output() {
                if ver.status.success() {
                    let output = String::from_utf8_lossy(&ver.stdout).trim().to_string();
                    os_info = output;
                }
            }
        } else if cfg!(target_os = "linux") {
            os_info = "Linux".to_string();
            // Try to get Linux distribution
            if let Ok(ver) = Command::new("cat").arg("/etc/os-release").output() {
                if ver.status.success() {
                    let output = String::from_utf8_lossy(&ver.stdout).trim().to_string();
                    if let Some(name_line) = output.lines().find(|l| l.starts_with("PRETTY_NAME=")) {
                        if let Some(name) = name_line.strip_prefix("PRETTY_NAME=") {
                            os_info = name.trim_matches('"').to_string();
                        }
                    }
                }
            }
        }
    }
    
    // If all else fails, use Rust's built-in OS detection
    if os_info.is_empty() {
        os_info = format!("OS: {}", env::consts::OS);
    }
    
    os_info
}
