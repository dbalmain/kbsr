use crate::config::Config;
use crate::deck::{Deck, KeyboardMode, list_decks};
use crate::keybind::{Chord, Keybind};
use crate::matcher::{MatchState, Matcher};
use crate::scheduler::{Rating, Scheduler};
use crate::storage::{DeckStats, Storage, StoredCard};
use crate::ui;
use anyhow::Result;
use crossterm::event::{
    self, Event, KeyCode, KeyEvent, KeyEventKind,
    KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::execute;
use ratatui::{DefaultTerminal, Frame};
use std::collections::HashSet;
use std::io::stdout;
use rand::seq::SliceRandom;
use std::time::{Duration, Instant};

/// Application state phases
#[derive(Debug, Clone, PartialEq)]
enum Phase {
    DeckSelection,
    Studying,
    ShowingSuccess,
    ShowingAnswer,
    Paused,
    Summary,
}

/// Study session statistics
struct SessionStats {
    reviewed: usize,
    correct: usize,
    start_time: Instant,
    end_time: Option<Instant>,
}

/// Main application state
pub struct App {
    config: Config,
    storage: Storage,
    scheduler: Scheduler,
    phase: Phase,
    // Deck selection state
    available_decks: Vec<DeckStats>,
    selected_deck_idx: usize,
    // Study state
    current_cards: Vec<StudyCard>,
    current_card_idx: usize,
    matcher: Option<Matcher>,
    card_start_time: Instant,
    attempts: u8,
    first_attempt_failed: bool,
    failed_display_until: Option<Instant>,
    success_display_until: Option<Instant>,
    // Keyboard mode for current study session
    current_keyboard_mode: Option<KeyboardMode>,
    // Pause state
    pause_chord: Option<Chord>,
    quit_chord: Option<Chord>,
    phase_before_pause: Option<Phase>,
    pause_start: Option<Instant>,
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
        let scheduler = Scheduler::new(config.desired_retention)?;

        let pause_chord = Chord::parse(&config.pause_keybind).ok();
        let quit_chord = Chord::parse(&config.quit_keybind).ok();

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
            first_attempt_failed: false,
            failed_display_until: None,
            success_display_until: None,
            current_keyboard_mode: None,
            pause_chord,
            quit_chord,
            phase_before_pause: None,
            pause_start: None,
            stats: SessionStats {
                reviewed: 0,
                correct: 0,
                start_time: Instant::now(),
                end_time: None,
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
        use crate::deck::KeyboardMode;
        use std::collections::HashMap;

        Storage::create_daily_backup(&self.config.db_path)?;

        let deck_files = list_decks(&self.config.decks_dir)?;
        let mut active_decks = HashSet::new();
        let mut keyboard_modes: HashMap<String, KeyboardMode> = HashMap::new();

        // Load each deck and sync with database
        for path in deck_files {
            let deck = Deck::load(&path)?;
            active_decks.insert(deck.name.clone());
            keyboard_modes.insert(deck.name.clone(), deck.keyboard_mode);

            // Collect keybinds in this deck file
            let mut deck_keybinds = HashSet::new();

            // Upsert all cards
            for card in &deck.cards {
                let keybind_str = card.keybind.to_string();
                deck_keybinds.insert(keybind_str.clone());
                self.storage.upsert_card(
                    &deck.name,
                    &keybind_str,
                    &card.description,
                )?;
            }

            // Delete cards that are no longer in the deck file
            self.storage.delete_removed_cards(&deck.name, &deck_keybinds)?;
        }

        // Delete decks that no longer have TSV files
        self.storage.delete_orphaned_decks(&active_decks)?;

        // Get deck stats from database
        self.available_decks = self.storage.get_deck_stats(&keyboard_modes)?;

        Ok(())
    }

    /// Push keyboard enhancement flags for a specific mode
    fn push_keyboard_mode(&mut self, mode: KeyboardMode) {
        // Pop any existing mode first
        self.pop_keyboard_mode();

        let flags = match mode {
            KeyboardMode::Raw => {
                // Raw mode: no REPORT_ALTERNATE_KEYS, get Shift+1 as-is
                KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                    | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
            }
            KeyboardMode::Chars => {
                // Character mode: with REPORT_ALTERNATE_KEYS, get '!' from Shift+1
                KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                    | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
                    | KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS
            }
        };

        let _ = execute!(stdout(), PushKeyboardEnhancementFlags(flags));
        self.current_keyboard_mode = Some(mode);
    }

    /// Pop keyboard enhancement flags if we pushed them
    fn pop_keyboard_mode(&mut self) {
        if self.current_keyboard_mode.is_some() {
            let _ = execute!(stdout(), PopKeyboardEnhancementFlags);
            self.current_keyboard_mode = None;
        }
    }

    /// Start studying the selected deck(s)
    fn start_studying(&mut self) -> Result<()> {
        self.current_cards.clear();
        self.current_card_idx = 0;
        self.stats = SessionStats {
            reviewed: 0,
            correct: 0,
            start_time: Instant::now(),
            end_time: None,
        };

        // Determine keyboard mode for this session
        let keyboard_mode = if self.selected_deck_idx < self.available_decks.len() {
            // Single deck - use its mode
            let deck = &self.available_decks[self.selected_deck_idx];
            let mode = deck.keyboard_mode;
            self.load_due_cards(&deck.name.clone())?;
            mode
        } else {
            // All decks - default to Raw (most compatible)
            for deck in self.available_decks.clone() {
                self.load_due_cards(&deck.name)?;
            }
            KeyboardMode::Raw
        };

        if self.current_cards.is_empty() {
            // No cards due
            self.stats.end_time = Some(Instant::now());
            self.phase = Phase::Summary;
        } else {
            // Push keyboard mode for this session
            self.push_keyboard_mode(keyboard_mode);

            // Randomize card order to avoid sequence-based hints
            if self.config.shuffle_cards {
                self.current_cards.shuffle(&mut rand::rng());
            }
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
            self.first_attempt_failed = false;
        }
    }

    /// Render the UI
    fn render(&self, frame: &mut Frame) {
        match self.phase {
            Phase::DeckSelection => {
                ui::render_deck_selection(frame, &self.available_decks, self.selected_deck_idx);
            }
            Phase::Studying | Phase::ShowingSuccess | Phase::ShowingAnswer => {
                if let Some(card) = self.current_cards.get(self.current_card_idx) {
                    let matcher = self.matcher.as_ref().unwrap();
                    let match_state = matcher.state();

                    let message = if self.phase == Phase::ShowingAnswer {
                        Some("Type the answer to continue")
                    } else if self.phase == Phase::Studying
                        && self.card_start_time.elapsed()
                            >= Duration::from_secs(self.config.timeout_secs)
                    {
                        Some("Time's up! Keep trying...")
                    } else {
                        None
                    };

                    let ui_state = ui::UiState {
                        deck: &card.stored.deck,
                        clue: &card.stored.description,
                        match_state: &match_state,
                        showing_answer: self.phase == Phase::ShowingAnswer,
                        answer: &card.keybind.to_string(),
                        message,
                        show_success_checkmark: self.phase == Phase::ShowingSuccess,
                    };
                    ui::render(frame, &ui_state);
                }
            }
            Phase::Paused => {
                let keybind_str = self
                    .pause_chord
                    .as_ref()
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "pause keybind".to_string());
                ui::render_paused(frame, &keybind_str);
            }
            Phase::Summary => {
                let elapsed = self
                    .stats
                    .end_time
                    .map(|end| end.duration_since(self.stats.start_time))
                    .unwrap_or_else(|| self.stats.start_time.elapsed());
                ui::render_summary(
                    frame,
                    self.stats.reviewed,
                    self.stats.correct,
                    elapsed.as_secs(),
                );
            }
        }
    }

    /// Handle input events
    fn handle_events(&mut self) -> Result<()> {
        // Check if we're in failed display period
        if let Some(until) = self.failed_display_until {
            if Instant::now() >= until {
                // Time expired, reset matcher now
                if let Some(matcher) = &mut self.matcher {
                    matcher.reset();
                }
                self.failed_display_until = None;
            } else {
                // Still showing failed state, just poll without processing
                let _ = event::poll(Duration::from_millis(50));
                return Ok(());
            }
        }

        // Check if we're in success display period
        if self.phase == Phase::ShowingSuccess
            && let Some(until) = self.success_display_until
        {
            if Instant::now() >= until {
                // Time expired, move to next card
                self.success_display_until = None;
                self.next_card()?;
            } else {
                // Still showing success state, just poll without processing
                let _ = event::poll(Duration::from_millis(50));
                return Ok(());
            }
        }

        // Poll with timeout for time-based checks
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                // Only handle key press events
                if key.kind != KeyEventKind::Press {
                    return Ok(());
                }

                // Check for quit keybind (works in any phase)
                if let Some(ref quit_chord) = self.quit_chord
                    && quit_chord.matches(&key)
                {
                    self.should_exit = true;
                    return Ok(());
                }

                // Check for pause keybind (except in DeckSelection and Summary)
                if let Some(ref pause_chord) = self.pause_chord
                    && pause_chord.matches(&key)
                {
                    if self.phase == Phase::Paused {
                        self.resume();
                        return Ok(());
                    } else if self.phase != Phase::DeckSelection && self.phase != Phase::Summary {
                        self.pause();
                        return Ok(());
                    }
                }

                match self.phase {
                    Phase::DeckSelection => self.handle_deck_selection(key)?,
                    Phase::Studying => self.handle_studying(key)?,
                    Phase::ShowingSuccess => {} // Ignore input during success display
                    Phase::ShowingAnswer => self.handle_showing_answer(key)?,
                    Phase::Paused => self.handle_paused(key),
                    Phase::Summary => self.handle_summary(key)?,
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
    fn handle_deck_selection(&mut self, key: KeyEvent) -> Result<()> {
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
                self.start_studying()?;
            }
            KeyCode::Esc | KeyCode::Char('q') => {
                self.should_exit = true;
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle studying input
    fn handle_studying(&mut self, key: KeyEvent) -> Result<()> {
        // Escape reveals the answer (counts as failed first attempt)
        if key.code == KeyCode::Esc {
            self.first_attempt_failed = true;
            self.reveal_answer()?;
            return Ok(());
        }

        self.process_keybind_input(key, false)
    }

    /// Handle showing answer input
    fn handle_showing_answer(&mut self, key: KeyEvent) -> Result<()> {
        self.process_keybind_input(key, true)
    }

    /// Common input processing for both studying and showing answer phases
    fn process_keybind_input(&mut self, key: KeyEvent, answer_revealed: bool) -> Result<()> {
        // Ignore modifier-only key presses (Ctrl, Alt, Shift by themselves)
        if matches!(key.code, KeyCode::Modifier(_)) {
            return Ok(());
        }

        // Process the key
        if let Some(matcher) = &mut self.matcher {
            let state = matcher.process(key);

            match state {
                MatchState::Complete(_) => {
                    if !answer_revealed {
                        self.attempts += 1;
                        if !self.first_attempt_failed {
                            // Got it right on first attempt! Score the card
                            self.score_card()?;
                        }
                    }
                    // Always show success flash
                    self.phase = Phase::ShowingSuccess;
                    self.success_display_until =
                        Some(Instant::now() + Duration::from_millis(self.config.success_delay_ms));
                }
                MatchState::Failed(_) => {
                    if !answer_revealed {
                        // Wrong - increment attempts (only during studying)
                        if self.attempts == 0 {
                            self.first_attempt_failed = true;
                        }
                        self.attempts += 1;
                        if self.attempts >= self.config.max_attempts {
                            self.reveal_answer()?;
                            return Ok(());
                        }
                    }
                    // Show failed state before allowing retry
                    self.failed_display_until =
                        Some(Instant::now() + Duration::from_millis(self.config.failed_flash_delay_ms));
                }
                MatchState::InProgress(_) => {
                    // Keep going
                }
            }
        }

        Ok(())
    }

    /// Handle summary input
    fn handle_summary(&mut self, key: KeyEvent) -> Result<()> {
        if key.code == KeyCode::Char('q') {
            self.should_exit = true;
        } else {
            self.phase = Phase::DeckSelection;
            self.load_deck_info()?;
        }
        Ok(())
    }

    /// Handle paused input (pause/quit keybinds handled globally in handle_events)
    fn handle_paused(&mut self, _key: KeyEvent) {
        // No additional input handling needed - pause toggle and quit are global
    }

    /// Pause the app
    fn pause(&mut self) {
        self.phase_before_pause = Some(self.phase.clone());
        self.pause_start = Some(Instant::now());
        self.phase = Phase::Paused;
    }

    /// Resume from pause
    fn resume(&mut self) {
        if let Some(prev_phase) = self.phase_before_pause.take() {
            // Add paused duration to card_start_time to avoid timeout during pause
            if let Some(pause_start) = self.pause_start.take() {
                let paused_duration = pause_start.elapsed();
                self.card_start_time += paused_duration;
            }
            self.phase = prev_phase;
        }
    }

    /// Check for timeout
    fn check_timeout(&mut self) -> Result<()> {
        let elapsed = self.card_start_time.elapsed();
        let timeout = Duration::from_secs(self.config.timeout_secs);

        if elapsed >= timeout && self.attempts == 0 {
            // Auto-mark as failed attempt on first timeout
            self.attempts = 1;
            self.first_attempt_failed = true;
        }

        Ok(())
    }

    /// Reveal the answer (scoring happens when user completes typing)
    fn reveal_answer(&mut self) -> Result<()> {
        self.phase = Phase::ShowingAnswer;

        // Reset matcher for typing the answer
        if let Some(card) = self.current_cards.get(self.current_card_idx) {
            self.matcher = Some(Matcher::new(card.keybind.clone()));
        }

        Ok(())
    }

    /// Score the current card (only called when user gets it right on first attempt)
    fn score_card(&mut self) -> Result<()> {
        if let Some(card) = self.current_cards.get(self.current_card_idx) {
            let response_time_ms = self.card_start_time.elapsed().as_millis() as u64;

            // Calculate rating based on performance and prior presentation count
            let rating = Rating::from_performance(
                response_time_ms,
                self.attempts,
                card.stored.current_presentation_count,
            );

            // Get current memory state
            let memory_state = card.stored.stability.and_then(|s| {
                card.stored
                    .difficulty
                    .map(|d| Scheduler::memory_state_from_stored(s, d))
            });

            // Schedule next review
            let (new_memory, due_date) =
                self.scheduler
                    .schedule(memory_state, card.stored.last_review, rating)?;

            // Update storage (also resets presentation count)
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
            self.stats.correct += 1;
        }

        Ok(())
    }

    /// Move to the next card
    fn next_card(&mut self) -> Result<()> {
        // If first attempt failed, increment presentation count and push card to back of queue
        if self.first_attempt_failed
            && let Some(card) = self.current_cards.get(self.current_card_idx)
        {
            self.storage.increment_presentation_count(card.stored.id)?;

            let mut updated_stored = card.stored.clone();
            updated_stored.current_presentation_count += 1;
            self.current_cards.push(StudyCard {
                stored: updated_stored,
                keybind: card.keybind.clone(),
            });
        }

        self.current_card_idx += 1;

        if self.current_card_idx >= self.current_cards.len() {
            // Done with all cards
            self.stats.end_time = Some(Instant::now());
            self.pop_keyboard_mode();
            self.phase = Phase::Summary;
        } else {
            self.phase = Phase::Studying;
            self.setup_current_card();
        }

        Ok(())
    }
}
