use anyhow::{Result, bail};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::fmt;

/// A single key combination (e.g., Ctrl+S, Alt+Left, or just 'g')
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chord(pub KeyEvent);

/// A sequence of chords (e.g., "Ctrl+K Ctrl+C" or "g g")
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keybind(pub Vec<Chord>);

impl Chord {
    /// Parse a single chord from a string like "Ctrl+S" or "Alt+Left" or "g"
    pub fn parse(s: &str) -> Result<Self> {
        let s = s.trim();
        if s.is_empty() {
            bail!("Empty chord");
        }

        let parts: Vec<&str> = s.split('+').collect();
        let mut modifiers = KeyModifiers::NONE;
        let mut key_part = None;

        for part in &parts {
            let part_lower = part.to_lowercase();
            match part_lower.as_str() {
                "ctrl" | "control" => modifiers |= KeyModifiers::CONTROL,
                "alt" => modifiers |= KeyModifiers::ALT,
                "shift" => modifiers |= KeyModifiers::SHIFT,
                "super" => modifiers |= KeyModifiers::SUPER,
                "meta" => modifiers |= KeyModifiers::META,
                "hyper" => modifiers |= KeyModifiers::HYPER,
                _ => {
                    if key_part.is_some() {
                        bail!("Multiple non-modifier keys in chord: {}", s);
                    }
                    key_part = Some(*part);
                }
            }
        }

        let key_str = key_part.ok_or_else(|| anyhow::anyhow!("No key in chord: {}", s))?;
        let code = parse_key_code(key_str)?;

        Ok(Chord(KeyEvent::new(code, modifiers)))
    }

    /// Check if this chord matches a key event
    /// Handles case-insensitivity for character keys with modifiers
    pub fn matches(&self, event: &KeyEvent) -> bool {
        // Modifiers must match
        if self.0.modifiers != event.modifiers {
            return false;
        }

        // For character keys, compare case-insensitively when modifiers are present
        // (crossterm reports Ctrl+S as Char('s') with CONTROL, not Char('S'))
        match (&self.0.code, &event.code) {
            (KeyCode::Char(expected), KeyCode::Char(actual)) => {
                expected.eq_ignore_ascii_case(actual)
            }
            (a, b) => a == b,
        }
    }
}

impl fmt::Display for Chord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts = Vec::new();

        if self.0.modifiers.contains(KeyModifiers::CONTROL) {
            parts.push("Ctrl");
        }
        if self.0.modifiers.contains(KeyModifiers::ALT) {
            parts.push("Alt");
        }
        if self.0.modifiers.contains(KeyModifiers::SHIFT) {
            parts.push("Shift");
        }
        if self.0.modifiers.contains(KeyModifiers::SUPER) {
            parts.push("Super");
        }
        if self.0.modifiers.contains(KeyModifiers::META) {
            parts.push("Meta");
        }
        if self.0.modifiers.contains(KeyModifiers::HYPER) {
            parts.push("Hyper");
        }

        let key_str = format_key_code(&self.0.code);
        parts.push(&key_str);

        let result = parts.join("+");
        write!(f, "{}", result)
    }
}

impl Keybind {
    /// Parse a keybind string with space-separated chords
    /// e.g., "Ctrl+K Ctrl+C" or "g g"
    pub fn parse(s: &str) -> Result<Self> {
        let s = s.trim();
        if s.is_empty() {
            bail!("Empty keybind");
        }

        let chords: Result<Vec<Chord>> = s.split_whitespace().map(Chord::parse).collect();

        Ok(Keybind(chords?))
    }

    /// Number of chords in this keybind
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

impl fmt::Display for Keybind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let parts: Vec<String> = self.0.iter().map(|c| c.to_string()).collect();
        write!(f, "{}", parts.join(" "))
    }
}

/// Parse a key code string to a KeyCode
fn parse_key_code(s: &str) -> Result<KeyCode> {
    // Single character
    if s.chars().count() == 1 {
        let c = s.chars().next().unwrap();
        return Ok(KeyCode::Char(c));
    }

    // Named keys (case-insensitive)
    let lower = s.to_lowercase();
    let code = match lower.as_str() {
        "backspace" | "back" => KeyCode::Backspace,
        "enter" | "return" => KeyCode::Enter,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "pageup" | "pgup" => KeyCode::PageUp,
        "pagedown" | "pgdn" | "pgdown" => KeyCode::PageDown,
        "tab" => KeyCode::Tab,
        "backtab" => KeyCode::BackTab,
        "delete" | "del" => KeyCode::Delete,
        "insert" | "ins" => KeyCode::Insert,
        "esc" | "escape" => KeyCode::Esc,
        "space" => KeyCode::Char(' '),
        "capslock" => KeyCode::CapsLock,
        "scrolllock" => KeyCode::ScrollLock,
        "numlock" => KeyCode::NumLock,
        "printscreen" | "print" => KeyCode::PrintScreen,
        "pause" => KeyCode::Pause,
        "menu" => KeyCode::Menu,
        // Function keys
        s if s.starts_with('f') => {
            let num: u8 = s[1..].parse().map_err(|_| anyhow::anyhow!("Invalid function key: {}", s))?;
            KeyCode::F(num)
        }
        _ => bail!("Unknown key: {}", s),
    };

    Ok(code)
}

/// Format a KeyCode to a display string
fn format_key_code(code: &KeyCode) -> String {
    match code {
        KeyCode::Backspace => "Backspace".to_string(),
        KeyCode::Enter => "Enter".to_string(),
        KeyCode::Left => "Left".to_string(),
        KeyCode::Right => "Right".to_string(),
        KeyCode::Up => "Up".to_string(),
        KeyCode::Down => "Down".to_string(),
        KeyCode::Home => "Home".to_string(),
        KeyCode::End => "End".to_string(),
        KeyCode::PageUp => "PageUp".to_string(),
        KeyCode::PageDown => "PageDown".to_string(),
        KeyCode::Tab => "Tab".to_string(),
        KeyCode::BackTab => "BackTab".to_string(),
        KeyCode::Delete => "Delete".to_string(),
        KeyCode::Insert => "Insert".to_string(),
        KeyCode::Esc => "Esc".to_string(),
        KeyCode::CapsLock => "CapsLock".to_string(),
        KeyCode::ScrollLock => "ScrollLock".to_string(),
        KeyCode::NumLock => "NumLock".to_string(),
        KeyCode::PrintScreen => "PrintScreen".to_string(),
        KeyCode::Pause => "Pause".to_string(),
        KeyCode::Menu => "Menu".to_string(),
        KeyCode::F(n) => format!("F{}", n),
        KeyCode::Char(' ') => "Space".to_string(),
        KeyCode::Char(c) => c.to_string(),
        KeyCode::Null => "Null".to_string(),
        KeyCode::KeypadBegin => "KeypadBegin".to_string(),
        KeyCode::Media(m) => format!("Media({:?})", m),
        KeyCode::Modifier(m) => format!("Modifier({:?})", m),
    }
}

/// Convert a KeyEvent to a Chord (for display purposes)
pub fn key_event_to_chord(event: &KeyEvent) -> Chord {
    Chord(KeyEvent::new(event.code, event.modifiers))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_char() {
        let chord = Chord::parse("g").unwrap();
        assert_eq!(chord.0.code, KeyCode::Char('g'));
        assert_eq!(chord.0.modifiers, KeyModifiers::NONE);
    }

    #[test]
    fn test_parse_ctrl_char() {
        let chord = Chord::parse("Ctrl+S").unwrap();
        assert_eq!(chord.0.code, KeyCode::Char('S'));
        assert_eq!(chord.0.modifiers, KeyModifiers::CONTROL);
    }

    #[test]
    fn test_parse_ctrl_shift() {
        let chord = Chord::parse("Ctrl+Shift+K").unwrap();
        assert_eq!(chord.0.code, KeyCode::Char('K'));
        assert_eq!(
            chord.0.modifiers,
            KeyModifiers::CONTROL | KeyModifiers::SHIFT
        );
    }

    #[test]
    fn test_parse_alt_arrow() {
        let chord = Chord::parse("Alt+Left").unwrap();
        assert_eq!(chord.0.code, KeyCode::Left);
        assert_eq!(chord.0.modifiers, KeyModifiers::ALT);
    }

    #[test]
    fn test_parse_function_key() {
        let chord = Chord::parse("F12").unwrap();
        assert_eq!(chord.0.code, KeyCode::F(12));
        assert_eq!(chord.0.modifiers, KeyModifiers::NONE);
    }

    #[test]
    fn test_parse_keybind_single() {
        let kb = Keybind::parse("Ctrl+S").unwrap();
        assert_eq!(kb.len(), 1);
    }

    #[test]
    fn test_parse_keybind_multi() {
        let kb = Keybind::parse("Ctrl+K Ctrl+C").unwrap();
        assert_eq!(kb.len(), 2);
        assert_eq!(kb.0[0].0.code, KeyCode::Char('K'));
        assert_eq!(kb.0[1].0.code, KeyCode::Char('C'));
    }

    #[test]
    fn test_parse_keybind_vim_gg() {
        let kb = Keybind::parse("g g").unwrap();
        assert_eq!(kb.len(), 2);
        assert_eq!(kb.0[0].0.code, KeyCode::Char('g'));
        assert_eq!(kb.0[1].0.code, KeyCode::Char('g'));
    }

    #[test]
    fn test_chord_display() {
        let chord = Chord::parse("Ctrl+Shift+K").unwrap();
        assert_eq!(chord.to_string(), "Ctrl+Shift+K");
    }

    #[test]
    fn test_keybind_display() {
        let kb = Keybind::parse("Ctrl+K Ctrl+C").unwrap();
        assert_eq!(kb.to_string(), "Ctrl+K Ctrl+C");
    }

    #[test]
    fn test_chord_matches() {
        let chord = Chord::parse("Ctrl+S").unwrap();

        // Uppercase matches
        let event = KeyEvent::new(KeyCode::Char('S'), KeyModifiers::CONTROL);
        assert!(chord.matches(&event));

        // Lowercase also matches (crossterm reports Ctrl+S as lowercase)
        let lowercase_event = KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL);
        assert!(chord.matches(&lowercase_event));

        // Different key doesn't match
        let wrong_key = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL);
        assert!(!chord.matches(&wrong_key));

        // Different modifiers don't match
        let wrong_mods = KeyEvent::new(KeyCode::Char('s'), KeyModifiers::ALT);
        assert!(!chord.matches(&wrong_mods));
    }
}
