# Scheduling

kbsr uses the [FSRS](https://github.com/open-spaced-repetition/fsrs-rs) (Free Spaced Repetition Scheduler) algorithm to decide when you should next review each card. FSRS is the same algorithm used by Anki, but kbsr compresses the intervals to suit muscle memory practice rather than long-term recall.

## How a Card is Scored

Every card is scored on its **first showing** in a session. The score is based on how fast you typed the keybind correctly:

| Rating | Criteria |
|--------|----------|
| **Easy** | Correct, 1 attempt, under the easy threshold |
| **Good** | Correct, 1 attempt, between easy and hard thresholds |
| **Hard** | Correct but slow (≥ hard threshold), or took 2 attempts |
| **Again** | 3+ attempts, or timed out |

If you press Escape to reveal the answer, or exceed `max_attempts`, the card is scored as **Again**.

The default thresholds are:

| Threshold | Default |
|-----------|---------|
| `easy_threshold_ms` | 2000 ms |
| `hard_threshold_ms` | 5000 ms |
| `timeout_secs` | 10 s |

### Multi-chord scaling

For keybinds with multiple chords (e.g., `Ctrl+K Ctrl+C` or `y s i w )`), the time thresholds scale by **+20% per additional chord**. For example, with the default 2000 ms easy threshold:

| Chords | Easy threshold |
|--------|---------------|
| 1 | 2000 ms |
| 2 | 2400 ms |
| 3 | 2800 ms |
| 5 | 3600 ms |

## Session Practice

After scoring, if the card wasn't rated **Easy**, it's pushed to the back of the session queue for more practice. You keep seeing the card until you can type it fast enough for an Easy rating. These practice repetitions don't affect scheduling — only the first showing counts.

## How FSRS Scheduling Works

FSRS tracks two values per card:

- **Stability (S):** The number of days for your recall probability to decay from 100% to your `desired_retention` (default 90%). A stability of 10 means after 10 days you have a 90% chance of remembering. Higher stability = you remember longer.

- **Difficulty (D):** A value (1–10) representing how inherently hard the card is. Higher difficulty means stability grows more slowly across reviews.

When you review a card, FSRS takes the card's current stability, difficulty, days since last review, and your desired retention rate, and computes a new stability and difficulty based on your rating:

- **Easy** → large stability increase, difficulty decreases
- **Good** → moderate stability increase
- **Hard** → small stability increase, difficulty increases
- **Again** → stability drops significantly, difficulty increases

For a new card with no prior reviews, FSRS uses built-in default parameters to compute initial stability and difficulty from the first rating.

## Interval Compression

Standard FSRS intervals are designed for textbook-style recall — days to weeks between reviews. Muscle memory benefits from much more frequent practice, so kbsr applies an `interval_modifier` (default 0.12) to compress the raw FSRS intervals:

| Rating | Raw FSRS interval | After 0.12 modifier |
|--------|-------------------|---------------------|
| **Again** | ~5 hours | ~36 minutes |
| **Hard** | ~31 hours | ~3.7 hours |
| **Good** | ~55 hours | ~6.6 hours |
| **Easy** | ~199 hours | ~1 day |

These are approximate values for a new card. As stability grows across reviews, intervals increase.

A `max_interval_days` cap (default 30) prevents any card from being scheduled further than 30 days out. Set `interval_modifier` to `1.0` for standard FSRS intervals (like Anki).

## Configuration

All scheduling parameters are configurable in `~/.config/kbsr/config.toml`. See [Configuration](configuration.md) for the full reference.
