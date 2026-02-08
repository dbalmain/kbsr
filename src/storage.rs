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
    #[allow(dead_code)] // Used in DB queries, will be used for UI stats
    pub due_date: Option<DateTime<Utc>>,
    pub last_review: Option<DateTime<Utc>>,
    #[allow(dead_code)] // Used in DB, will be used for stats display
    pub review_count: i32,
    /// Number of times card was presented before getting it right first try
    pub current_presentation_count: i32,
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
        due_date: row.get::<_, Option<String>>(6)?.and_then(|s| s.parse().ok()),
        last_review: row.get::<_, Option<String>>(7)?.and_then(|s| s.parse().ok()),
        review_count: row.get(8)?,
        current_presentation_count: row.get(9)?,
    })
}

fn end_of_today_utc() -> String {
    let today = Local::now().date_naive();
    let end_of_today = today
        .succ_opt()
        .unwrap_or(today)
        .and_hms_opt(0, 0, 0)
        .unwrap();
    end_of_today
        .and_local_timezone(Local)
        .unwrap()
        .with_timezone(&Utc)
        .to_rfc3339()
}

/// A review record (for FSRS parameter training)
#[derive(Debug, Clone)]
#[allow(dead_code)] // Struct used for future FSRS parameter optimization
pub struct Review {
    pub id: i64,
    pub card_id: i64,
    pub rating: i32,
    pub response_time_ms: i64,
    pub attempts: i32,
    pub reviewed_at: DateTime<Utc>,
}

pub struct Storage {
    conn: Connection,
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
                current_presentation_count INTEGER DEFAULT 0,
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
            ",
        )?;

        Ok(())
    }

    /// Upsert a card (insert or update if exists)
    /// Resets progress if description changes
    pub fn upsert_card(&self, deck: &str, keybind: &str, description: &str) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO cards (deck, keybind, description)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(deck, keybind) DO UPDATE SET
                description = ?3,
                stability = CASE WHEN description != ?3 THEN NULL ELSE stability END,
                difficulty = CASE WHEN description != ?3 THEN NULL ELSE difficulty END,
                due_date = CASE WHEN description != ?3 THEN NULL ELSE due_date END,
                last_review = CASE WHEN description != ?3 THEN NULL ELSE last_review END,
                review_count = CASE WHEN description != ?3 THEN 0 ELSE review_count END,
                current_presentation_count = CASE WHEN description != ?3 THEN 0 ELSE current_presentation_count END",
            params![deck, keybind, description],
        )?;

        let id = self.conn.query_row(
            "SELECT id FROM cards WHERE deck = ?1 AND keybind = ?2",
            params![deck, keybind],
            |row| row.get(0),
        )?;

        Ok(id)
    }

    /// Get due cards for a deck (due by end of today in local timezone, or never reviewed)
    pub fn get_due_cards(&self, deck: &str) -> Result<Vec<StoredCard>> {
        let end_of_today_utc = end_of_today_utc();

        let mut stmt = self.conn.prepare(
            "SELECT id, deck, keybind, description, stability, difficulty,
                    due_date, last_review, review_count, current_presentation_count
             FROM cards
             WHERE deck = ?1 AND (due_date IS NULL OR due_date <= ?2)
             ORDER BY due_date ASC NULLS FIRST",
        )?;

        let cards = stmt
            .query_map(params![deck, end_of_today_utc], row_to_stored_card)?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(cards)
    }

    /// Update card after review (resets presentation count since they got it right)
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
                review_count = review_count + 1,
                current_presentation_count = 0
             WHERE id = ?5",
            params![stability, difficulty, due, now, id],
        )?;

        Ok(())
    }

    /// Increment presentation count for a card (called when card is shown but not scored)
    pub fn increment_presentation_count(&self, id: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE cards SET current_presentation_count = current_presentation_count + 1 WHERE id = ?1",
            params![id],
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

    /// Get all decks with card counts (due = due by end of today)
    /// keyboard_modes maps deck name to its KeyboardMode (from TSV files)
    pub fn get_deck_stats(&self, keyboard_modes: &std::collections::HashMap<String, KeyboardMode>) -> Result<Vec<DeckStats>> {
        let mut stmt = self.conn.prepare(
            "SELECT deck, COUNT(*), SUM(CASE WHEN due_date IS NULL OR due_date <= ?1 THEN 1 ELSE 0 END)
             FROM cards GROUP BY deck ORDER BY deck",
        )?;

        let end_of_today_utc = end_of_today_utc();

        let stats = stmt
            .query_map(params![end_of_today_utc], |row| {
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

    /// Get reviews for a card.
    /// Reserved for future FSRS parameter training from user review history.
    #[allow(dead_code)]
    pub fn get_reviews_for_card(&self, card_id: i64) -> Result<Vec<Review>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, card_id, rating, response_time_ms, attempts, reviewed_at
             FROM reviews WHERE card_id = ?1 ORDER BY reviewed_at ASC",
        )?;

        let reviews = stmt
            .query_map(params![card_id], |row| {
                Ok(Review {
                    id: row.get(0)?,
                    card_id: row.get(1)?,
                    rating: row.get(2)?,
                    response_time_ms: row.get(3)?,
                    attempts: row.get(4)?,
                    reviewed_at: row.get::<_, String>(5)?
                        .parse()
                        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                            5, rusqlite::types::Type::Text, Box::new(e),
                        ))?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(reviews)
    }

    /// Get all keybinds for a deck
    pub fn get_deck_keybinds(&self, deck: &str) -> Result<HashSet<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT keybind FROM cards WHERE deck = ?1")?;

        let keybinds = stmt
            .query_map(params![deck], |row| row.get(0))?
            .collect::<Result<HashSet<String>, _>>()?;

        Ok(keybinds)
    }

    /// Delete cards from a deck that are not in the given set of keybinds.
    /// Returns the number of cards deleted.
    pub fn delete_removed_cards(&self, deck: &str, keep_keybinds: &HashSet<String>) -> Result<usize> {
        let existing = self.get_deck_keybinds(deck)?;
        let to_delete: Vec<_> = existing.difference(keep_keybinds).collect();

        if to_delete.is_empty() {
            return Ok(0);
        }

        let mut deleted = 0;
        for keybind in &to_delete {
            self.conn.execute(
                "DELETE FROM reviews WHERE card_id IN (SELECT id FROM cards WHERE deck = ?1 AND keybind = ?2)",
                params![deck, keybind],
            )?;
            deleted += self.conn.execute(
                "DELETE FROM cards WHERE deck = ?1 AND keybind = ?2",
                params![deck, keybind],
            )?;
        }

        Ok(deleted)
    }

    /// Delete decks that are no longer present in the filesystem.
    /// Returns the names of deleted decks.
    pub fn delete_orphaned_decks(&self, active_decks: &HashSet<String>) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare("SELECT DISTINCT deck FROM cards")?;
        let db_decks: HashSet<String> = stmt
            .query_map([], |row| row.get(0))?
            .collect::<Result<HashSet<_>, _>>()?;

        let orphaned: Vec<String> = db_decks.difference(active_decks).cloned().collect();

        for deck in &orphaned {
            self.conn.execute(
                "DELETE FROM reviews WHERE card_id IN (SELECT id FROM cards WHERE deck = ?1)",
                params![deck],
            )?;
            self.conn
                .execute("DELETE FROM cards WHERE deck = ?1", params![deck])?;
        }

        Ok(orphaned)
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
