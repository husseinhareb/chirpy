// src/ui/keybindings.rs
//! Keyboard input handling and key mappings.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Map digit/shifted-digit keys to section number (1..4).
pub fn map_key_to_digit(k: &KeyEvent) -> Option<usize> {
    if let KeyCode::Char(c) = k.code {
        match c {
            '1' | '!' => Some(1),
            '2' | '@' => Some(2),
            '3' | '#' => Some(3),
            '4' | '$' => Some(4),
            _ => None,
        }
    } else {
        None
    }
}

/// Check if the key event is a shifted symbol (!, @, #, $).
pub fn is_shifted_symbol(key: &KeyEvent) -> bool {
    matches!(
        key.code,
        KeyCode::Char('!') | KeyCode::Char('@') | KeyCode::Char('#') | KeyCode::Char('$')
    )
}

/// Check if this is a section toggle key press (Shift+number).
pub fn is_section_toggle(key: &KeyEvent) -> bool {
    if let Some(_) = map_key_to_digit(key) {
        key.modifiers.contains(KeyModifiers::SHIFT) || is_shifted_symbol(key)
    } else {
        false
    }
}

/// Navigation actions derived from key events.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NavigationAction {
    Up,
    Down,
    Enter,
    Back,
    TogglePause,
    Quit,
    ToggleSection(usize),
    None,
}

/// Convert a key event to a navigation action.
pub fn key_to_action(key: &KeyEvent) -> NavigationAction {
    // Check for section toggle first
    if let Some(d) = map_key_to_digit(key) {
        if key.modifiers.contains(KeyModifiers::SHIFT) || is_shifted_symbol(key) {
            return NavigationAction::ToggleSection(d);
        }
    }

    match key.code {
        KeyCode::Down => NavigationAction::Down,
        KeyCode::Up => NavigationAction::Up,
        KeyCode::Enter | KeyCode::Right => NavigationAction::Enter,
        KeyCode::Left => NavigationAction::Back,
        KeyCode::Char(' ') => NavigationAction::TogglePause,
        KeyCode::Char('q') => NavigationAction::Quit,
        _ => NavigationAction::None,
    }
}
