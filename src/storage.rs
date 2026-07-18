use anyhow::{Context, Result};
use chrono::{DateTime, Local, Utc};
use rusqlite::{Connection, params};
use std::collections::HashSet;
use std::path::Path;

/// Stored card state in the database
#[derive(Debug, Clone)]
pub struct StoredCard {
    pub id: i64,
    pub deck: String,
    pub keybind: String,
    pub description: String,
    pub stability: Option<f32>,
    pub difficulty: Option<f32>,
    pub last_review: Option<DateTime<Utc>>,
}

use crate::deck::KeyboardMode;

/// Stats about a deck
#[derive(Debug, Clone)]
pub struct DeckStats {
    pub name: String,
    pub total_cards: i32,
    pub due_cards: i32,
    pub keyboard_mode: KeyboardMode,
}

fn row_to_stored_card(row: &rusqlite::Row) -> rusqlite::Result<StoredCard> {
    Ok(StoredCard {
        id: row.get(0)?,
        deck: row.get(1)?,
        keybind: row.get(2)?,
        description: row.get(3)?,
        stability: row.get(4)?,
        difficulty: row.get(5)?,
        last_review: row
            .get::<_, Option<String>>(6)?
            .and_then(|s| s.parse().ok()),
    })
}

pub struct Storage {
    conn: Connection,
}

pub struct DeckSyncInput {
    pub deck_name: String,
    pub keybinds: Vec<(String, String)>,
}

impl Storage {
    /// Open or create the database
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)
            .with_context(|| format!("Failed to open database: {}", path.display()))?;

        conn.pragma_update(None, "foreign_keys", "ON")?;

        let storage = Storage { conn };
        storage.init_schema()?;

        Ok(storage)
    }

    /// Initialize database schema
    fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS cards (
                id INTEGER PRIMARY KEY,
                deck TEXT NOT NULL,
                keybind TEXT NOT NULL,
                description TEXT NOT NULL,
                stability REAL,
                difficulty REAL,
                due_date TEXT,
                last_review TEXT,
                review_count INTEGER DEFAULT 0,
                UNIQUE(deck, keybind)
            );

            CREATE TABLE IF NOT EXISTS reviews (
                id INTEGER PRIMARY KEY,
                card_id INTEGER NOT NULL,
                rating INTEGER NOT NULL,
                response_time_ms INTEGER,
                attempts INTEGER,
                reviewed_at TEXT NOT NULL,
                FOREIGN KEY (card_id) REFERENCES cards(id)
            );

            CREATE INDEX IF NOT EXISTS idx_cards_deck ON cards(deck);
            CREATE INDEX IF NOT EXISTS idx_cards_due ON cards(due_date);
            CREATE INDEX IF NOT EXISTS idx_reviews_card ON reviews(card_id);

            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            ",
        )?;

        Ok(())
    }

    /// Sync all decks in a single transaction: upsert cards, delete removed cards, delete orphaned decks.
    pub fn sync_decks(
        &mut self,
        decks: Vec<DeckSyncInput>,
        active_deck_names: &HashSet<String>,
    ) -> Result<()> {
        let tx = self.conn.transaction()?;

        for deck in &decks {
            let mut deck_keybinds = HashSet::new();

            for (keybind, description) in &deck.keybinds {
                deck_keybinds.insert(keybind.clone());
                tx.execute(
                    "INSERT INTO cards (deck, keybind, description)
                     VALUES (?1, ?2, ?3)
                     ON CONFLICT(deck, keybind) DO UPDATE SET
                        description = ?3,
                        stability = CASE WHEN description != ?3 THEN NULL ELSE stability END,
                        difficulty = CASE WHEN description != ?3 THEN NULL ELSE difficulty END,
                        due_date = CASE WHEN description != ?3 THEN NULL ELSE due_date END,
                        last_review = CASE WHEN description != ?3 THEN NULL ELSE last_review END,
                        review_count = CASE WHEN description != ?3 THEN 0 ELSE review_count END",
                    params![deck.deck_name, keybind, description],
                )?;
            }

            let mut stmt = tx.prepare("SELECT keybind FROM cards WHERE deck = ?1")?;
            let existing: HashSet<String> = stmt
                .query_map(params![deck.deck_name], |row| row.get(0))?
                .collect::<Result<HashSet<_>, _>>()?;
            drop(stmt);

            for keybind in existing.difference(&deck_keybinds) {
                tx.execute(
                    "DELETE FROM reviews WHERE card_id IN (SELECT id FROM cards WHERE deck = ?1 AND keybind = ?2)",
                    params![deck.deck_name, keybind],
                )?;
                tx.execute(
                    "DELETE FROM cards WHERE deck = ?1 AND keybind = ?2",
                    params![deck.deck_name, keybind],
                )?;
            }
        }

        let mut stmt = tx.prepare("SELECT DISTINCT deck FROM cards")?;
        let db_decks: HashSet<String> = stmt
            .query_map([], |row| row.get(0))?
            .collect::<Result<HashSet<_>, _>>()?;
        drop(stmt);

        for deck in db_decks.difference(active_deck_names) {
            tx.execute(
                "DELETE FROM reviews WHERE card_id IN (SELECT id FROM cards WHERE deck = ?1)",
                params![deck],
            )?;
            tx.execute("DELETE FROM cards WHERE deck = ?1", params![deck])?;
        }

        tx.commit()?;
        Ok(())
    }

    /// Get due cards for a deck (due now or never reviewed)
    pub fn get_due_cards(&self, deck: &str) -> Result<Vec<StoredCard>> {
        let now = Utc::now().to_rfc3339();

        let mut stmt = self.conn.prepare(
            "SELECT id, deck, keybind, description, stability, difficulty, last_review
             FROM cards
             WHERE deck = ?1 AND (due_date IS NULL OR due_date <= ?2)
             ORDER BY due_date ASC NULLS FIRST",
        )?;

        let cards = stmt
            .query_map(params![deck, now], row_to_stored_card)?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(cards)
    }

    /// Update card after review
    pub fn update_card_after_review(
        &self,
        id: i64,
        stability: f32,
        difficulty: f32,
        due_date: DateTime<Utc>,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let due = due_date.to_rfc3339();

        self.conn.execute(
            "UPDATE cards SET
                stability = ?1,
                difficulty = ?2,
                due_date = ?3,
                last_review = ?4,
                review_count = review_count + 1
             WHERE id = ?5",
            params![stability, difficulty, due, now, id],
        )?;

        Ok(())
    }

    /// Record a review
    pub fn record_review(
        &self,
        card_id: i64,
        rating: i32,
        response_time_ms: i64,
        attempts: i32,
    ) -> Result<i64> {
        let now = Utc::now().to_rfc3339();

        self.conn.execute(
            "INSERT INTO reviews (card_id, rating, response_time_ms, attempts, reviewed_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![card_id, rating, response_time_ms, attempts, now],
        )?;

        Ok(self.conn.last_insert_rowid())
    }

    /// Get all decks with card counts (due = due now)
    /// keyboard_modes maps deck name to its KeyboardMode (from TSV files)
    pub fn get_deck_stats(
        &self,
        keyboard_modes: &std::collections::HashMap<String, KeyboardMode>,
    ) -> Result<Vec<DeckStats>> {
        let mut stmt = self.conn.prepare(
            "SELECT deck, COUNT(*), SUM(CASE WHEN due_date IS NULL OR due_date <= ?1 THEN 1 ELSE 0 END)
             FROM cards GROUP BY deck ORDER BY deck",
        )?;

        let now = Utc::now().to_rfc3339();

        let stats = stmt
            .query_map(params![now], |row| {
                let name: String = row.get(0)?;
                let keyboard_mode = keyboard_modes.get(&name).copied().unwrap_or_default();
                Ok(DeckStats {
                    name,
                    total_cards: row.get(1)?,
                    due_cards: row.get(2)?,
                    keyboard_mode,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(stats)
    }

    /// Get a setting value by key
    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let result = self.conn.query_row(
            "SELECT value FROM settings WHERE key = ?1",
            params![key],
            |row| row.get(0),
        );
        match result {
            Ok(value) => Ok(Some(value)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Set a setting value
    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = ?2",
            params![key, value],
        )?;
        Ok(())
    }

    /// Create a daily backup of the database if one doesn't exist for today.
    /// Backups are stored in the same directory as the database with format: kbsr.db.backup.YYYY-MM-DD
    pub fn create_daily_backup(db_path: &Path) -> Result<Option<std::path::PathBuf>> {
        if !db_path.exists() {
            return Ok(None);
        }

        let today = Local::now().format("%Y-%m-%d").to_string();
        let backup_name = format!(
            "{}.backup.{}",
            db_path.file_name().unwrap_or_default().to_string_lossy(),
            today
        );
        let backup_path = db_path.with_file_name(backup_name);

        if backup_path.exists() {
            return Ok(None);
        }

        std::fs::copy(db_path, &backup_path)
            .with_context(|| format!("Failed to create backup at {}", backup_path.display()))?;

        Ok(Some(backup_path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    struct CardRow {
        id: i64,
        stability: Option<f32>,
        difficulty: Option<f32>,
        due_date: Option<String>,
        last_review: Option<String>,
        review_count: i32,
    }

    fn fetch(storage: &Storage, deck: &str, keybind: &str) -> Option<CardRow> {
        storage
            .conn
            .query_row(
                "SELECT id, stability, difficulty, due_date, last_review, review_count
                 FROM cards WHERE deck = ?1 AND keybind = ?2",
                params![deck, keybind],
                |row| {
                    Ok(CardRow {
                        id: row.get(0)?,
                        stability: row.get(1)?,
                        difficulty: row.get(2)?,
                        due_date: row.get(3)?,
                        last_review: row.get(4)?,
                        review_count: row.get(5)?,
                    })
                },
            )
            .ok()
    }

    fn count_reviews(storage: &Storage, card_id: i64) -> i64 {
        storage
            .conn
            .query_row(
                "SELECT COUNT(*) FROM reviews WHERE card_id = ?1",
                params![card_id],
                |row| row.get(0),
            )
            .unwrap()
    }

    fn new_storage() -> (Storage, TempDir) {
        let dir = TempDir::new().unwrap();
        let storage = Storage::open(&dir.path().join("test.db")).unwrap();
        (storage, dir)
    }

    fn sync_one(storage: &mut Storage, deck: &str, cards: &[(&str, &str)]) {
        let mut active = HashSet::new();
        active.insert(deck.to_string());
        let input = DeckSyncInput {
            deck_name: deck.to_string(),
            keybinds: cards
                .iter()
                .map(|(k, d)| (k.to_string(), d.to_string()))
                .collect(),
        };
        storage.sync_decks(vec![input], &active).unwrap();
    }

    #[test]
    fn sync_decks_inserts_new_cards() {
        let (mut storage, _dir) = new_storage();
        sync_one(&mut storage, "vim", &[("g g", "Top"), ("G", "Bottom")]);

        assert!(fetch(&storage, "vim", "g g").is_some());
        assert!(fetch(&storage, "vim", "G").is_some());
    }

    #[test]
    fn sync_decks_preserves_progress_when_description_unchanged() {
        let (mut storage, _dir) = new_storage();
        sync_one(&mut storage, "vim", &[("g g", "Top")]);

        let row = fetch(&storage, "vim", "g g").unwrap();
        storage
            .update_card_after_review(row.id, 4.2, 6.5, Utc::now() + chrono::Duration::days(5))
            .unwrap();

        sync_one(&mut storage, "vim", &[("g g", "Top")]);

        let after = fetch(&storage, "vim", "g g").unwrap();
        assert_eq!(after.stability, Some(4.2));
        assert_eq!(after.difficulty, Some(6.5));
        assert!(after.due_date.is_some());
        assert!(after.last_review.is_some());
        assert_eq!(after.review_count, 1);
    }

    #[test]
    fn sync_decks_resets_progress_when_description_changes() {
        let (mut storage, _dir) = new_storage();
        sync_one(&mut storage, "vim", &[("g g", "Top")]);

        let row = fetch(&storage, "vim", "g g").unwrap();
        storage
            .update_card_after_review(row.id, 4.2, 6.5, Utc::now() + chrono::Duration::days(5))
            .unwrap();

        sync_one(&mut storage, "vim", &[("g g", "Go to top of file")]);

        let after = fetch(&storage, "vim", "g g").unwrap();
        assert!(after.stability.is_none());
        assert!(after.difficulty.is_none());
        assert!(after.due_date.is_none());
        assert!(after.last_review.is_none());
        assert_eq!(after.review_count, 0);
    }

    #[test]
    fn sync_decks_removes_dropped_keybinds_and_their_reviews() {
        let (mut storage, _dir) = new_storage();
        sync_one(&mut storage, "vim", &[("g g", "Top"), ("G", "Bottom")]);

        let g = fetch(&storage, "vim", "G").unwrap();
        storage.record_review(g.id, 3, 1500, 1).unwrap();
        assert_eq!(count_reviews(&storage, g.id), 1);

        sync_one(&mut storage, "vim", &[("g g", "Top")]);

        assert!(fetch(&storage, "vim", "G").is_none());
        // Deleting the card must also delete its reviews — verified by querying
        // the previous card_id directly since the row may already be gone.
        let orphans: i64 = storage
            .conn
            .query_row(
                "SELECT COUNT(*) FROM reviews WHERE card_id = ?1",
                params![g.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(orphans, 0);
    }

    #[test]
    fn sync_decks_removes_orphaned_decks() {
        let (mut storage, _dir) = new_storage();
        sync_one(&mut storage, "vim", &[("g g", "Top")]);
        sync_one(&mut storage, "vscode", &[("Ctrl+S", "Save")]);

        // Now sync with only vim present in the active set.
        let mut active = HashSet::new();
        active.insert("vim".to_string());
        storage
            .sync_decks(
                vec![DeckSyncInput {
                    deck_name: "vim".to_string(),
                    keybinds: vec![("g g".to_string(), "Top".to_string())],
                }],
                &active,
            )
            .unwrap();

        assert!(fetch(&storage, "vim", "g g").is_some());
        assert!(fetch(&storage, "vscode", "Ctrl+S").is_none());
    }
}
