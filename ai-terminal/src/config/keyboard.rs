use iced::keyboard::{KeyCode, Modifiers, Event as KeyEvent};

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum FocusTarget {
    Terminal,
    AiChat,
}

pub fn handle_keyboard_shortcuts(key_event: KeyEvent, current_focus: &mut FocusTarget) -> bool {
    match key_event {
        KeyEvent::KeyPressed { 
            key_code: KeyCode::E,
            modifiers,
            ..
        } if modifiers.control() => {
            *current_focus = match *current_focus {
                FocusTarget::Terminal => FocusTarget::AiChat,
                FocusTarget::AiChat => FocusTarget::Terminal,
            };
            true
        }
        _ => false,
    }
} 