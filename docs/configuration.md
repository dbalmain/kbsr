# Configuration

Configuration is optional. All settings have sensible defaults. To customize, create `~/.config/kbsr/config.toml`:

```toml
# Timeout in seconds before marking as incorrect (default: 10)
# After timeout, you can keep trying but the card won't count as "first try correct"
timeout_secs = 10

# Maximum attempts before revealing the answer (default: 3)
max_attempts = 3

# Response time threshold in ms for Easy rating (default: 3000)
# Cards answered faster than this (1 attempt) are rated Easy
easy_threshold_ms = 3000

# Response time threshold in ms for Hard rating (default: 6000)
# Cards answered slower than this are rated Hard
hard_threshold_ms = 6000

# Delay in ms to show success checkmark (default: 500)
success_delay_ms = 500

# Delay in ms to show red failed flash before retry (default: 500)
failed_flash_delay_ms = 500

# Keybind to pause during study (default: "Super+Ctrl+P")
# Uses obscure modifier combo to avoid conflicting with keybinds you're learning
pause_keybind = "Super+Ctrl+P"

# Keybind to quit the app (default: "Super+Ctrl+Q")
quit_keybind = "Super+Ctrl+Q"

# Shuffle cards before each session (default: true)
shuffle_cards = true

# FSRS desired retention rate 0.0-1.0 (default: 0.9)
# Higher = more frequent reviews, better retention
# Lower = fewer reviews, more forgetting
desired_retention = 0.9

# Interval multiplier applied to FSRS intervals (default: 0.12)
# Lower = more frequent reviews. At 0.12, "Easy" on a new card ≈ 1 day.
# Set to 1.0 for standard FSRS intervals (like Anki).
interval_modifier = 0.12

# Maximum interval in days between reviews (default: 30)
# Cards will never be scheduled further out than this.
max_interval_days = 30.0

# Custom paths (optional, defaults to XDG directories)
# decks_dir = "/path/to/decks"
# db_path = "/path/to/kbsr.db"
```

## Settings Reference

| Setting | Default | Description |
|---------|---------|-------------|
| `timeout_secs` | `10` | Seconds before auto-marking card as incorrect |
| `max_attempts` | `3` | Wrong attempts before answer is revealed |
| `easy_threshold_ms` | `3000` | Response time threshold (ms) for Easy rating |
| `hard_threshold_ms` | `6000` | Response time threshold (ms) for Hard rating |
| `success_delay_ms` | `500` | How long the green checkmark is shown |
| `failed_flash_delay_ms` | `500` | How long wrong input flashes red before retry |
| `pause_keybind` | `Super+Ctrl+P` | Chord to pause the session |
| `quit_keybind` | `Super+Ctrl+Q` | Chord to quit from any screen |
| `shuffle_cards` | `true` | Randomize card order each session |
| `desired_retention` | `0.9` | Target recall probability for FSRS scheduling |
| `interval_modifier` | `0.12` | Multiplier for FSRS intervals (lower = more frequent reviews) |
| `max_interval_days` | `30.0` | Maximum days between reviews |
| `decks_dir` | `~/.config/kbsr/decks` | Where deck TSV files are stored |
| `db_path` | `~/.local/share/kbsr/kbsr.db` | SQLite database location |
