mod app;
mod config;
mod model;
mod ollama;
mod terminal;
mod ui;

use iced::{Settings, Application, window, Font};
use app::TerminalApp;
use crate::config::constants::{WINDOW_WIDTH, WINDOW_HEIGHT};

fn main() -> iced::Result {
    let window_settings = window::Settings {
        size: (WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32),
        min_size: Some((800, 600)),
        position: window::Position::Centered,
        decorations: true,
        transparent: false,
        resizable: true,
        ..window::Settings::default()
    };

    let settings = Settings {
        window: window_settings,
        antialiasing: true,
        exit_on_close_request: true,
        default_font: Font::DEFAULT,
        default_text_size: 14.0,
        ..Settings::default()
    };
    
    TerminalApp::run(settings)
}
