use fltk::app as fltk_app;

mod config;
mod model;
mod ollama;
mod terminal;
mod ui;

use ui::app_ui::AppUI;

fn main() {
    // Create the FLTK application
    let app = fltk_app::App::default();
    
    // Create the UI
    let mut ui = AppUI::new();
    
    // Setup event handlers
    ui.setup_events();
    
    // Run the application
    app.run().unwrap();
}
