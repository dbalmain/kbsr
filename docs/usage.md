# Usage Guide

## Study Flow

1. **Select a deck** - Pick a single deck or "All decks"
2. **Read the clue** - The description tells you what keybind to type
3. **Type the keybind** - Press the actual keys (captured as raw key events)
4. **Get feedback**:
   - **Green checkmark**: Correct on first try - the card is scored and scheduled for future review
   - **Red flash**: Wrong chord - try again from the beginning of the sequence
   - **Timeout** (5s default): Card is marked as missed, but you can keep trying
   - **Max attempts** (3 default) or **Escape**: Answer is revealed
5. **If the answer was revealed**: You must type the correct keybind to continue (reinforces muscle memory)
6. **Session ends** when all due cards have been reviewed

## Controls

### Deck Selection

| Key | Action |
|-----|--------|
| `Up` / `k` | Move selection up |
| `Down` / `j` | Move selection down |
| `Enter` | Start studying selected deck |
| `q` / `Esc` | Quit |

### During Study

| Key | Action |
|-----|--------|
| *(type the keybind)* | Answer the card |
| `Escape` | Reveal the answer |
| `Super+Ctrl+P` | Pause session (configurable) |
| `Super+Ctrl+Q` | Quit (configurable) |

### Summary Screen

| Key | Action |
|-----|--------|
| `q` | Quit |
| Any other key | Return to deck selection |

## How Scoring Works

The FSRS rating is determined by:

| Rating | Condition |
|--------|-----------|
| **Easy** | Correct in < 2 seconds, first attempt, no prior presentations |
| **Good** | Correct in 2-5 seconds, first attempt, no prior presentations |
| **Hard** | Correct but slow (>= 5s), or took 2 attempts, or needed 1-2 prior presentations |
| **Again** | 3+ attempts, or needed 3+ prior presentations this session |

**Cards you miss** are pushed to the end of the current session queue with an incremented presentation count. This gives you another chance to practice, but repeated presentations reduce the rating even when you eventually get it right.

## Tips

### Avoiding keybind capture

Some keybindings may actually be captured before reaching kbsr. For example, if you're practicing your tmux keybinds, they'll all be captured by tmux instead of going to kbsr, so you should avoid running it in tmux when studying your tmux keybinds.

Similarly, your operating system keybinds will be captured before passing to kbsr. If you use Hyprland, you can work around this with a passthrough submap that disables all other keybinds. Add this to your Hyprland config:

```
bind = $mainMod, Escape, submap, passthrough

submap = passthrough
bind = $mainMod, Escape, submap, reset
submap = reset
```

Press `Super+Escape` before starting a study session to enter passthrough mode — all keybinds except `Super+Escape` are disabled and go directly to kbsr. Press `Super+Escape` again when you're done to restore your normal keybinds.

### Terminal Compatibility

kbsr uses the Kitty keyboard protocol for enhanced key detection. Supported terminals:

- Kitty
- WezTerm
- Ghostty
- foot
- Alacritty

### Backups

A daily backup of the database is created automatically at `~/.local/share/kbsr/kbsr.db.backup.YYYY-MM-DD` when you start a session. If you accidentally delete progress by editing a deck, you can restore from a backup.
