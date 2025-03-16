use ratatui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
    Frame,
};

use crate::config::{SEPARATOR_LINE, TERMINAL_TITLE, ASSISTANT_TITLE, INPUT_TITLE, SUGGESTIONS_TITLE, MAX_VISIBLE_SUGGESTIONS};
use crate::model::{App, CommandStatus, Panel};

pub fn draw_ui<B: Backend>(f: &mut Frame, app: &mut App) {
    let size = f.area();

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

    // Draw terminal panel
    draw_terminal_panel::<B>(f, app, main_chunks[0]);

    // Draw AI assistant panel
    draw_assistant_panel::<B>(f, app, main_chunks[1]);
}

fn draw_terminal_panel<B: Backend>(f: &mut Frame, app: &mut App, area: Rect) {
    // Terminal panel (left side)
    let terminal_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(3)].as_ref())
        .split(area);

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
                } else if line.starts_with(SEPARATOR_LINE) {
                    // This is a separator line, style it appropriately
                    lines.push(Line::from(vec![
                        Span::styled(
                            SEPARATOR_LINE.repeat(terminal_chunks[0].width as usize - 2),
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
                    .title(TERMINAL_TITLE),
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
                    .title(TERMINAL_TITLE),
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
        draw_autocomplete_suggestions::<B>(f, app, terminal_chunks[1], f.area());
    }

    // Set cursor position for terminal input
    if app.active_panel == Panel::Terminal {
        f.set_cursor_position(
            (terminal_chunks[1].x + app.cursor_position as u16 + 1,
            terminal_chunks[1].y + 1)
        );
    }
}

fn draw_assistant_panel<B: Backend>(f: &mut Frame, app: &mut App, area: Rect) {
    // AI Assistant panel (right side)
    let assistant_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(3)].as_ref())
        .split(area);

    // AI output area
    let ai_output_text = Text::from(
        app.ai_output
            .iter()
            .enumerate()
            .flat_map(|(_i, line)| {
                let mut lines = Vec::new();
                
                // Now add the line itself
                if line.starts_with("> ") {
                    // Add the user message with a distinct color
                    lines.push(Line::from(vec![
                        Span::styled(line.clone(), Style::default().fg(Color::Cyan))
                    ]));
                } else if line.starts_with(SEPARATOR_LINE) {
                    // This is a separator line, style it appropriately
                    lines.push(Line::from(vec![
                        Span::styled(
                            SEPARATOR_LINE.repeat(assistant_chunks[0].width as usize - 2),
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
    
    // Create the AI assistant title
    let ai_title = if app.ollama_thinking {
        format!("{} [{}] (Thinking...)", ASSISTANT_TITLE, app.ollama_model)
    } else {
        format!("{} [{}]", ASSISTANT_TITLE, app.ollama_model)
    };
    
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
                    .title(ai_title),
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
                    .title(ai_title),
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
                .title(INPUT_TITLE),
        );

    f.render_widget(ai_input, assistant_chunks[1]);

    // Set cursor position for assistant input
    if app.active_panel == Panel::Assistant {
        f.set_cursor_position(
            (assistant_chunks[1].x + app.ai_cursor_position as u16 + 1,
            assistant_chunks[1].y + 1)
        );
    }
}

fn draw_autocomplete_suggestions<B: Backend>(f: &mut Frame, app: &App, input_area: Rect, screen_size: Rect) {
    // Calculate the position for the suggestions popup
    // It should appear below the input area
    let max_suggestions = MAX_VISIBLE_SUGGESTIONS;
    let suggestions_count = app.autocomplete_suggestions.len().min(max_suggestions);
    let suggestions_height = suggestions_count as u16 + 2; // +2 for borders
    
    // Calculate width based on the longest suggestion
    let suggestions_width = app.autocomplete_suggestions
        .iter()
        .take(max_suggestions)
        .map(|s| s.len())
        .max()
        .unwrap_or(20)
        .min(input_area.width.saturating_sub(4) as usize) as u16 + 4; // +4 for padding
    
    let suggestions_x = input_area.x + 1;
    let suggestions_y = input_area.y + 3;
    
    // Make sure the popup doesn't go off-screen
    let suggestions_y = if suggestions_y + suggestions_height > screen_size.height {
        input_area.y.saturating_sub(suggestions_height)
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
                            Style::default().fg(Color::Black).bg(Color::White)
                        )
                    ])
                } else {
                    Line::from(vec![
                        Span::styled(
                            format!(" {} ", display_text),
                            Style::default().fg(Color::White)
                        )
                    ])
                }
            })
            .collect::<Vec<Line>>(),
    );
    
    // Add count indicator if there are more suggestions than shown
    let title = if app.autocomplete_suggestions.len() > max_suggestions {
        format!("{} ({}/{})", SUGGESTIONS_TITLE, max_suggestions, app.autocomplete_suggestions.len())
    } else {
        SUGGESTIONS_TITLE.to_string()
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