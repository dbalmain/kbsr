use crate::matcher::MatchState;
use crate::storage::DeckStats;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

const DECK_SELECTION_HINTS: &[(&[&str], &str)] = &[
    (&["↑", "↓"], "move"),
    (&["Enter"], "study"),
    (&["q", "Esc"], "quit"),
    (&["?"], "toggle hints"),
];

const STUDY_HINTS: &[(&[&str], &str)] = &[(&["Esc"], "reveal")];

const SUMMARY_HINTS: &[(&[&str], &str)] = &[(&["any key"], "continue")];

/// UI state for rendering
pub struct UiState<'a> {
    /// The deck name
    pub deck: &'a str,
    /// The clue/description to display
    pub clue: &'a str,
    /// Current match state (typed chords and success/fail)
    pub match_state: &'a MatchState,
    /// Whether we're showing the answer
    pub showing_answer: bool,
    /// The correct answer (for showing after reveal)
    pub answer: &'a str,
    /// Message to display (e.g., "Type the answer to continue")
    pub message: Option<&'a str>,
    /// Whether to show the success checkmark
    pub show_success_checkmark: bool,
    /// Whether the card was cleared (rated Easy, won't be re-queued)
    pub card_cleared: bool,
    /// Whether to show key hints
    pub show_hints: bool,
    /// Configured pause keybind string
    pub pause_keybind: &'a str,
    /// Configured quit keybind string
    pub quit_keybind: &'a str,
    /// Number of cards remaining in the session
    pub cards_remaining: usize,
}

/// Render the minimal UI
pub fn render(frame: &mut Frame, state: &UiState) {
    let area = frame.area();

    let chunks = Layout::vertical([
        Constraint::Fill(1),   // Top spacer
        Constraint::Length(1), // Deck name
        Constraint::Length(3), // Clue area
        Constraint::Length(2), // Typed keys area
        Constraint::Length(1), // Spacer before answer/message
        Constraint::Length(1), // Answer or checkmark area
        Constraint::Length(1), // Message area
        Constraint::Fill(1),   // Bottom spacer
    ])
    .split(area);

    // Render deck name (dimmed, centered)
    let deck = Paragraph::new(state.deck)
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    frame.render_widget(deck, chunks[1]);

    // Render clue (centered)
    let clue = Paragraph::new(state.clue)
        .style(Style::default().fg(Color::White))
        .alignment(Alignment::Center);
    frame.render_widget(clue, chunks[2]);

    // Render typed keys with appropriate color
    let typed_line = render_typed_chords(state.match_state);
    let typed = Paragraph::new(typed_line).alignment(Alignment::Center);
    frame.render_widget(typed, chunks[3]);

    // chunks[4] is spacer

    // Render answer if showing (below typed keys)
    if state.showing_answer {
        let answer_line = Line::from(vec![
            Span::styled("Answer: ", Style::default().fg(Color::DarkGray)),
            Span::styled(state.answer, Style::default().fg(Color::White)),
        ]);
        let answer = Paragraph::new(answer_line).alignment(Alignment::Center);
        frame.render_widget(answer, chunks[5]);
    }

    // Render message or checkmark
    if let Some(msg) = state.message {
        let message = Paragraph::new(msg)
            .style(Style::default().fg(Color::Yellow))
            .alignment(Alignment::Center);
        frame.render_widget(message, chunks[6]);
    } else if state.show_success_checkmark {
        let style = if state.card_cleared {
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        let checkmark =
            Paragraph::new(Line::from(Span::styled("✓", style))).alignment(Alignment::Center);
        frame.render_widget(checkmark, chunks[5]);
    }

    if state.show_hints {
        render_study_hints(
            frame,
            area,
            state.pause_keybind,
            state.quit_keybind,
            state.cards_remaining,
        );
    }
}

/// Render the typed chords with appropriate coloring
fn render_typed_chords(state: &MatchState) -> Line<'static> {
    let chords = state.typed_chords();

    if chords.is_empty() {
        return Line::from("");
    }

    let text: String = chords
        .iter()
        .map(|c| c.to_string())
        .collect::<Vec<_>>()
        .join(" ");

    let color = match state {
        MatchState::InProgress(_) => Color::Green,
        MatchState::Complete(_) => Color::Green,
        MatchState::Failed(_) => Color::Red,
    };

    // Always show red/green feedback so user knows if they're typing correctly,
    // even when the answer is revealed (they might be touch-typing)
    let style = Style::default().fg(color);

    Line::from(Span::styled(text, style))
}

/// Render deck selection screen
pub fn render_deck_selection(
    frame: &mut Frame,
    decks: &[DeckStats],
    selected: usize,
    show_hints: bool,
) {
    let area = frame.area();

    let chunks = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(2),
        Constraint::Length((decks.len() + 1) as u16),
        Constraint::Fill(1),
    ])
    .split(area);

    // Title
    let title = Paragraph::new("Select a deck")
        .style(Style::default().fg(Color::White))
        .alignment(Alignment::Center);
    frame.render_widget(title, chunks[1]);

    // Deck list
    let mut lines: Vec<Line> = Vec::new();

    for (i, deck) in decks.iter().enumerate() {
        let prefix = if i == selected { "> " } else { "  " };
        let style = if i == selected {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::White)
        };

        let line = Line::from(Span::styled(
            format!(
                "{}{} ({} due / {} total)",
                prefix, deck.name, deck.due_cards, deck.total_cards
            ),
            style,
        ));
        lines.push(line);
    }

    let list = Paragraph::new(lines).alignment(Alignment::Center);
    frame.render_widget(list, chunks[2]);

    if show_hints {
        render_hints_bar(frame, area, DECK_SELECTION_HINTS);
    }
}

/// Render paused screen
pub fn render_paused(frame: &mut Frame, resume_keybind: &str) {
    let area = frame.area();

    let chunks = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(4),
        Constraint::Fill(1),
    ])
    .split(area);

    let lines = vec![
        Line::from(Span::styled("PAUSED", Style::default().fg(Color::Yellow))),
        Line::from(""),
        Line::from(Span::styled(
            format!("Press {} to resume", resume_keybind),
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let paused = Paragraph::new(lines).alignment(Alignment::Center);
    frame.render_widget(paused, chunks[1]);
}

/// Render session summary
pub fn render_summary(
    frame: &mut Frame,
    reviewed: usize,
    correct: usize,
    total_time_secs: u64,
    show_hints: bool,
) {
    let area = frame.area();

    let chunks = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(5),
        Constraint::Fill(1),
    ])
    .split(area);

    let accuracy = if reviewed > 0 {
        (correct as f64 / reviewed as f64) * 100.0
    } else {
        0.0
    };

    let lines = vec![
        Line::from(Span::styled(
            "Session Complete",
            Style::default().fg(Color::Green),
        )),
        Line::from(""),
        Line::from(format!("Cards reviewed: {}", reviewed)),
        Line::from(format!("Correct: {} ({:.0}%)", correct, accuracy)),
        Line::from(format!("Time: {}s", total_time_secs)),
    ];

    let summary = Paragraph::new(lines).alignment(Alignment::Center);
    frame.render_widget(summary, chunks[1]);

    if show_hints {
        render_hints_bar(frame, area, SUMMARY_HINTS);
    }
}

fn render_study_hints(
    frame: &mut Frame,
    area: Rect,
    pause_keybind: &str,
    quit_keybind: &str,
    cards_remaining: usize,
) {
    let bar_area = Rect {
        x: area.x,
        y: area.y + area.height.saturating_sub(2),
        width: area.width,
        height: 1,
    };

    let key_style = Style::default().fg(Color::Cyan);
    let desc_style = Style::default().fg(Color::DarkGray);
    let progress_style = Style::default()
        .fg(Color::White)
        .add_modifier(Modifier::DIM);

    let mut spans = Vec::new();

    for (keys, description) in STUDY_HINTS {
        for key in *keys {
            spans.push(Span::styled(*key, key_style));
        }
        spans.push(Span::raw(" "));
        spans.push(Span::styled(*description, desc_style));
        spans.push(Span::raw("  "));
    }

    if !pause_keybind.is_empty() {
        spans.push(Span::styled(pause_keybind, key_style));
        spans.push(Span::raw(" "));
        spans.push(Span::styled("pause", desc_style));
        spans.push(Span::raw("  "));
    }
    if !quit_keybind.is_empty() {
        spans.push(Span::styled(quit_keybind, key_style));
        spans.push(Span::raw(" "));
        spans.push(Span::styled("quit", desc_style));
        spans.push(Span::raw("  "));
    }

    spans.push(Span::styled(
        format!("•  {}", cards_remaining),
        progress_style,
    ));

    let paragraph = Paragraph::new(Line::from(spans)).alignment(Alignment::Center);
    frame.render_widget(paragraph, bar_area);
}

fn render_hints_bar(frame: &mut Frame, area: Rect, hints: &[(&[&str], &str)]) {
    if hints.is_empty() {
        return;
    }

    let bar_area = Rect {
        x: area.x,
        y: area.y + area.height.saturating_sub(2),
        width: area.width,
        height: 1,
    };

    let key_style = Style::default().fg(Color::Cyan);
    let desc_style = Style::default().fg(Color::DarkGray);
    let sep_style = desc_style.add_modifier(Modifier::DIM);

    let mut spans = Vec::new();
    for (keys, description) in hints {
        for (i, key) in keys.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled("/", sep_style));
            }
            spans.push(Span::styled(*key, key_style));
        }
        spans.push(Span::raw(" "));
        spans.push(Span::styled(*description, desc_style));
        spans.push(Span::raw("  "));
    }

    let paragraph = Paragraph::new(Line::from(spans)).alignment(Alignment::Center);
    frame.render_widget(paragraph, bar_area);
}
