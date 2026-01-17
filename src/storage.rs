use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{Connection, params};
use std::path::Path;

/// Stored card state in the database
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct StoredCard {
    pub id: i64,
    pub deck: String,
    pub keybind: String,
    pub description: String,
    pub stability: Option<f32>,
    pub difficulty: Option<f32>,
    pub due_date: Option<DateTime<Utc>>,
    pub last_review: Option<DateTime<Utc>>,
    pub review_count: i32,
}

/// A review record (for FSRS parameter training)
#[derive(Debug, Clone)]
#[allow(dead_code)]
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
            ",
        )?;

        Ok(())
    }

    /// Upsert a card (insert or update if exists)
    pub fn upsert_card(&self, deck: &str, keybind: &str, description: &str) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO cards (deck, keybind, description)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(deck, keybind) DO UPDATE SET description = ?3",
            params![deck, keybind, description],
        )?;

        let id = self.conn.query_row(
            "SELECT id FROM cards WHERE deck = ?1 AND keybind = ?2",
            params![deck, keybind],
            |row| row.get(0),
        )?;

        Ok(id)
    }

    /// Get a card by ID
    #[allow(dead_code)]
    pub fn get_card(&self, id: i64) -> Result<Option<StoredCard>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, deck, keybind, description, stability, difficulty,
                    due_date, last_review, review_count
             FROM cards WHERE id = ?1",
        )?;

        let card = stmt.query_row(params![id], |row| {
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
            })
        });

        match card {
            Ok(c) => Ok(Some(c)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Get all cards for a deck
    #[allow(dead_code)]
    pub fn get_cards_for_deck(&self, deck: &str) -> Result<Vec<StoredCard>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, deck, keybind, description, stability, difficulty,
                    due_date, last_review, review_count
             FROM cards WHERE deck = ?1",
        )?;

        let cards = stmt
            .query_map(params![deck], |row| {
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
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(cards)
    }

    /// Get due cards for a deck (due_date <= now or never reviewed)
    pub fn get_due_cards(&self, deck: &str) -> Result<Vec<StoredCard>> {
        let now = Utc::now().to_rfc3339();

        let mut stmt = self.conn.prepare(
            "SELECT id, deck, keybind, description, stability, difficulty,
                    due_date, last_review, review_count
             FROM cards
             WHERE deck = ?1 AND (due_date IS NULL OR due_date <= ?2)
             ORDER BY due_date ASC NULLS FIRST",
        )?;

        let cards = stmt
            .query_map(params![deck, now], |row| {
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
                })
            })?
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

    /// Get all decks with card counts
    pub fn get_deck_stats(&self) -> Result<Vec<(String, i32, i32)>> {
        let mut stmt = self.conn.prepare(
            "SELECT deck, COUNT(*), SUM(CASE WHEN due_date IS NULL OR due_date <= ?1 THEN 1 ELSE 0 END)
             FROM cards GROUP BY deck ORDER BY deck",
        )?;

        let now = Utc::now().to_rfc3339();

        let stats = stmt
            .query_map(params![now], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(stats)
    }

    /// Get reviews for a card (for FSRS training)
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
                    reviewed_at: row.get::<_, String>(5)?.parse().unwrap(),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(reviews)
    }
}
