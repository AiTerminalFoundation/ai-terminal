use iced::Subscription;
use iced::event::{Event, Status};
use iced::keyboard::Event as KeyEvent;
use std::time::Duration;

use crate::ui::messages::Message;
use crate::config::keyboard::{handle_keyboard_event, ShortcutAction};

#[derive(Debug, Clone)]
pub enum State {
    Ready,
    Waiting,
}

pub struct EventHandler;

impl EventHandler {
    pub fn handle(event: Event, status: Status) -> Option<Message> {
        // Log status when we receive keyboard events
        if let Event::Keyboard(key_event) = &event {
            println!("[subscriptions.rs:handle] Keyboard event: {:?}, status: {:?}", key_event, status);
            
            // Special handling for character events
            if let KeyEvent::CharacterReceived(ch) = key_event {
                println!("[subscriptions.rs:handle] Character received: '{}'", ch);
            }
        }
        
        if let Event::Keyboard(key_event) = event {
            let action = handle_keyboard_event(key_event);
            match action {
                ShortcutAction::ToggleFocus => Some(Message::ToggleFocus),
                ShortcutAction::ResizeLeft => Some(Message::ResizeLeft),
                ShortcutAction::ResizeRight => Some(Message::ResizeRight),
                ShortcutAction::HistoryUp => Some(Message::HistoryUp),
                ShortcutAction::HistoryDown => Some(Message::HistoryDown),
                ShortcutAction::TildeInsert => Some(Message::TildePressed),
                ShortcutAction::TerminateCommand => Some(Message::HandleCtrlC),
                ShortcutAction::ToggleSearch => Some(Message::ToggleSearch),
                ShortcutAction::ToggleTerminalSearchFocus => Some(Message::ToggleTerminalSearchFocus),
                ShortcutAction::TabAutocomplete => Some(Message::TabPressed),
                ShortcutAction::ExecuteCommand => Some(Message::ExecuteCommand),
                ShortcutAction::None => None,
            }
        } else {
            None
        }
    }
}

pub fn create_subscriptions(has_command_running: bool) -> Subscription<Message> {
    let keyboard_events = iced::subscription::events_with(EventHandler::handle);

    // Terminal polling subscription
    let terminal_poll = if has_command_running {
        iced::subscription::unfold(
            "terminal_poll",
            State::Ready,
            move |state| async move {
                match state {
                    State::Ready => {
                        // Use 0ms wait time for maximum responsiveness
                        tokio::time::sleep(Duration::from_millis(0)).await;
                        (Message::PollCommandOutput, State::Waiting)
                    }
                    State::Waiting => {
                        // Use 0ms wait time for maximum responsiveness
                        tokio::time::sleep(Duration::from_millis(0)).await;
                        (Message::PollCommandOutput, State::Waiting)
                    }
                }
            },
        )
    } else {
        // Even with no active command, poll regularly but less aggressively
        iced::subscription::unfold("inactive_poll", State::Ready, |state| async move {
            match state {
                State::Ready => {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    (Message::PollCommandOutput, State::Waiting)
                }
                State::Waiting => {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    (Message::PollCommandOutput, State::Waiting)
                }
            }
        })
    };

    // UI refresh subscription
    let ui_refresh = if has_command_running {
        iced::subscription::unfold(
            "ui_refresh",
            State::Ready,
            move |state| async move {
                match state {
                    State::Ready => {
                        // Use extremely short delay for maximum UI responsiveness
                        tokio::time::sleep(Duration::from_millis(16)).await; // ~60fps refresh rate
                        (Message::CheckCommandOutput, State::Waiting)
                    }
                    State::Waiting => {
                        tokio::time::sleep(Duration::from_millis(16)).await;
                        (Message::CheckCommandOutput, State::Waiting)
                    }
                }
            },
        )
    } else {
        // No-op subscription when no command is running
        iced::subscription::unfold("inactive_ui_refresh", State::Ready, |state| async move {
            match state {
                State::Ready => {
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    (Message::NoOp, State::Waiting)
                }
                State::Waiting => {
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    (Message::NoOp, State::Waiting)
                }
            }
        })
    };

    Subscription::batch(vec![
        keyboard_events,
        terminal_poll,
        ui_refresh,
    ])
} 