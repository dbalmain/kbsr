use crate::keybind::{Chord, Keybind, key_event_to_chord};
use crossterm::event::KeyEvent;

/// State of the input matching
#[derive(Debug, Clone)]
pub enum MatchState {
    /// Currently matching, with successfully typed chords
    InProgress(Vec<Chord>),
    /// Successfully matched the entire keybind
    Complete(Vec<Chord>),
    /// Failed to match, with all typed chords (to display in red)
    Failed(Vec<Chord>),
}

impl MatchState {
    /// Get the typed chords for display
    pub fn typed_chords(&self) -> &[Chord] {
        match self {
            MatchState::InProgress(chords) => chords,
            MatchState::Complete(chords) => chords,
            MatchState::Failed(chords) => chords,
        }
    }

    #[cfg(test)]
    pub fn is_failed(&self) -> bool {
        matches!(self, MatchState::Failed(_))
    }

    #[cfg(test)]
    pub fn is_complete(&self) -> bool {
        matches!(self, MatchState::Complete(_))
    }
}

/// Matcher for tracking input against expected keybind
pub struct Matcher {
    expected: Keybind,
    typed: Vec<Chord>,
    failed: bool,
}

impl Matcher {
    /// Create a new matcher for the given keybind
    pub fn new(expected: Keybind) -> Self {
        Self {
            expected,
            typed: Vec::new(),
            failed: false,
        }
    }

    /// Process a key event and return the new state
    pub fn process(&mut self, event: KeyEvent) -> MatchState {
        let chord = key_event_to_chord(&event);

        // If already failed, check if this is the start of a retry
        if self.failed {
            // User is starting over
            self.typed.clear();
            self.failed = false;
        }

        // Add the typed chord
        self.typed.push(chord.clone());

        // Check if it matches the expected chord at this position
        let position = self.typed.len() - 1;
        if position >= self.expected.len() {
            self.failed = true;
            return MatchState::Failed(self.typed.clone());
        }
        let expected_chord = &self.expected.0[position];

        if !expected_chord.matches(&event) {
            // Wrong chord - fail
            self.failed = true;
            return MatchState::Failed(self.typed.clone());
        }

        // Correct chord - check if complete
        if self.typed.len() == self.expected.len() {
            return MatchState::Complete(self.typed.clone());
        }

        // Still in progress
        MatchState::InProgress(self.typed.clone())
    }

    /// Reset the matcher (for retry after failure)
    pub fn reset(&mut self) {
        self.typed.clear();
        self.failed = false;
    }

    /// Get current state without processing
    pub fn state(&self) -> MatchState {
        if self.failed {
            MatchState::Failed(self.typed.clone())
        } else if self.typed.len() == self.expected.len() && !self.typed.is_empty() {
            MatchState::Complete(self.typed.clone())
        } else {
            MatchState::InProgress(self.typed.clone())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyModifiers};

    fn make_event(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    #[test]
    fn test_single_chord_match() {
        let kb = Keybind::parse("Ctrl+S").unwrap();
        let mut matcher = Matcher::new(kb);

        let state = matcher.process(make_event(KeyCode::Char('S'), KeyModifiers::CONTROL));
        assert!(state.is_complete());
    }

    #[test]
    fn test_single_chord_fail() {
        let kb = Keybind::parse("Ctrl+S").unwrap();
        let mut matcher = Matcher::new(kb);

        let state = matcher.process(make_event(KeyCode::Char('X'), KeyModifiers::CONTROL));
        assert!(state.is_failed());
    }

    #[test]
    fn test_multi_chord_progress() {
        let kb = Keybind::parse("g g").unwrap();
        let mut matcher = Matcher::new(kb);

        let state = matcher.process(make_event(KeyCode::Char('g'), KeyModifiers::NONE));
        assert!(matches!(state, MatchState::InProgress(_)));

        let state = matcher.process(make_event(KeyCode::Char('g'), KeyModifiers::NONE));
        assert!(state.is_complete());
    }

    #[test]
    fn test_multi_chord_fail_mid() {
        let kb = Keybind::parse("Ctrl+K Ctrl+C").unwrap();
        let mut matcher = Matcher::new(kb);

        let state = matcher.process(make_event(KeyCode::Char('K'), KeyModifiers::CONTROL));
        assert!(matches!(state, MatchState::InProgress(_)));

        let state = matcher.process(make_event(KeyCode::Char('X'), KeyModifiers::CONTROL));
        assert!(state.is_failed());
        assert_eq!(state.typed_chords().len(), 2);
    }

    #[test]
    fn test_reset_after_fail() {
        let kb = Keybind::parse("g g").unwrap();
        let mut matcher = Matcher::new(kb);

        // Fail first
        let _ = matcher.process(make_event(KeyCode::Char('x'), KeyModifiers::NONE));
        assert!(matcher.state().is_failed());

        // Reset and try again
        matcher.reset();
        let state = matcher.process(make_event(KeyCode::Char('g'), KeyModifiers::NONE));
        assert!(matches!(state, MatchState::InProgress(_)));
    }
}
