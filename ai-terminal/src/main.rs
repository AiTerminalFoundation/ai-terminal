mod app;
mod config;
mod model;
mod ollama;
mod terminal;
mod ui;

use iced::{Settings, Application};
use app::TerminalApp;

fn main() -> iced::Result {
    TerminalApp::run(Settings::default())
}
