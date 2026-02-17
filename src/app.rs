use crate::config::Config;
use crate::deck::{Deck, KeyboardMode, list_decks};
use crate::keybind::{Chord, Keybind};
use crate::matcher::{MatchState, Matcher};
use crate::scheduler::{Rating, Scheduler};
use crate::storage::{DeckStats, DeckSyncInput, Storage, StoredCard};
use crate::ui;
use anyhow::{Context, Result};
use crossterm::event::{
    self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, KeyboardEnhancementFlags,
    PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::execute;
use rand::seq::SliceRandom;
use ratatui::{DefaultTerminal, Frame};
use std::collections::{HashMap, HashSet};
use std::io::stdout;
use std::time::{Duration, Instant};

struct SessionStats {
    reviewed: usize,
    correct: usize,
    start_time: Instant,
    end_time: Option<Instant>,
}

struct StudyCard {
    stored: StoredCard,
    keybind: Keybind,
}

struct DeckSelectionState {
    available_decks: Vec<DeckStats>,
}

struct StudyState {
    cards: Vec<StudyCard>,
    card_idx: usize,
    matcher: Matcher,
    card_start_time: Instant,
    attempts: u8,
    scored_card_ids: HashSet<i64>,
    requeue_for_practice: bool,
    failed_display_until: Option<Instant>,
    success_display_until: Option<Instant>,
    stats: SessionStats,
}

struct PausedState {
    previous: Box<AppState>,
    started_at: Instant,
}

struct SummaryState {
    stats: SessionStats,
}

enum AppState {
    DeckSelection(DeckSelectionState),
    Studying(StudyState),
    Paused(PausedState),
    Summary(SummaryState),
}

impl Default for AppState {
    fn default() -> Self {
        AppState::DeckSelection(DeckSelectionState {
            available_decks: Vec::new(),
        })
    }
}

pub struct App {
    config: Config,
    storage: Storage,
    scheduler: Scheduler,
    pause_chord: Option<Chord>,
    quit_chord: Option<Chord>,
    should_exit: bool,
    current_keyboard_mode: Option<KeyboardMode>,
    keyboard_modes: HashMap<String, KeyboardMode>,
    selected_deck_idx: usize,
    show_hints: bool,
    state: AppState,
}

impl App {
    pub fn new(config: Config) -> Result<Self> {
        config.ensure_dirs()?;
        let storage = Storage::open(&config.db_path)?;
        let scheduler = Scheduler::new(
            config.desired_retention,
            config.interval_modifier,
            config.max_interval_days,
        )?;

        let pause_chord = Some(Chord::parse(&config.pause_keybind).with_context(|| {
            format!("Invalid pause_keybind '{}' in config", config.pause_keybind)
        })?);
        let quit_chord = Some(Chord::parse(&config.quit_keybind).with_context(|| {
            format!("Invalid quit_keybind '{}' in config", config.quit_keybind)
        })?);

        let show_hints = storage
            .get_setting("show_hints")
            .ok()
            .flatten()
            .map(|v| v != "false")
            .unwrap_or(true);

        Ok(Self {
            config,
            storage,
            scheduler,
            pause_chord,
            quit_chord,
            should_exit: false,
            current_keyboard_mode: None,
            keyboard_modes: HashMap::new(),
            selected_deck_idx: 0,
            show_hints,
            state: AppState::DeckSelection(DeckSelectionState {
                available_decks: Vec::new(),
            }),
        })
    }

    pub fn run(mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        self.sync_deck_info()?;

        while !self.should_exit {
            terminal.draw(|frame| self.render(frame))?;
            self.handle_events()?;
        }

        Ok(())
    }

    fn sync_deck_info(&mut self) -> Result<()> {
        Storage::create_daily_backup(&self.config.db_path)?;

        let deck_files = list_decks(&self.config.decks_dir)?;
        let mut active_decks = HashSet::new();
        let mut sync_inputs = Vec::new();

        for path in deck_files {
            let deck = Deck::load(&path)?;
            active_decks.insert(deck.name.clone());
            self.keyboard_modes
                .insert(deck.name.clone(), deck.keyboard_mode);

            let keybinds = deck
                .cards
                .iter()
                .map(|card| (card.keybind.to_string(), card.description.clone()))
                .collect();

            sync_inputs.push(DeckSyncInput {
                deck_name: deck.name,
                keybinds,
            });
        }

        self.storage.sync_decks(sync_inputs, &active_decks)?;
        self.refresh_deck_stats()?;

        Ok(())
    }

    fn refresh_deck_stats(&mut self) -> Result<()> {
        let available_decks = self.storage.get_deck_stats(&self.keyboard_modes)?;
        self.selected_deck_idx = self.selected_deck_idx.min(available_decks.len());
        self.state = AppState::DeckSelection(DeckSelectionState { available_decks });
        Ok(())
    }

    fn push_keyboard_mode(&mut self, mode: KeyboardMode) {
        self.pop_keyboard_mode();

        let flags = match mode {
            KeyboardMode::Raw => {
                KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                    | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
            }
            KeyboardMode::Chars => {
                KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                    | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
                    | KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS
            }
        };

        if execute!(stdout(), PushKeyboardEnhancementFlags(flags)).is_ok() {
            self.current_keyboard_mode = Some(mode);
        }
    }

    fn pop_keyboard_mode(&mut self) {
        if self.current_keyboard_mode.is_some() {
            let _ = execute!(stdout(), PopKeyboardEnhancementFlags);
            self.current_keyboard_mode = None;
        }
    }

    fn render(&self, frame: &mut Frame) {
        match &self.state {
            AppState::DeckSelection(s) => {
                ui::render_deck_selection(
                    frame,
                    &s.available_decks,
                    self.selected_deck_idx,
                    self.show_hints,
                );
            }
            AppState::Studying(s) => {
                if let Some(card) = s.cards.get(s.card_idx) {
                    let match_state = s.matcher.state();

                    let message = if s.attempts >= self.config.max_attempts {
                        Some("Type the answer to continue")
                    } else {
                        None
                    };

                    let answer_str = card.keybind.to_string();
                    let pause_str = self
                        .pause_chord
                        .as_ref()
                        .map(|c| c.to_string())
                        .unwrap_or_default();
                    let quit_str = self
                        .quit_chord
                        .as_ref()
                        .map(|c| c.to_string())
                        .unwrap_or_default();
                    let ui_state = ui::UiState {
                        deck: &card.stored.deck,
                        clue: &card.stored.description,
                        match_state: &match_state,
                        showing_answer: s.attempts >= self.config.max_attempts,
                        answer: &answer_str,
                        message,
                        show_success_checkmark: s.success_display_until.is_some(),
                        card_cleared: !s.requeue_for_practice,
                        show_hints: self.show_hints,
                        pause_keybind: &pause_str,
                        quit_keybind: &quit_str,
                        cards_remaining: s.cards.len() - s.card_idx,
                    };
                    ui::render(frame, &ui_state);
                }
            }
            AppState::Paused(_) => {
                let keybind_str = self
                    .pause_chord
                    .as_ref()
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "pause keybind".to_string());
                ui::render_paused(frame, &keybind_str);
            }
            AppState::Summary(s) => {
                let elapsed = s
                    .stats
                    .end_time
                    .map(|end| end.duration_since(s.stats.start_time))
                    .unwrap_or_else(|| s.stats.start_time.elapsed());
                ui::render_summary(
                    frame,
                    s.stats.reviewed,
                    s.stats.correct,
                    elapsed.as_secs(),
                    self.show_hints,
                );
            }
        }
    }

    fn handle_events(&mut self) -> Result<()> {
        if let AppState::Studying(ref mut study) = self.state {
            if let Some(until) = study.failed_display_until {
                if Instant::now() >= until {
                    study.matcher.reset();
                    study.failed_display_until = None;
                } else {
                    let _ = event::poll(Duration::from_millis(50));
                    return Ok(());
                }
            }

            if let Some(until) = study.success_display_until {
                if Instant::now() >= until {
                    let AppState::Studying(mut study) = std::mem::take(&mut self.state) else {
                        unreachable!()
                    };
                    study.success_display_until = None;
                    self.next_card(study)?;
                } else {
                    let _ = event::poll(Duration::from_millis(50));
                    return Ok(());
                }
            }
        }

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    return Ok(());
                }

                if let Some(ref quit_chord) = self.quit_chord
                    && quit_chord.matches(&key, KeyboardMode::Raw)
                {
                    if matches!(self.state, AppState::DeckSelection(_)) {
                        self.should_exit = true;
                    } else {
                        self.pop_keyboard_mode();
                        self.refresh_deck_stats()?;
                    }
                    return Ok(());
                }

                if let Some(ref pause_chord) = self.pause_chord
                    && pause_chord.matches(&key, KeyboardMode::Raw)
                {
                    if matches!(self.state, AppState::Paused(_)) {
                        self.resume();
                        return Ok(());
                    } else if !matches!(
                        self.state,
                        AppState::DeckSelection(_) | AppState::Summary(_)
                    ) {
                        self.pause();
                        return Ok(());
                    }
                }

                match &self.state {
                    AppState::DeckSelection(_) => self.handle_deck_selection_key(key)?,
                    AppState::Studying(_) => self.handle_studying_key(key)?,
                    AppState::Paused(_) => {}
                    AppState::Summary(_) => self.handle_summary_key(key)?,
                }
            }
        } else if matches!(self.state, AppState::Studying(_)) {
            self.check_timeout();
        }

        Ok(())
    }

    fn handle_deck_selection_key(&mut self, key: KeyEvent) -> Result<()> {
        let AppState::DeckSelection(ref mut s) = self.state else {
            return Ok(());
        };
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if self.selected_deck_idx > 0 {
                    self.selected_deck_idx -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.selected_deck_idx < s.available_decks.len() {
                    self.selected_deck_idx += 1;
                }
            }
            KeyCode::Enter => {
                if let AppState::DeckSelection(ds) = std::mem::take(&mut self.state) {
                    self.start_studying(ds)?;
                };
            }
            KeyCode::Esc | KeyCode::Char('q') => {
                self.should_exit = true;
            }
            KeyCode::Char('/') if key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.show_hints = !self.show_hints;
                let _ = self
                    .storage
                    .set_setting("show_hints", if self.show_hints { "true" } else { "false" });
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_studying_key(&mut self, key: KeyEvent) -> Result<()> {
        let AppState::Studying(ref mut study) = self.state else {
            return Ok(());
        };

        if matches!(key.code, KeyCode::Modifier(_)) {
            return Ok(());
        }

        if key.code == KeyCode::Esc && study.attempts < self.config.max_attempts {
            study.attempts = self.config.max_attempts;
            study.matcher = Matcher::new(
                study.cards[study.card_idx].keybind.clone(),
                self.current_keyboard_mode.unwrap_or_default(),
            );
            return Ok(());
        }

        let result = study.matcher.process(key);

        match result {
            MatchState::Complete(_) => {
                let AppState::Studying(ref mut study) = self.state else {
                    unreachable!()
                };
                study.attempts = study.attempts.saturating_add(1);
                let response_time_ms = study.card_start_time.elapsed().as_millis() as u64;
                let card = &study.cards[study.card_idx];
                let card_id = card.stored.id;
                let num_chords = card.keybind.len();
                let easy_ms = Rating::scale_threshold(self.config.easy_threshold_ms, num_chords);
                let hard_ms = Rating::scale_threshold(self.config.hard_threshold_ms, num_chords);
                let rating = Rating::from_speed(
                    response_time_ms,
                    study.attempts,
                    easy_ms,
                    hard_ms,
                    self.config.max_attempts,
                );

                if study.scored_card_ids.insert(card_id) {
                    let memory_state = card.stored.stability.and_then(|s| {
                        card.stored
                            .difficulty
                            .map(|d| Scheduler::memory_state_from_stored(s, d))
                    });
                    let last_review = card.stored.last_review;
                    let (new_memory, due_date) =
                        self.scheduler.schedule(memory_state, last_review, rating)?;
                    self.storage.update_card_after_review(
                        card_id,
                        new_memory.stability,
                        new_memory.difficulty,
                        due_date,
                    )?;
                    self.storage.record_review(
                        card_id,
                        rating.as_u32() as i32,
                        response_time_ms as i64,
                        study.attempts as i32,
                    )?;
                    study.stats.reviewed += 1;
                }

                if rating != Rating::Easy {
                    study.requeue_for_practice = true;
                }
                study.success_display_until =
                    Some(Instant::now() + Duration::from_millis(self.config.success_delay_ms));
            }
            MatchState::Failed(_) => {
                study.attempts = study.attempts.saturating_add(1);
                study.failed_display_until =
                    Some(Instant::now() + Duration::from_millis(self.config.failed_flash_delay_ms));
            }
            MatchState::InProgress(_) => {}
        }

        Ok(())
    }

    fn handle_summary_key(&mut self, _key: KeyEvent) -> Result<()> {
        self.refresh_deck_stats()?;
        Ok(())
    }

    fn start_studying(&mut self, deck_selection: DeckSelectionState) -> Result<()> {
        let mut cards = Vec::new();
        let stats = SessionStats {
            reviewed: 0,
            correct: 0,
            start_time: Instant::now(),
            end_time: None,
        };

        let keyboard_mode = if self.selected_deck_idx < deck_selection.available_decks.len() {
            let deck = &deck_selection.available_decks[self.selected_deck_idx];
            let mode = deck.keyboard_mode;
            let name = deck.name.clone();
            self.load_due_cards(&name, &mut cards)?;
            mode
        } else {
            let deck_names: Vec<String> = deck_selection
                .available_decks
                .iter()
                .map(|d| d.name.clone())
                .collect();
            for name in &deck_names {
                self.load_due_cards(name, &mut cards)?;
            }
            KeyboardMode::Raw
        };

        if cards.is_empty() {
            self.state = AppState::Summary(SummaryState {
                stats: SessionStats {
                    reviewed: 0,
                    correct: 0,
                    start_time: Instant::now(),
                    end_time: Some(Instant::now()),
                },
            });
        } else {
            self.push_keyboard_mode(keyboard_mode);

            if self.config.shuffle_cards {
                cards.shuffle(&mut rand::rng());
            }

            let matcher = Matcher::new(cards[0].keybind.clone(), keyboard_mode);

            self.state = AppState::Studying(StudyState {
                cards,
                card_idx: 0,
                matcher,
                card_start_time: Instant::now(),
                attempts: 0,
                scored_card_ids: HashSet::new(),
                requeue_for_practice: false,
                failed_display_until: None,
                success_display_until: None,
                stats,
            });
        }

        Ok(())
    }

    fn load_due_cards(&mut self, deck_name: &str, cards: &mut Vec<StudyCard>) -> Result<()> {
        let stored_cards = self.storage.get_due_cards(deck_name)?;

        for stored in stored_cards {
            if let Ok(keybind) = Keybind::parse(&stored.keybind) {
                cards.push(StudyCard { stored, keybind });
            }
        }

        Ok(())
    }

    fn setup_current_card(study: &mut StudyState, mode: KeyboardMode) {
        if let Some(card) = study.cards.get(study.card_idx) {
            study.matcher = Matcher::new(card.keybind.clone(), mode);
            study.card_start_time = Instant::now();
            study.attempts = 0;
            study.requeue_for_practice = false;
        }
    }

    fn next_card(&mut self, mut study: StudyState) -> Result<()> {
        if study.requeue_for_practice {
            if let Some(card) = study.cards.get(study.card_idx) {
                study.cards.push(StudyCard {
                    stored: card.stored.clone(),
                    keybind: card.keybind.clone(),
                });
            }
        } else {
            study.stats.correct += 1;
        }

        study.card_idx += 1;

        if study.card_idx >= study.cards.len() {
            study.stats.end_time = Some(Instant::now());
            self.pop_keyboard_mode();
            self.state = AppState::Summary(SummaryState { stats: study.stats });
        } else {
            Self::setup_current_card(&mut study, self.current_keyboard_mode.unwrap_or_default());
            self.state = AppState::Studying(study);
        }

        Ok(())
    }

    fn pause(&mut self) {
        let prev = std::mem::take(&mut self.state);
        self.state = AppState::Paused(PausedState {
            previous: Box::new(prev),
            started_at: Instant::now(),
        });
    }

    fn resume(&mut self) {
        let AppState::Paused(paused) = std::mem::take(&mut self.state) else {
            return;
        };
        let mut prev = *paused.previous;
        let delta = paused.started_at.elapsed();
        if let AppState::Studying(ref mut s) = prev {
            s.card_start_time += delta;
        }
        self.state = prev;
    }

    fn check_timeout(&mut self) {
        let AppState::Studying(ref mut study) = self.state else {
            return;
        };
        let elapsed = study.card_start_time.elapsed();
        let timeout = Duration::from_secs(self.config.timeout_secs);

        if elapsed >= timeout && study.attempts < self.config.max_attempts {
            study.attempts = self.config.max_attempts;
            study.matcher = Matcher::new(
                study.cards[study.card_idx].keybind.clone(),
                self.current_keyboard_mode.unwrap_or_default(),
            );
        }
    }
}
