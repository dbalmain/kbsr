use crate::keybind::Keybind;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

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
}

impl Deck {
    /// Load a deck from a TSV file
    /// Format: keybind<TAB>description
    /// Lines starting with # are comments
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

        for (line_num, line) in content.lines().enumerate() {
            let line = line.trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') {
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

            let keybind = Keybind::parse(parts[0]).with_context(|| {
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

        Ok(Deck { name, cards })
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
