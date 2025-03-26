use iced::Element;
use crate::model::App as AppState;
use crate::config::keyboard::FocusTarget;
use crate::ui::components::{TerminalPanel, AiPanel};
use crate::ui::messages::Message;

// Panel data container
pub struct PanelViews<'a> {
    pub terminal: Element<'a, Message>,
    pub ai: Element<'a, Message>,
}

// Terminal panel state management
pub struct TerminalPanelState {
    pub panel: TerminalPanel,
    pub input: String,
    pub focus: bool,
    pub search_mode: bool,
    pub search_input: String, 
    pub search_index: usize,
    pub search_matches: Vec<usize>,
}

impl TerminalPanelState {
    pub fn new(app_state: AppState, input: String, focus_target: FocusTarget, search_mode: bool) -> Self {
        Self {
            panel: TerminalPanel::new(
                app_state.clone(),
                input.clone(),
                focus_target.clone(),
                search_mode
            ),
            input,
            focus: true,
            search_mode,
            search_input: String::new(),
            search_index: 0,
            search_matches: Vec::new(),
        }
    }
    
    pub fn recreate(&mut self, app_state: AppState, input: String, focus_target: FocusTarget) {
        self.input = input.clone();
        self.panel = TerminalPanel::new(
            app_state.clone(),
            input.clone(),
            focus_target,
            self.search_mode
        );
        self.panel.set_terminal_focus(self.focus);
        
        if self.search_mode {
            self.panel.update_search_input(self.search_input.clone());
            self.panel.update_search_count(self.search_index, self.search_matches.len());
        }
    }
    
    // Delegating methods to the panel
    pub fn set_terminal_focus(&mut self, focus: bool) {
        self.focus = focus;
        self.panel.set_terminal_focus(focus);
    }

    pub fn update_search_count(&mut self, index: usize, total: usize) {
        self.panel.update_search_count(index, total);
    }
    
    pub fn view(&self) -> Element<'_, Message> {
        self.panel.view()
    }
}

// AI panel state management 
pub struct AiPanelState {
    pub panel: AiPanel,
    pub input: String,
}

impl AiPanelState {
    pub fn new(app_state: AppState, input: String, focus_target: FocusTarget) -> Self {
        Self {
            panel: AiPanel::new(
                app_state.clone(),
                input.clone(),
                focus_target
            ),
            input,
        }
    }
    
    pub fn recreate(&mut self, app_state: AppState, focus_target: FocusTarget) {
        self.panel = AiPanel::new(
            app_state.clone(),
            self.input.clone(),
            focus_target
        );
    }
    
    pub fn view(&self) -> Element<'_, Message> {
        self.panel.view()
    }

    pub fn update_input(&mut self, input: String) {
        self.input = input.clone();
        self.panel.update_input(input);
    }
} 