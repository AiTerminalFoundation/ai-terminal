use crate::model::{App, Panel};
use crossterm::event::{
    self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind,
};
use std::io;

pub fn handle_event(app: &mut App) -> io::Result<Option<bool>> {
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
                    match app.active_panel {
                        Panel::Terminal => {
                            // Navigate command history (if any)
                            if !app.command_history.is_empty() {
                                // Store the current position in history
                                let history_position =
                                    app.history_position.unwrap_or(app.command_history.len());

                                if history_position > 0 {
                                    // Move up in history (older commands)
                                    let new_position = history_position - 1;
                                    app.history_position = Some(new_position);

                                    // Replace input with the historical command
                                    app.input = app.command_history[new_position].clone();
                                    app.cursor_position = app.input.len();
                                }
                            }
                        }
                        Panel::Assistant => {
                            // No history navigation for assistant panel
                        }
                    }
                }
                KeyCode::Down => {
                    match app.active_panel {
                        Panel::Terminal => {
                            // Navigate command history (if any)
                            if let Some(history_position) = app.history_position {
                                if history_position < app.command_history.len() - 1 {
                                    // Move down in history (newer commands)
                                    let new_position = history_position + 1;
                                    app.history_position = Some(new_position);

                                    // Replace input with the historical command
                                    app.input = app.command_history[new_position].clone();
                                    app.cursor_position = app.input.len();
                                } else {
                                    // At the end of history, clear the input
                                    app.history_position = None;
                                    app.input.clear();
                                    app.cursor_position = 0;
                                }
                            }
                        }
                        Panel::Assistant => {
                            // No history navigation for assistant panel
                        }
                    }
                }
                KeyCode::Enter => {
                    match app.active_panel {
                        Panel::Terminal => {
                            app.execute_command();
                            // Reset history position when executing a command
                            app.history_position = None;
                        }
                        Panel::Assistant => {
                            // Send the input to the AI assistant
                            app.send_to_ai_assistant();

                            // Set scroll to 0 to always show the most recent output at the bottom
                            app.assistant_scroll = 0;
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
                    return Ok(Some(true));
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
                                && mouse_event.column < terminal_area.x + terminal_area.width
                            {
                                app.active_panel = Panel::Terminal;
                            }
                        }

                        if let Some(assistant_area) = app.assistant_area {
                            if mouse_event.column >= assistant_area.x
                                && mouse_event.column < assistant_area.x + assistant_area.width
                            {
                                app.active_panel = Panel::Assistant;

                                // Check if a command was clicked
                                if let Some(assistant_chunks) = app.assistant_area.map(|area| {
                                    ratatui::layout::Layout::default()
                                        .direction(ratatui::layout::Direction::Vertical)
                                        .constraints(
                                            [
                                                ratatui::layout::Constraint::Min(1),
                                                ratatui::layout::Constraint::Length(3),
                                            ]
                                            .as_ref(),
                                        )
                                        .split(area)
                                }) {
                                    let ai_output_area = assistant_chunks[0];

                                    // Check if click is within the AI output area
                                    if mouse_event.column >= ai_output_area.x
                                        && mouse_event.column
                                            < ai_output_area.x + ai_output_area.width
                                        && mouse_event.row >= ai_output_area.y
                                        && mouse_event.row
                                            < ai_output_area.y + ai_output_area.height
                                    {
                                        // Calculate which line was clicked
                                        let _visible_height =
                                            ai_output_area.height.saturating_sub(2);
                                        let scroll_offset = app.assistant_scroll as u16;
                                        let clicked_line = mouse_event
                                            .row
                                            .saturating_sub(ai_output_area.y + 1)
                                            .saturating_add(scroll_offset);

                                        // Check if the clicked line contains a command
                                        for &(line_idx, ref cmd) in &app.extracted_commands {
                                            if line_idx as u16 == clicked_line {
                                                // Get the line content to check if click is on one of the buttons
                                                if let Some(line_content) =
                                                    app.ai_output.get(line_idx)
                                                {
                                                    let line_start_x = ai_output_area.x + 1; // +1 for border
                                                    let _line_length = line_content.len() as u16;

                                                    // Check if click is on the copy button (📋)
                                                    // The copy button is at the end of the line
                                                    // Find the positions of the buttons in the line
                                                    let copy_button_start =
                                                        line_content.find("[📋]").unwrap_or(0);
                                                    let execute_button_start =
                                                        line_content.find("[▶️]").unwrap_or(0);

                                                    // Calculate the actual screen positions
                                                    let copy_button_x_start =
                                                        line_start_x + copy_button_start as u16;
                                                    let copy_button_x_end = copy_button_x_start + 4; // "[📋]" is 4 chars wide

                                                    let execute_button_x_start =
                                                        line_start_x + execute_button_start as u16;
                                                    let execute_button_x_end =
                                                        execute_button_x_start + 4; // "[▶️]" is 4 chars wide

                                                    // Check if click is on the copy button
                                                    if mouse_event.column >= copy_button_x_start
                                                        && mouse_event.column < copy_button_x_end
                                                    {
                                                        // Copy button clicked
                                                        let command_to_copy = cmd.clone();
                                                        app.copy_command_to_terminal(
                                                            &command_to_copy,
                                                        );
                                                        break;
                                                    }
                                                    // Check if click is on the execute button
                                                    else if mouse_event.column
                                                        >= execute_button_x_start
                                                        && mouse_event.column < execute_button_x_end
                                                    {
                                                        // Execute button clicked
                                                        let command_to_execute = cmd.clone();
                                                        app.copy_and_execute_command(
                                                            &command_to_execute,
                                                        );
                                                        break;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
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
                        let new_ratio = ((new_x as f32 / total_width as f32) * 100.0) as u16;

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
                if let (Some(terminal_area), Some(assistant_area)) =
                    (app.terminal_area, app.assistant_area)
                {
                    if mouse_event.column >= terminal_area.x
                        && mouse_event.column < terminal_area.x + terminal_area.width
                    {
                        // Mouse is over terminal panel
                        if app.terminal_scroll > 0 {
                            app.terminal_scroll -= 1;
                        }
                    } else if mouse_event.column >= assistant_area.x
                        && mouse_event.column < assistant_area.x + assistant_area.width
                    {
                        // Mouse is over assistant panel
                        if app.assistant_scroll > 0 {
                            app.assistant_scroll -= 1;
                        }
                    }
                }
            }
            MouseEventKind::ScrollUp => {
                // Determine which panel to scroll based on mouse position
                if let (Some(terminal_area), Some(assistant_area)) =
                    (app.terminal_area, app.assistant_area)
                {
                    if mouse_event.column >= terminal_area.x
                        && mouse_event.column < terminal_area.x + terminal_area.width
                    {
                        // Mouse is over terminal panel
                        if app.terminal_scroll < app.output.len().saturating_sub(1) {
                            app.terminal_scroll += 1;
                        }
                    } else if mouse_event.column >= assistant_area.x
                        && mouse_event.column < assistant_area.x + assistant_area.width
                    {
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

    Ok(None)
}
