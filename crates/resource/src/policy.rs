use std::time::Duration;

/// Retry policy for resource fetches and mutations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetryPolicy {
    max_attempts: u32,
    base_delay: Duration,
}

impl RetryPolicy {
    /// Creates a retry policy.
    pub fn new(max_attempts: u32, base_delay: Duration) -> Self {
        Self {
            max_attempts,
            base_delay,
        }
    }

    /// Returns true when another attempt should run after `attempts_completed`.
    pub fn should_retry(&self, attempts_completed: u32) -> bool {
        attempts_completed < self.max_attempts
    }

    /// Returns the next delay for `attempts_completed`.
    pub fn next_delay(&self, attempts_completed: u32) -> Option<Duration> {
        self.should_retry(attempts_completed)
            .then(|| self.base_delay.saturating_mul(attempts_completed.max(1)))
    }
}
