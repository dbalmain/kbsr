# Deck Format

Decks are TSV (tab-separated values) files placed in `~/.config/kbsr/decks/`. Each line has a keybind and description separated by a tab:

```
KEYBIND<TAB>DESCRIPTION
```

Lines starting with `#` are comments (or directives). Empty lines are skipped.

## Keyboard Modes

Different applications handle keyboard input differently. Use the `# mode:` directive at the top of your deck to match your target application.

### Raw Mode (default)

Best for **window managers** (hyprland, sway, i3) and applications that bind to physical keys with modifiers:

```tsv
# mode: raw
Super+1	Switch to workspace 1
Super+Shift+1	Move window to workspace 1
Super+Return	Open terminal
Ctrl+Shift+V	Paste in terminal
```

In raw mode, `Shift` is treated as an explicit modifier. Typing `Shift+1` matches `Shift+1`, not `!`.

### Chars Mode

Best for **vim**, **emacs**, and applications that respond to the resulting character:

```tsv
# mode: chars
G	Go to end of file
g g	Go to start of file
$	Go to end of line
d d	Delete line
Ctrl+R	Redo
```

In chars mode, typing `Shift+g` produces `G`, and `Shift+4` produces `$`. The keybind is matched against the character, not the physical keys.

### Command Mode

Best for learning **CLI commands** like git, docker, kubectl, etc:

```tsv
# mode: command
ls -la	List files with details
git stash pop	Apply and drop latest stash
docker compose up -d	Start containers in background
kubectl get pods	List pods
```

In command mode, each character of the command (including spaces) becomes its own input. You type the command character by character and press `Enter` to submit. The answer is displayed as the full command string.

## Multi-Chord Sequences

Keybinds can contain multiple chords separated by spaces. Each chord is typed sequentially:

```tsv
g g	Go to top of file
Ctrl+K Ctrl+C	Comment selection (VS Code)
Ctrl+X Ctrl+S	Save file (Emacs)
```

During practice, each correct chord appears in green. A wrong chord turns the entire input red and you restart the sequence.

## Supported Keys

**Modifiers:** `Ctrl`, `Alt`, `Shift`, `Super`, `Meta`, `Hyper`

**Special keys:** `Space`, `Tab`, `Enter`, `Escape`, `Backspace`, `Delete`, `Insert`, `Home`, `End`, `PageUp`, `PageDown`, `Up`, `Down`, `Left`, `Right`, `F1`-`F12`, `CapsLock`, `ScrollLock`, `NumLock`, `PrintScreen`, `Pause`, `Menu`

**Characters:** Any single character (`a`, `A`, `$`, `!`, etc.)

## Example Decks

### vim.tsv

```tsv
# mode: chars
# Navigation
g g	Go to first line
G	Go to last line
0	Go to start of line
$	Go to end of line
w	Next word
b	Previous word

# Editing
d d	Delete line
y y	Yank line
p	Paste after cursor
u	Undo
Ctrl+R	Redo

# Search
/	Search forward
?	Search backward
n	Next match
N	Previous match
```

### git.tsv

```tsv
# mode: command
git stash	Stash changes
git stash pop	Apply and drop latest stash
git rebase -i HEAD~3	Interactive rebase last 3 commits
git log --oneline	Compact log
```

### hyprland.tsv

```tsv
# mode: raw
# Workspaces
Super+1	Switch to workspace 1
Super+2	Switch to workspace 2
Super+3	Switch to workspace 3
Super+Shift+1	Move window to workspace 1
Super+Shift+2	Move window to workspace 2

# Windows
Super+Q	Close window
Super+F	Toggle fullscreen
Super+V	Toggle floating
Super+Left	Focus left
Super+Right	Focus right

# Launch
Super+Return	Terminal
Super+D	App launcher
```

## Editing Decks

When you edit a deck file and restart kbsr:

- **New cards** are added automatically
- **Removed cards** are deleted from the database (along with their review history)
- **Changed descriptions** reset that card's spaced repetition progress

Daily backups are created automatically in `~/.local/share/kbsr/` in case you need to restore progress.

## Aligning Columns in Your Editor

Since deck files are tab-separated, you can use an `.editorconfig` file to make the columns line up nicely. Place this in your decks directory (`~/.config/kbsr/decks/.editorconfig`):

```ini
[*.tsv]
tab_width = 20
indent_style = tab
```

This sets the tab stop width to 20 characters (adjust as required), so keybinds and descriptions align into readable columns in any editor that supports [EditorConfig](https://editorconfig.org/).
