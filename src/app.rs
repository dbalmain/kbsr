use crate::config::Config;
use crate::deck::{Deck, list_decks};
use crate::keybind::Keybind;
use crate::matcher::{MatchState, Matcher};
use crate::scheduler::{Rating, Scheduler};
use crate::storage::{Storage, StoredCard};
use crate::ui;
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{DefaultTerminal, Frame};
use std::time::{Duration, Instant};

/// Application state phases
#[derive(Debug, Clone, PartialEq)]
enum Phase {
    DeckSelection,
    Studying,
    ShowingAnswer,
    Summary,
}

/// Study session statistics
struct SessionStats {
    reviewed: usize,
    correct: usize,
    start_time: Instant,
}

/// Main application state
pub struct App {
    config: Config,
    storage: Storage,
    scheduler: Scheduler,
    phase: Phase,
    // Deck selection state
    available_decks: Vec<(String, i32, i32)>,
    selected_deck_idx: usize,
    // Study state
    current_cards: Vec<StudyCard>,
    current_card_idx: usize,
    matcher: Option<Matcher>,
    card_start_time: Instant,
    attempts: u8,
    // Session stats
    stats: SessionStats,
    // Exit flag
    should_exit: bool,
}

/// A card being studied with its storage info
struct StudyCard {
    stored: StoredCard,
    keybind: Keybind,
}

impl App {
    /// Create a new application
    pub fn new(config: Config) -> Result<Self> {
        config.ensure_dirs()?;
        let storage = Storage::open(&config.db_path)?;
        let scheduler = Scheduler::with_default_retention()?;

        Ok(Self {
            config,
            storage,
            scheduler,
            phase: Phase::DeckSelection,
            available_decks: Vec::new(),
            selected_deck_idx: 0,
            current_cards: Vec::new(),
            current_card_idx: 0,
            matcher: None,
            card_start_time: Instant::now(),
            attempts: 0,
            stats: SessionStats {
                reviewed: 0,
                correct: 0,
                start_time: Instant::now(),
            },
            should_exit: false,
        })
    }

    /// Run the application
    pub fn run(mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        // Load deck info
        self.load_deck_info()?;

        // Main event loop
        while !self.should_exit {
            terminal.draw(|frame| self.render(frame))?;
            self.handle_events()?;
        }

        Ok(())
    }

    /// Load deck information from files and database
    fn load_deck_info(&mut self) -> Result<()> {
        let deck_files = list_decks(&self.config.decks_dir)?;

        // Load each deck and sync with database
        for path in deck_files {
            let deck = Deck::load(&path)?;

            // Upsert all cards
            for card in &deck.cards {
                self.storage.upsert_card(
                    &deck.name,
                    &card.keybind.to_string(),
                    &card.description,
                )?;
            }
        }

        // Get deck stats from database
        self.available_decks = self.storage.get_deck_stats()?;

        Ok(())
    }

    /// Start studying the selected deck(s)
    fn start_studying(&mut self) -> Result<()> {
        self.current_cards.clear();
        self.current_card_idx = 0;
        self.stats = SessionStats {
            reviewed: 0,
            correct: 0,
            start_time: Instant::now(),
        };

        if self.selected_deck_idx < self.available_decks.len() {
            // Single deck
            let deck_name = self.available_decks[self.selected_deck_idx].0.clone();
            self.load_due_cards(&deck_name)?;
        } else {
            // All decks (sequential)
            for (deck_name, _, _) in self.available_decks.clone() {
                self.load_due_cards(&deck_name)?;
            }
        }

        if self.current_cards.is_empty() {
            // No cards due
            self.phase = Phase::Summary;
        } else {
            self.phase = Phase::Studying;
            self.setup_current_card();
        }

        Ok(())
    }

    /// Load due cards for a deck
    fn load_due_cards(&mut self, deck_name: &str) -> Result<()> {
        let stored_cards = self.storage.get_due_cards(deck_name)?;

        for stored in stored_cards {
            if let Ok(keybind) = Keybind::parse(&stored.keybind) {
                self.current_cards.push(StudyCard { stored, keybind });
            }
        }

        Ok(())
    }

    /// Set up the current card for study
    fn setup_current_card(&mut self) {
        if self.current_card_idx < self.current_cards.len() {
            let card = &self.current_cards[self.current_card_idx];
            self.matcher = Some(Matcher::new(card.keybind.clone()));
            self.card_start_time = Instant::now();
            self.attempts = 0;
        }
    }

    /// Render the UI
    fn render(&self, frame: &mut Frame) {
        match self.phase {
            Phase::DeckSelection => {
                ui::render_deck_selection(frame, &self.available_decks, self.selected_deck_idx);
            }
            Phase::Studying | Phase::ShowingAnswer => {
                if let Some(card) = self.current_cards.get(self.current_card_idx) {
                    let matcher = self.matcher.as_ref().unwrap();
                    let match_state = matcher.state();

                    let message = if self.phase == Phase::ShowingAnswer {
                        Some("Type the answer to continue")
                    } else if self.card_start_time.elapsed() >= Duration::from_secs(self.config.timeout_secs) {
                        Some("Time's up! Keep trying...")
                    } else {
                        None
                    };

                    let ui_state = ui::UiState {
                        clue: &card.stored.description,
                        match_state: &match_state,
                        showing_answer: self.phase == Phase::ShowingAnswer,
                        answer: &card.keybind.to_string(),
                        message,
                    };
                    ui::render(frame, &ui_state);
                }
            }
            Phase::Summary => {
                ui::render_summary(
                    frame,
                    self.stats.reviewed,
                    self.stats.correct,
                    self.stats.start_time.elapsed().as_secs(),
                );
            }
        }
    }

    /// Handle input events
    fn handle_events(&mut self) -> Result<()> {
        // Poll with timeout for time-based checks
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                // Only handle key press events
                if key.kind != KeyEventKind::Press {
                    return Ok(());
                }

                match self.phase {
                    Phase::DeckSelection => self.handle_deck_selection(key),
                    Phase::Studying => self.handle_studying(key)?,
                    Phase::ShowingAnswer => self.handle_showing_answer(key)?,
                    Phase::Summary => self.handle_summary(key),
                }
            }
        } else {
            // Check for timeout in studying phase
            if self.phase == Phase::Studying {
                self.check_timeout()?;
            }
        }

        Ok(())
    }

    /// Handle deck selection input
    fn handle_deck_selection(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if self.selected_deck_idx > 0 {
                    self.selected_deck_idx -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.selected_deck_idx < self.available_decks.len() {
                    self.selected_deck_idx += 1;
                }
            }
            KeyCode::Enter => {
                let _ = self.start_studying();
            }
            KeyCode::Esc | KeyCode::Char('q') => {
                self.should_exit = true;
            }
            _ => {}
        }
    }

    /// Handle studying input
    fn handle_studying(&mut self, key: KeyEvent) -> Result<()> {
        // Escape reveals the answer
        if key.code == KeyCode::Esc {
            self.reveal_answer()?;
            return Ok(());
        }

        // Quit with Ctrl+C or Ctrl+Q
        if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL)
            && (key.code == KeyCode::Char('c') || key.code == KeyCode::Char('q'))
        {
            self.should_exit = true;
            return Ok(());
        }

        // Process the key
        if let Some(matcher) = &mut self.matcher {
            let state = matcher.process(key);

            match state {
                MatchState::Complete(_) => {
                    // Correct! Score and move to next card
                    self.attempts += 1;
                    self.score_card(true)?;
                    self.next_card()?;
                }
                MatchState::Failed(_) => {
                    // Wrong - increment attempts
                    self.attempts += 1;
                    if self.attempts >= self.config.max_attempts {
                        self.reveal_answer()?;
                    } else {
                        // Reset for retry
                        matcher.reset();
                    }
                }
                MatchState::InProgress(_) => {
                    // Keep going
                }
            }
        }

        Ok(())
    }

    /// Handle showing answer input
    fn handle_showing_answer(&mut self, key: KeyEvent) -> Result<()> {
        // User must type the correct answer to continue
        if let Some(matcher) = &mut self.matcher {
            let state = matcher.process(key);

            if state.is_complete() {
                // They typed it correctly - move on
                self.next_card()?;
            } else if state.is_failed() {
                // Reset and try again
                matcher.reset();
            }
        }

        Ok(())
    }

    /// Handle summary input
    fn handle_summary(&mut self, _key: KeyEvent) {
        self.should_exit = true;
    }

    /// Check for timeout
    fn check_timeout(&mut self) -> Result<()> {
        let elapsed = self.card_start_time.elapsed();
        let timeout = Duration::from_secs(self.config.timeout_secs);

        if elapsed >= timeout && self.attempts == 0 {
            // Auto-mark as failed attempt on first timeout
            self.attempts = 1;
        }

        Ok(())
    }

    /// Reveal the answer
    fn reveal_answer(&mut self) -> Result<()> {
        self.score_card(false)?;
        self.phase = Phase::ShowingAnswer;

        // Reset matcher for typing the answer
        if let Some(card) = self.current_cards.get(self.current_card_idx) {
            self.matcher = Some(Matcher::new(card.keybind.clone()));
        }

        Ok(())
    }

    /// Score the current card
    fn score_card(&mut self, correct: bool) -> Result<()> {
        if let Some(card) = self.current_cards.get(self.current_card_idx) {
            let response_time_ms = self.card_start_time.elapsed().as_millis() as u64;

            let rating = if correct {
                Rating::from_performance(response_time_ms, self.attempts, self.config.timeout_secs)
            } else {
                Rating::Again
            };

            // Get current memory state
            let memory_state = card.stored.stability.and_then(|s| {
                card.stored.difficulty.map(|d| {
                    Scheduler::memory_state_from_stored(s, d)
                })
            });

            // Schedule next review
            let (new_memory, due_date) =
                self.scheduler
                    .schedule(memory_state, card.stored.last_review, rating)?;

            // Update storage
            self.storage.update_card_after_review(
                card.stored.id,
                new_memory.stability,
                new_memory.difficulty,
                due_date,
            )?;

            // Record the review
            self.storage.record_review(
                card.stored.id,
                rating.as_u32() as i32,
                response_time_ms as i64,
                self.attempts as i32,
            )?;

            // Update stats
            self.stats.reviewed += 1;
            if correct {
                self.stats.correct += 1;
            }
        }

        Ok(())
    }

    /// Move to the next card
    fn next_card(&mut self) -> Result<()> {
        self.current_card_idx += 1;

        if self.current_card_idx >= self.current_cards.len() {
            // Done with all cards
            self.phase = Phase::Summary;
        } else {
            self.phase = Phase::Studying;
            self.setup_current_card();
        }

        Ok(())
    }
}
