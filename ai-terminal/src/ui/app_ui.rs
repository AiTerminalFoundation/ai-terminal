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
        main_flex.set_spacing(6); // Add a larger spacing between panels for the resize handle
        
        // Terminal panel (left side)
        let mut terminal_flex = Flex::new(0, 0, 0, 0, None);
        terminal_flex.set_type(fltk::group::FlexType::Column);
        
        // Terminal output
        let mut terminal_output = TextDisplay::new(0, 0, 0, 0, None);
        let terminal_buffer = TextBuffer::default();
        terminal_output.set_buffer(terminal_buffer);
        terminal_output.set_text_font(Font::Courier);
        terminal_output.set_frame(FrameType::FlatBox);
        terminal_output.set_color(Color::Black);
        terminal_output.set_text_color(Color::White);
        
        // Terminal input area
        let mut terminal_input_group = Flex::new(0, 0, 0, 35, None);
        terminal_input_group.set_type(fltk::group::FlexType::Row);
        
        let mut terminal_input = Input::new(0, 0, 0, 0, None);
        terminal_input.set_frame(FrameType::FlatBox);
        terminal_input.set_color(Color::GrayRamp);
        terminal_input.set_text_color(Color::White);
        terminal_input.set_label_color(Color::Yellow);
        terminal_input.set_label_font(Font::HelveticaBold);
        terminal_input.set_label(app.current_dir.to_string_lossy().as_ref());
        
        terminal_input_group.end();
        
        terminal_flex.end();
        terminal_flex.fixed(&terminal_input_group, 35);
        
        // AI Assistant panel (right side)
        let mut assistant_flex = Flex::new(0, 0, 0, 0, None);
        assistant_flex.set_type(fltk::group::FlexType::Column);
        
        // AI output
        let mut ai_output = TextDisplay::new(0, 0, 0, 0, None);
        let ai_buffer = TextBuffer::default();
        ai_output.set_buffer(ai_buffer);
        ai_output.set_text_font(Font::Courier);
        ai_output.set_frame(FrameType::FlatBox);
        ai_output.set_color(Color::Black);
        ai_output.set_text_color(Color::White);
        
        // AI input area
        let mut ai_input_group = Flex::new(0, 0, 0, 35, None);
        ai_input_group.set_type(fltk::group::FlexType::Row);
        
        let mut ai_input = Input::new(0, 0, 0, 35, None);
        ai_input.set_frame(FrameType::BorderFrame);
        ai_input.set_color(Color::Gray0);
        ai_input.set_text_color(Color::White);

        ai_input_group.end();

        assistant_flex.end();
        assistant_flex.fixed(&ai_input_group, 20);
        
        main_flex.end();
        
        // Set panel sizes (50/50 split by default)
        let terminal_width = (WINDOW_WIDTH as f64 * (app.panel_ratio as f64 / 100.0)) as i32;
        main_flex.fixed(&terminal_flex, terminal_width);
        
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
        
        AppUI {
            app,
            window,
            terminal_output,
            terminal_input,
            ai_output,
            ai_input,
            is_fullscreen: false,
        }
    }
    
    // Update the terminal output display
    pub fn update_terminal_output(&mut self) {
        let mut text = String::new();
        for line in &self.app.output {
            text.push_str(line);
            text.push('\n');
        }
        self.terminal_output.buffer().unwrap().set_text(&text);
        self.terminal_output.scroll(self.terminal_output.count_lines(0, self.terminal_output.buffer().unwrap().length(), true), 0);
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
                // Make sure we have at least 2 children (terminal and assistant panels)
                if flex_group.children() >= 2 {
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
    }
    
    // Execute a terminal command
    pub fn execute_command(&mut self) {
        let command = self.terminal_input.value();
        if command.is_empty() {
            return;
        }
        
        // Add command to output with prompt
        self.app.output.push(format!("> {}", command));
        
        // Execute the command
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
        self.terminal_input.set_label(self.app.current_dir.to_string_lossy().as_ref());
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
        }));
        
        // Terminal input events
        let app_ui_ref = Rc::clone(&app_ui);
        let mut terminal_input = self.terminal_input.clone();
        
        terminal_input.handle(move |_, event| {
            match event {
                Event::KeyDown => {
                    if fltk_app::event_key() == Key::Enter {
                        if let Ok(mut ui) = app_ui_ref.try_borrow_mut() {
                            ui.execute_command();
                        }
                        return true;
                    }
                }
                _ => {}
            }
            false
        });


        // AI input events
        let app_ui_ref = Rc::clone(&app_ui);
        let mut ai_input = self.ai_input.clone();
        
        ai_input.handle(move |_, event| {
            match event {
                Event::KeyDown => {
                    if fltk_app::event_key() == Key::Enter {
                        if let Ok(mut ui) = app_ui_ref.try_borrow_mut() {
                            ui.process_ai_input();
                        }
                        return true;
                    }
                }
                _ => {}
            }
            false
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
                        if let Ok(mut ui) = app_ui_ref.try_borrow_mut() {
                            ui.is_fullscreen = !ui.is_fullscreen;
                            win.fullscreen(ui.is_fullscreen);
                        }
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
                        
                        // If click is near the divider (within 30 pixels)
                        if x >= terminal_width - 30 && x <= terminal_width + 30 {
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