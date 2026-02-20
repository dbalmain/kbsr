use crate::keybind::Keybind;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Keyboard input mode for a deck
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KeyboardMode {
    /// Raw mode: report keys with explicit modifiers (e.g., Super+Shift+1)
    /// Best for window manager bindings like hyprland
    #[default]
    Raw,
    /// Character mode: report the resulting character (e.g., G, $, !)
    /// Best for vim-style bindings
    Chars,
    /// Command mode: each character in a CLI command becomes its own chord
    /// Best for learning CLI commands (e.g., `ls -la`, `git stash pop`)
    Command,
}

/// A single card in a deck
#[derive(Debug, Clone)]
pub struct Card {
    pub keybind: Keybind,
    pub description: String,
}

/// A deck of cards loaded from a TSV file
#[derive(Debug, Clone)]
pub struct Deck {
    pub name: String,
    pub cards: Vec<Card>,
    pub keyboard_mode: KeyboardMode,
}

impl Deck {
    /// Load a deck from a TSV file
    /// Format: keybind<TAB>description
    /// Lines starting with # are comments (or directives like `# mode: chars`)
    /// Empty lines are skipped
    pub fn load(path: &Path) -> Result<Self> {
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read deck file: {}", path.display()))?;

        let mut cards = Vec::new();
        let mut keyboard_mode = KeyboardMode::default();

        for (line_num, line) in content.lines().enumerate() {
            let line = line.trim();

            // Skip empty lines
            if line.is_empty() {
                continue;
            }

            // Handle comments and directives
            if line.starts_with('#') {
                // Check for mode directive: `# mode: raw` or `# mode: chars`
                if let Some(rest) = line.strip_prefix('#') {
                    let rest = rest.trim();
                    if let Some(mode_value) = rest.strip_prefix("mode:") {
                        match mode_value.trim().to_lowercase().as_str() {
                            "raw" => keyboard_mode = KeyboardMode::Raw,
                            "chars" | "char" | "characters" => keyboard_mode = KeyboardMode::Chars,
                            "command" | "commands" => keyboard_mode = KeyboardMode::Command,
                            other => anyhow::bail!(
                                "Unknown keyboard mode '{}' on line {} in {}. Use 'raw', 'chars', or 'commands'.",
                                other,
                                line_num + 1,
                                path.display()
                            ),
                        }
                    }
                }
                continue;
            }

            let parts: Vec<&str> = line.splitn(2, '\t').collect();
            if parts.len() != 2 {
                anyhow::bail!(
                    "Invalid line {} in {}: expected keybind<TAB>description",
                    line_num + 1,
                    path.display()
                );
            }

            let keybind = if keyboard_mode == KeyboardMode::Command {
                Keybind::parse_command(parts[0])
            } else {
                Keybind::parse(parts[0])
            }
            .with_context(|| {
                format!(
                    "Failed to parse keybind on line {} in {}",
                    line_num + 1,
                    path.display()
                )
            })?;

            cards.push(Card {
                keybind,
                description: parts[1].to_string(),
            });
        }

        Ok(Deck {
            name,
            cards,
            keyboard_mode,
        })
    }
}

/// List available deck files in a directory
pub fn list_decks(dir: &Path) -> Result<Vec<PathBuf>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut decks = Vec::new();

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() && path.extension().is_some_and(|ext| ext == "tsv") {
            decks.push(path);
        }
    }

    decks.sort();
    Ok(decks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_load_deck() {
        let mut file = NamedTempFile::with_suffix(".tsv").unwrap();
        writeln!(file, "Ctrl+S\tSave file").unwrap();
        writeln!(file, "g g\tGo to top").unwrap();
        writeln!(file, "# This is a comment").unwrap();
        writeln!(file, "").unwrap();
        writeln!(file, "Ctrl+K Ctrl+C\tComment selection").unwrap();

        let deck = Deck::load(file.path()).unwrap();
        assert_eq!(deck.cards.len(), 3);
        assert_eq!(deck.cards[0].description, "Save file");
        assert_eq!(deck.cards[1].keybind.len(), 2);
        assert_eq!(deck.cards[2].keybind.len(), 2);
    }
}
