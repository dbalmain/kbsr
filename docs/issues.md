# Code Issues

Tracked issues from code review, ordered by severity.

## #1 — ~~BUG: Success checkmark renders in wrong layout chunk~~ RESOLVED

Not a bug. The answer and checkmark are mutually exclusive and correctly share `chunks[5]`. The layout comments were misleading and have been fixed.

---

## #2 — ~~BUG: Panic on corrupted datetime in review history~~ RESOLVED

Replaced `.unwrap()` with `.map_err()` into `rusqlite::Error::FromSqlConversionFailure` so the error propagates instead of panicking. Fixed alongside issue #6.

---

## #3 — ~~Errors from start_studying silently swallowed~~ RESOLVED

Changed `handle_deck_selection` to return `Result<()>` and propagate errors from `start_studying()` with `?` instead of discarding with `let _ =`.

---

## #4 — ~~Errors from load_deck_info silently swallowed~~ RESOLVED

Changed `handle_summary` to return `Result<()>` and propagate errors from `load_deck_info()` with `?` instead of discarding with `.ok()`.

---

## #5 — ~~Silent datetime parse failures in row_to_stored_card~~ WON'T FIX

Current behavior is intentional. A corrupted due_date treated as `None` means "due now", which self-heals: the user studies the card (< 2 seconds), and the next review writes a fresh valid timestamp.

---

## #6 — ~~Foreign key constraints not enforced~~ RESOLVED

Added `conn.pragma_update(None, "foreign_keys", "ON")?;` in `Storage::open` before schema init.

---

## #7 — ~~App is a god-struct with 18+ fields~~ RESOLVED

Replaced flat `Phase` enum + 18 scattered fields with a discriminated `AppState` enum (`DeckSelection`, `Studying`, `ShowingSuccess`, `ShowingAnswer`, `Paused`, `Summary`) where each variant carries only its relevant state. `App` now holds only shared fields (config, storage, scheduler, keybinds). `matcher` is non-optional in `StudyState`. Pause uses `Box<AppState>` snapshot with `std::mem::replace`.

---

## #8 — Deck sync has no transaction

**File:** `src/app.rs:131-172`

`load_deck_info` performs multiple upserts, deletes removed cards, then deletes orphaned decks with no enclosing transaction. A failure mid-way leaves the DB in a partially-updated state.

**Fix:** Wrap the entire sync in a single SQLite transaction.

---

## #9 — push_keyboard_mode updates state even on failure

**File:** `src/app.rs:193`

`push_keyboard_mode` ignores the result of `execute!` but unconditionally sets `current_keyboard_mode = Some(mode)`. If the terminal doesn't support the mode, internal state drifts from actual terminal state, and `pop_keyboard_mode` will issue a spurious pop.

```rust
let _ = execute!(stdout(), PushKeyboardEnhancementFlags(flags));
self.current_keyboard_mode = Some(mode);
```

**Fix:** Only update state on success. Consider an RAII guard that pops on drop to guarantee cleanup.

---

## #10 — Invalid pause/quit keybinds fail silently

**File:** `src/app.rs:83-84`

If the user's config has a malformed pause or quit keybind, `Chord::parse().ok()` silently produces `None` and those features just stop working.

```rust
let pause_chord = Chord::parse(&config.pause_keybind).ok();
let quit_chord = Chord::parse(&config.quit_keybind).ok();
```

**Fix:** Fail fast with a clear error message, or warn and fall back to the default keybind.

---

## #11 — ~~Dead parameter in render_typed_chords~~ RESOLVED

Removed unused `_showing_answer` parameter from `render_typed_chords` and updated the call site. Fixed alongside issue #7.
