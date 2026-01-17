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
    /// Derive rating from response time and number of attempts
    /// This is called when the user got the answer correct
    pub fn from_performance(response_time_ms: u64, attempts: u8, _timeout_secs: u64) -> Self {
        // Too many attempts = Again (shouldn't normally happen since we reveal at 3)
        if attempts >= 3 {
            return Rating::Again;
        }

        // 2 attempts or slow response (>= 5s) = Hard
        if attempts == 2 || response_time_ms >= 5000 {
            return Rating::Hard;
        }

        // Fast response with 1 attempt = Easy
        if response_time_ms < 2000 && attempts == 1 {
            return Rating::Easy;
        }

        // Otherwise = Good
        Rating::Good
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
}

impl Scheduler {
    /// Create a new scheduler with desired retention rate (0.0 - 1.0)
    pub fn new(desired_retention: f32) -> Result<Self> {
        Ok(Self {
            fsrs: FSRS::new(Some(&DEFAULT_PARAMETERS))?,
            desired_retention,
        })
    }

    /// Create with default 0.9 retention rate
    pub fn with_default_retention() -> Result<Self> {
        Self::new(0.9)
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

        let next_states = self
            .fsrs
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

        // Calculate due date from interval (in days)
        let interval_days = item_state.interval.max(1.0) as i64;
        let due_date = Utc::now() + Duration::days(interval_days);

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

    #[test]
    fn test_rating_easy() {
        let rating = Rating::from_performance(1500, 1, 5);
        assert_eq!(rating, Rating::Easy);
    }

    #[test]
    fn test_rating_good() {
        let rating = Rating::from_performance(3000, 1, 5);
        assert_eq!(rating, Rating::Good);
    }

    #[test]
    fn test_rating_hard_time() {
        let rating = Rating::from_performance(6000, 1, 5);
        assert_eq!(rating, Rating::Hard);
    }

    #[test]
    fn test_rating_hard_attempts() {
        let rating = Rating::from_performance(2000, 2, 5);
        assert_eq!(rating, Rating::Hard);
    }

    #[test]
    fn test_rating_hard_at_timeout() {
        // At exactly the timeout threshold, but user still got it correct = Hard
        let rating = Rating::from_performance(5000, 1, 5);
        assert_eq!(rating, Rating::Hard);
    }

    #[test]
    fn test_rating_again_attempts() {
        let rating = Rating::from_performance(2000, 3, 5);
        assert_eq!(rating, Rating::Again);
    }

    #[test]
    fn test_schedule_new_card() {
        let scheduler = Scheduler::with_default_retention().unwrap();
        let (memory, due) = scheduler.schedule(None, None, Rating::Good).unwrap();

        assert!(memory.stability > 0.0);
        assert!(due > Utc::now());
    }
}
