use fltk::{
    app as fltk_app,
    enums::{Color, Event, Font, FrameType, Key},
    group::Flex,
    input::Input,
    prelude::*,
    text::{TextBuffer, TextDisplay},
    window::Window,
};
use std::rc::Rc;
use std::cell::RefCell;

use crate::config::{
    WINDOW_HEIGHT, WINDOW_WIDTH,
};
use crate::model::App;
use crate::ollama::commands;
use crate::terminal::commands as term_commands;

pub struct AppUI {
    pub app: App,
    pub window: Window,
    pub terminal_output: TextDisplay,
    pub terminal_input: Input,
    pub ai_output: TextDisplay,
    pub ai_input: Input,
    pub is_fullscreen: bool,
    pub resize_handle: fltk::frame::Frame,
    pub active_panel: ActivePanel,
    pub terminal_style_buffer: TextBuffer,
}

// Enum to track which panel is active
#[derive(Clone, Copy, PartialEq)]
pub enum ActivePanel {
    Terminal,
    AI,
}

impl AppUI {
    pub fn new() -> Self {
        // Create app state
        let app = App::new();
        
        // Create window
        let mut window = Window::new(100, 100, WINDOW_WIDTH, WINDOW_HEIGHT, "AI Terminal");
        
        // Create main layout
        let mut main_flex = Flex::new(0, 0, WINDOW_WIDTH, WINDOW_HEIGHT, None);
        main_flex.set_type(fltk::group::FlexType::Row);
        main_flex.set_spacing(2); // Smaller spacing between panels
        
        // Terminal panel (left side)
        let mut terminal_flex = Flex::new(0, 0, 0, 0, None);
        terminal_flex.set_type(fltk::group::FlexType::Column);
        terminal_flex.set_spacing(2); // Add spacing between elements
        
        // Terminal output
        let mut terminal_output = TextDisplay::new(0, 0, 0, 0, None);
        let terminal_buffer = TextBuffer::default();
        terminal_output.set_buffer(terminal_buffer);
        terminal_output.set_text_font(Font::Courier);
        terminal_output.set_frame(FrameType::BorderBox);
        terminal_output.set_color(Color::Black);
        terminal_output.set_text_color(Color::White);
        terminal_output.wrap_mode(fltk::text::WrapMode::AtBounds, 0);
        terminal_output.set_cursor_style(fltk::text::Cursor::Simple);
        terminal_output.show_cursor(true);
        
        // Create and set up style buffer for terminal output
        let terminal_style_buffer = TextBuffer::default();
        terminal_output.set_highlight_data(
            terminal_style_buffer.clone(),
            vec![
                fltk::text::StyleTableEntry {
                    color: Color::White,
                    font: Font::Courier,
                    size: 0,
                },
                fltk::text::StyleTableEntry {
                    color: Color::Green,
                    font: Font::Courier,
                    size: 0,
                },
                fltk::text::StyleTableEntry {
                    color: Color::Red,
                    font: Font::Courier,
                    size: 0,
                },
            ],
        );
        
        // Customize scrollbars for terminal output
        terminal_output.set_scrollbar_size(10); // Thinner scrollbar
        terminal_output.set_scrollbar_align(fltk::enums::Align::Right);
        
        // White separator
        let mut separator = Flex::new(0, 0, 0, 2, None);
        separator.set_frame(FrameType::FlatBox);
        separator.set_color(Color::White);
        separator.end();
        
        // Terminal input area
        let mut terminal_input_group = Flex::new(0, 0, 0, 35, None);
        terminal_input_group.set_type(fltk::group::FlexType::Row);
        
        let mut terminal_input = Input::new(0, 0, 0, 0, None);
        terminal_input.set_frame(FrameType::BorderBox);
        terminal_input.set_color(Color::Black);
        terminal_input.set_text_color(Color::White);
        terminal_input.set_label_color(Color::Green); // Make directory label green
        terminal_input.set_label_font(Font::Courier);
        terminal_input.set_label_size(12);

        terminal_input_group.end();
        
        terminal_flex.end();
        terminal_flex.fixed(&separator, 2);
        terminal_flex.fixed(&terminal_input_group, 40);
        
        // Create a thin resize handle between panels
        let mut resize_handle = fltk::frame::Frame::new(0, 0, 2, WINDOW_HEIGHT, None);
        resize_handle.set_frame(FrameType::FlatBox);
        resize_handle.set_color(Color::White);
        
        // AI Assistant panel (right side)
        let mut assistant_flex = Flex::new(0, 0, 0, 0, None);
        assistant_flex.set_type(fltk::group::FlexType::Column);
        assistant_flex.set_spacing(2); // Add spacing between elements
        
        // AI output
        let mut ai_output = TextDisplay::new(0, 0, 0, 0, None);
        let ai_buffer = TextBuffer::default();
        ai_output.set_buffer(ai_buffer);
        ai_output.set_text_font(Font::Courier);
        ai_output.set_frame(FrameType::BorderBox);
        ai_output.set_color(Color::Black);
        ai_output.set_text_color(Color::White);
        ai_output.wrap_mode(fltk::text::WrapMode::AtBounds, 0);
        ai_output.set_cursor_style(fltk::text::Cursor::Simple);
        ai_output.show_cursor(true);
        
        // Customize scrollbars for AI output
        ai_output.set_scrollbar_size(10); // Thinner scrollbar
        ai_output.set_scrollbar_align(fltk::enums::Align::Right);
        
        // White separator
        let mut ai_separator = Flex::new(0, 0, 0, 2, None);
        ai_separator.set_frame(FrameType::FlatBox);
        ai_separator.set_color(Color::White);
        ai_separator.end();
        
        // AI input area
        let mut ai_input_group = Flex::new(0, 0, 0, 40, None);
        ai_input_group.set_type(fltk::group::FlexType::Row);
        
        let mut ai_input = Input::new(0, 0, 0, 0, None);
        ai_input.set_frame(FrameType::BorderBox);
        ai_input.set_color(Color::Black);
        ai_input.set_text_color(Color::White);

        ai_input_group.end();

        assistant_flex.end();
        assistant_flex.fixed(&ai_separator, 2);
        assistant_flex.fixed(&ai_input_group, 40);
        
        main_flex.end();
        
        // Set panel sizes (50/50 split by default)
        let terminal_width = (WINDOW_WIDTH as f64 * (app.panel_ratio as f64 / 100.0)) as i32;
        main_flex.fixed(&terminal_flex, terminal_width);
        main_flex.fixed(&resize_handle, 2);
        
        // Make the main flex resizable
        main_flex.set_margin(0);
        window.resizable(&main_flex);
        
        // Add a resize handle between panels
        terminal_flex.resizable(&terminal_output);
        assistant_flex.resizable(&ai_output);
        
        // Enable border and make window resizable
        window.make_resizable(true);
        window.end();
        window.show();
        
        // Initialize output text
        let mut terminal_text = String::new();
        for line in &app.output {
            terminal_text.push_str(line);
            terminal_text.push('\n');
        }
        terminal_output.buffer().unwrap().set_text(&terminal_text);
        
        let mut ai_text = String::new();
        for line in &app.ai_output {
            ai_text.push_str(line);
            ai_text.push('\n');
        }
        ai_output.buffer().unwrap().set_text(&ai_text);
        
        // Create the AppUI instance
        let mut app_ui = AppUI {
            app,
            window,
            terminal_output,
            terminal_input,
            ai_output,
            ai_input,
            is_fullscreen: false,
            resize_handle,
            active_panel: ActivePanel::Terminal, // Default to terminal panel
            terminal_style_buffer,
        };
        
        // Set the initial terminal input label to show current directory
        app_ui.update_terminal_input_label();
        
        // Highlight the active panel initially
        app_ui.highlight_active_panel();
        
        app_ui
    }
    
    // Update the terminal output display
    pub fn update_terminal_output(&mut self) {
        let mut buffer = self.terminal_output.buffer().unwrap();
        
        // Clear both buffers
        buffer.set_text("");
        self.terminal_style_buffer.set_text("");
        
        // Calculate the separator width based on current panel size
        let separator_width = self.calculate_separator_width();
        
        // Iterate through output lines and command status together
        let mut status_index = 0;
        let mut prev_line_was_command = false;
        
        for line in &self.app.output {
            // Check if this line is a command (starts with "> ")
            let is_command = line.starts_with("> ");
            
            // Add separator line before a new command (except for the first command)
            if is_command && prev_line_was_command == false && status_index > 0 {
                // Add a separator line with a blank line before it
                buffer.append("\n");
                let style_text = "A"; // Style for blank line
                self.terminal_style_buffer.append(&style_text);
                
                // Create a visible separator line that spans the full width
                let separator = "─".repeat(separator_width);
                buffer.append(&separator);
                buffer.append("\n");
                
                // Style for separator line (white color)
                let style_text = "A".repeat(separator.len() + 1);
                self.terminal_style_buffer.append(&style_text);
                
                // Add another blank line after the separator
                buffer.append("\n");
                let style_text = "A"; // Style for blank line
                self.terminal_style_buffer.append(&style_text);
            }
            
            // Add the actual line
            buffer.append(line);
            buffer.append("\n");
            
            // Apply styling
            if is_command && status_index < self.app.command_status.len() {
                // Set color based on command status
                let style_char = match self.app.command_status[status_index] {
                    crate::model::CommandStatus::Success => 'B', // Index 1 (Green)
                    crate::model::CommandStatus::Failure => 'C', // Index 2 (Red)
                    crate::model::CommandStatus::Running => 'A', // Index 0 (White) for running commands
                };
                
                // Apply style to the command line
                let style_text = style_char.to_string().repeat(line.len()) + "A"; // 'A' for newline
                self.terminal_style_buffer.append(&style_text);
                
                status_index += 1;
                prev_line_was_command = true;
            } else {
                // Regular output line, use default style (white)
                let style_text = "A".repeat(line.len() + 1); // 'A' for default style
                self.terminal_style_buffer.append(&style_text);
                prev_line_was_command = false;
            }
        }
        
        // Scroll to the bottom
        self.terminal_output.scroll(self.terminal_output.count_lines(0, buffer.length(), true), 0);
    }
    
    // Update the AI output display
    pub fn update_ai_output(&mut self) {
        let mut text = String::new();
        for line in &self.app.ai_output {
            text.push_str(line);
            text.push('\n');
        }
        self.ai_output.buffer().unwrap().set_text(&text);
        self.ai_output.scroll(self.ai_output.count_lines(0, self.ai_output.buffer().unwrap().length(), true), 0);
    }
    
    // Update panel layout
    pub fn update_panel_layout(&mut self) {
        // Recalculate the panel sizes based on the current ratio
        let window_width = if self.is_fullscreen {
            fltk_app::screen_size().0 as i32
        } else {
            self.window.width()
        };
        
        let new_width = (window_width as f64 * (self.app.panel_ratio as f64 / 100.0)) as i32;
        
        // Get the main flex container - the first child of the window
        if let Some(flex) = self.window.child(0) {
            if let Some(flex_group) = flex.as_group() {
                // Make sure we have at least 3 children (terminal, resize handle, and assistant panels)
                if flex_group.children() >= 3 {
                    // Get the terminal panel (first child)
                    if let Some(terminal_panel) = flex_group.child(0) {
                        // Update the layout - using unsafe because into_widget requires it
                        unsafe {
                            let mut flex_mut = flex.clone().into_widget::<Flex>();
                            flex_mut.fixed(&terminal_panel, new_width);
                            flex_mut.layout();
                            flex_mut.redraw();
                        }
                    }
                }
            }
        }
        
        // Update the terminal output to adjust separator lines
        self.update_terminal_output();
    }
    
    // Calculate the appropriate separator line width
    fn calculate_separator_width(&self) -> usize {
        // Get the width of the terminal panel
        let panel_width = self.terminal_output.width();
        
        // Calculate how many separator characters we need
        // Each character is approximately 8 pixels wide in most monospace fonts
        // We subtract a small margin to ensure it doesn't overflow
        let char_count = (panel_width / 8).max(10) - 2;
        
        char_count as usize
    }
    
    // Execute a terminal command
    pub fn execute_command(&mut self) {
        let command = self.terminal_input.value();
        if command.is_empty() {
            return;
        }
        
        // Add command to output with prompt
        self.app.output.push(format!("> {}", command));
        
        // Handle cd command specially
        if command.starts_with("cd ") {
            let path = command.trim_start_matches("cd ").trim();
            
            // Implement directory change logic directly
            let new_dir = if path.starts_with('/') {
                // Absolute path
                std::path::PathBuf::from(path)
            } else if path == "~" || path.starts_with("~/") {
                // Home directory
                if let Some(home) = dirs_next::home_dir() {
                    if path == "~" {
                        home
                    } else {
                        home.join(path.trim_start_matches("~/"))
                    }
                } else {
                    self.app.output.push("Error: Could not determine home directory".to_string());
                    
                    // Update command status
                    self.app.command_status.push(crate::model::CommandStatus::Failure);
                    
                    // Clear input and update display
                    self.terminal_input.set_value("");
                    self.update_terminal_output();
                    return;
                }
            } else if path == ".." {
                // Parent directory
                if let Some(parent) = self.app.current_dir.parent() {
                    std::path::PathBuf::from(parent)
                } else {
                    self.app.output.push("Error: Already at root directory".to_string());
                    
                    // Update command status
                    self.app.command_status.push(crate::model::CommandStatus::Failure);
                    
                    // Clear input and update display
                    self.terminal_input.set_value("");
                    self.update_terminal_output();
                    return;
                }
            } else {
                // Relative path
                self.app.current_dir.join(path)
            };
            
            // Try to change to the new directory
            match std::env::set_current_dir(&new_dir) {
                Ok(_) => {
                    self.app.current_dir = new_dir;
                    self.app.output.push(format!("Changed directory to: {}", self.app.current_dir.display()));
                    
                    // Update command status
                    self.app.command_status.push(crate::model::CommandStatus::Success);
                }
                Err(e) => {
                    self.app.output.push(format!("Error: {}", e));
                    
                    // Update command status
                    self.app.command_status.push(crate::model::CommandStatus::Failure);
                }
            }
        } else {
            // Execute other commands normally
            let (output, success) = term_commands::execute_command(&command, &self.app.current_dir);
            
            // Add command output to terminal output
            for line in output.iter() {
                self.app.output.push(line.clone());
            }
            
            // Update command status
            self.app.command_status.push(if success {
                crate::model::CommandStatus::Success
            } else {
                crate::model::CommandStatus::Failure
            });
            
            // Store command and output for context
            self.app.last_terminal_context = Some((command.clone(), output));
        }
        
        // Add command to history
        self.app.command_history.push(command);
        if self.app.command_history.len() > crate::config::MAX_COMMAND_HISTORY {
            self.app.command_history.remove(0);
        }
        
        // Clear input
        self.terminal_input.set_value("");
        
        // Update display
        self.update_terminal_output();
        
        // Update terminal input label with current directory
        self.update_terminal_input_label();
    }
    
    // Update terminal input label with current directory
    pub fn update_terminal_input_label(&mut self) {
        // Set the label to the current directory
        let dir_str = self.app.current_dir.to_string_lossy();
        self.terminal_input.set_label(&format!("{}> ", dir_str));
    }
    
    // Process AI input
    pub fn process_ai_input(&mut self) {
        let input = self.ai_input.value();
        if input.is_empty() {
            return;
        }
        
        // Update app state
        self.app.ai_input = input;
        
        // Process the input
        commands::process_ai_input(&mut self.app);
        
        // Clear input
        self.ai_input.set_value("");
        
        // Update display
        self.update_ai_output();
    }
    
    // Highlight the active panel
    pub fn highlight_active_panel(&mut self) {
        match self.active_panel {
            ActivePanel::Terminal => {
                // Highlight terminal input
                self.terminal_input.set_color(Color::from_rgb(20, 20, 30)); // Slightly lighter black for active
                self.ai_input.set_color(Color::Black);
            },
            ActivePanel::AI => {
                // Highlight AI input
                self.terminal_input.set_color(Color::Black);
                self.ai_input.set_color(Color::from_rgb(20, 20, 30)); // Slightly lighter black for active
            }
        }
        self.terminal_input.redraw();
        self.ai_input.redraw();
    }
    
    // Setup event handlers
    pub fn setup_events(&mut self) {
        // Create a shared reference to self
        let app_ui = Rc::new(RefCell::new(AppUI {
            app: self.app.clone(),
            window: self.window.clone(),
            terminal_output: self.terminal_output.clone(),
            terminal_input: self.terminal_input.clone(),
            ai_output: self.ai_output.clone(),
            ai_input: self.ai_input.clone(),
            is_fullscreen: self.is_fullscreen,
            resize_handle: self.resize_handle.clone(),
            active_panel: self.active_panel,
            terminal_style_buffer: self.terminal_style_buffer.clone(),
        }));
        
        // Terminal input events
        let app_ui_ref = Rc::clone(&app_ui);
        let mut terminal_input = self.terminal_input.clone();
        
        terminal_input.handle(move |_, event| {
            match event {
                Event::Push => {
                    if let Ok(mut ui) = app_ui_ref.try_borrow_mut() {
                        ui.active_panel = ActivePanel::Terminal;
                        ui.highlight_active_panel();
                    }
                    false
                },
                Event::KeyDown => {
                    if fltk_app::event_key() == Key::Enter {
                        if let Ok(mut ui) = app_ui_ref.try_borrow_mut() {
                            ui.execute_command();
                        }
                        return true;
                    }
                    false
                }
                _ => false,
            }
        });

        // AI input events
        let app_ui_ref = Rc::clone(&app_ui);
        let mut ai_input = self.ai_input.clone();
        
        ai_input.handle(move |_, event| {
            match event {
                Event::Push => {
                    if let Ok(mut ui) = app_ui_ref.try_borrow_mut() {
                        ui.active_panel = ActivePanel::AI;
                        ui.highlight_active_panel();
                    }
                    false
                },
                Event::KeyDown => {
                    if fltk_app::event_key() == Key::Enter {
                        if let Ok(mut ui) = app_ui_ref.try_borrow_mut() {
                            ui.process_ai_input();
                        }
                        return true;
                    }
                    false
                }
                _ => false,
            }
        });
        
        // Window event handling for fullscreen
        let app_ui_ref = Rc::clone(&app_ui);
        
        self.window.handle(move |win, event| {
            match event {
                Event::Resize => {
                    // Update fullscreen state based on window size
                    if let Ok(mut ui) = app_ui_ref.try_borrow_mut() {
                        let (w, h) = fltk_app::screen_size();
                        let (win_w, win_h) = (win.width(), win.height());
                        
                        // Check if window dimensions match screen dimensions
                        ui.is_fullscreen = win_w >= w as i32 - 5 && win_h >= h as i32 - 5;
                        
                        // Update resize handle height
                        ui.resize_handle.set_size(2, win_h);
                        
                        // Update the terminal output to adjust separator lines for the new size
                        ui.update_terminal_output();
                    }
                    true
                }
                Event::Fullscreen => {
                    // Handle fullscreen event
                    if let Ok(mut ui) = app_ui_ref.try_borrow_mut() {
                        ui.is_fullscreen = !ui.is_fullscreen;
                    }
                    true
                }
                Event::KeyDown => {
                    // Add F11 shortcut for fullscreen toggle
                    if fltk_app::event_key() == Key::F11 {
                        let mut toggle_fullscreen = false;
                        if let Ok(mut ui) = app_ui_ref.try_borrow_mut() {
                            ui.is_fullscreen = !ui.is_fullscreen;
                            toggle_fullscreen = ui.is_fullscreen;
                        }
                        win.fullscreen(toggle_fullscreen);
                        return true;
                    }
                    false
                }
                Event::Push => {
                    // Check if we're near the divider between panels
                    let x = fltk_app::event_x();
                    
                    // Get the current panel width
                    if let Ok(mut ui) = app_ui_ref.try_borrow_mut() {
                        let terminal_width = (win.width() as f64 * (ui.app.panel_ratio as f64 / 100.0)) as i32;
                        
                        // If click is near the divider (within 10 pixels)
                        if x >= terminal_width - 5 && x <= terminal_width + 7 {
                            // Set a flag or state to indicate we're resizing
                            ui.app.is_resizing = true;
                            return true;
                        }
                    }
                    false
                }
                Event::Drag => {
                    // If we're resizing, update the panel ratio
                    if let Ok(mut ui) = app_ui_ref.try_borrow_mut() {
                        if ui.app.is_resizing {
                            let x = fltk_app::event_x();
                            let new_ratio = (x as f64 / win.width() as f64 * 100.0) as i32;
                            
                            // Limit the ratio to reasonable bounds (10% to 90%)
                            if new_ratio >= 10 && new_ratio <= 90 {
                                ui.app.panel_ratio = new_ratio as u32;
                                
                                // Update the layout
                                ui.update_panel_layout();
                                return true;
                            }
                        }
                    }
                    false
                }
                Event::Released => {
                    // Stop resizing
                    if let Ok(mut ui) = app_ui_ref.try_borrow_mut() {
                        ui.app.is_resizing = false;
                    }
                    false
                }
                _ => false,
            }
        });
    }
} 