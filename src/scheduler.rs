use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use fsrs::{DEFAULT_PARAMETERS, FSRS, MemoryState, NextStates};

/// Rating derived from response time and attempts
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rating {
    /// Timeout or 3+ attempts - forgot
    Again = 1,
    /// >= 5s or 2 attempts - hard recall
    Hard = 2,
    /// < 5s, 1 attempt - good recall
    Good = 3,
    /// < 2s, 1 attempt - easy recall
    Easy = 4,
}

impl Rating {
    /// Rate based on response speed and attempt count.
    /// Thresholds should already be scaled for chord count by the caller.
    pub fn from_speed(
        response_time_ms: u64,
        attempts: u8,
        easy_threshold_ms: u64,
        hard_threshold_ms: u64,
        max_attempts: u8,
    ) -> Self {
        if attempts >= max_attempts {
            return Rating::Again;
        }

        if attempts == 2 || response_time_ms >= hard_threshold_ms {
            return Rating::Hard;
        }

        if response_time_ms < easy_threshold_ms && attempts == 1 {
            return Rating::Easy;
        }

        Rating::Good
    }

    /// Scale a base threshold for multi-chord sequences: +20% per additional chord.
    pub fn scale_threshold(base_ms: u64, num_chords: usize) -> u64 {
        let multiplier = 1.0 + 0.2 * num_chords.saturating_sub(1) as f64;
        (base_ms as f64 * multiplier) as u64
    }

    /// Convert to FSRS rating (1-4)
    pub fn as_u32(&self) -> u32 {
        *self as u32
    }
}

/// Scheduler wrapping FSRS
pub struct Scheduler {
    fsrs: FSRS,
    desired_retention: f32,
    interval_modifier: f32,
    max_interval_days: f32,
}

impl Scheduler {
    /// Create a new scheduler with desired retention rate (0.0 - 1.0)
    pub fn new(
        desired_retention: f32,
        interval_modifier: f32,
        max_interval_days: f32,
    ) -> Result<Self> {
        Ok(Self {
            fsrs: FSRS::new(Some(&DEFAULT_PARAMETERS))?,
            desired_retention,
            interval_modifier,
            max_interval_days,
        })
    }

    /// Get next states for a card
    /// Returns NextStates for scheduling
    pub fn get_next_states(
        &self,
        memory_state: Option<MemoryState>,
        last_review: Option<DateTime<Utc>>,
    ) -> Result<NextStates> {
        let elapsed_days: u32 = match last_review {
            Some(last) => {
                let duration = Utc::now().signed_duration_since(last);
                duration.num_days().max(0) as u32
            }
            None => 0,
        };

        let next_states =
            self.fsrs
                .next_states(memory_state, self.desired_retention, elapsed_days)?;

        Ok(next_states)
    }

    /// Schedule a card based on rating
    /// Returns (new_memory_state, due_date)
    pub fn schedule(
        &self,
        memory_state: Option<MemoryState>,
        last_review: Option<DateTime<Utc>>,
        rating: Rating,
    ) -> Result<(MemoryState, DateTime<Utc>)> {
        let next_states = self.get_next_states(memory_state, last_review)?;

        let item_state = match rating {
            Rating::Again => &next_states.again,
            Rating::Hard => &next_states.hard,
            Rating::Good => &next_states.good,
            Rating::Easy => &next_states.easy,
        };

        // Calculate due date from interval, applying modifier and cap
        let interval_days =
            (item_state.interval * self.interval_modifier).min(self.max_interval_days);
        let due_date =
            Utc::now() + Duration::seconds((interval_days * 86400.0) as i64) + Duration::hours(1);

        Ok((item_state.memory, due_date))
    }

    /// Create memory state from stored values
    pub fn memory_state_from_stored(stability: f32, difficulty: f32) -> MemoryState {
        MemoryState {
            stability,
            difficulty,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EASY: u64 = 2000;
    const HARD: u64 = 5000;
    const MAX: u8 = 3;

    #[test]
    fn test_rating_easy() {
        let rating = Rating::from_speed(1500, 1, EASY, HARD, MAX);
        assert_eq!(rating, Rating::Easy);
    }

    #[test]
    fn test_rating_good() {
        let rating = Rating::from_speed(3000, 1, EASY, HARD, MAX);
        assert_eq!(rating, Rating::Good);
    }

    #[test]
    fn test_rating_hard_time() {
        let rating = Rating::from_speed(6000, 1, EASY, HARD, MAX);
        assert_eq!(rating, Rating::Hard);
    }

    #[test]
    fn test_rating_hard_attempts() {
        let rating = Rating::from_speed(1000, 2, EASY, HARD, MAX);
        assert_eq!(rating, Rating::Hard);
    }

    #[test]
    fn test_rating_hard_at_threshold() {
        let rating = Rating::from_speed(5000, 1, EASY, HARD, MAX);
        assert_eq!(rating, Rating::Hard);
    }

    #[test]
    fn test_rating_again_attempts() {
        let rating = Rating::from_speed(1000, 3, EASY, HARD, MAX);
        assert_eq!(rating, Rating::Again);
    }

    #[test]
    fn test_rating_good_at_easy_boundary() {
        let rating = Rating::from_speed(2000, 1, EASY, HARD, MAX);
        assert_eq!(rating, Rating::Good);
    }

    #[test]
    fn test_scale_threshold() {
        assert_eq!(Rating::scale_threshold(2000, 1), 2000);
        assert_eq!(Rating::scale_threshold(2000, 2), 2400);
        assert_eq!(Rating::scale_threshold(2000, 3), 2800);
        assert_eq!(Rating::scale_threshold(2000, 5), 3600);
        assert_eq!(Rating::scale_threshold(5000, 3), 7000);
    }

    #[test]
    fn test_scaled_thresholds_in_rating() {
        let easy_3 = Rating::scale_threshold(EASY, 3);
        let hard_3 = Rating::scale_threshold(HARD, 3);

        let rating = Rating::from_speed(2700, 1, easy_3, hard_3, MAX);
        assert_eq!(rating, Rating::Easy);

        let rating = Rating::from_speed(2900, 1, easy_3, hard_3, MAX);
        assert_eq!(rating, Rating::Good);

        let rating = Rating::from_speed(6900, 1, easy_3, hard_3, MAX);
        assert_eq!(rating, Rating::Good);

        let rating = Rating::from_speed(7100, 1, easy_3, hard_3, MAX);
        assert_eq!(rating, Rating::Hard);
    }

    #[test]
    fn test_schedule_new_card() {
        let scheduler = Scheduler::new(0.9, 0.12, 30.0).unwrap();
        let (memory, due) = scheduler.schedule(None, None, Rating::Good).unwrap();

        assert!(memory.stability > 0.0);
        assert!(due > Utc::now());
    }
}
