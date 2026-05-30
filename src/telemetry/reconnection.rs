//! Reconnection logic for telemetry exporters.
//!
//! Implements exponential backoff with jitter and circuit breaker pattern
//! for resilient reconnection to telemetry endpoints.

use std::time::Duration;

/// Configuration for reconnection behavior
#[derive(Debug, Clone)]
pub struct ReconnectionConfig {
    /// Initial backoff duration
    pub initial_backoff: Duration,
    /// Maximum backoff duration
    pub max_backoff: Duration,
    /// Backoff multiplier for exponential growth
    pub backoff_multiplier: f64,
    /// Maximum number of consecutive failures before circuit opens
    pub max_failures: u32,
    /// Duration to keep circuit open before attempting reset
    pub circuit_open_duration: Duration,
}

impl Default for ReconnectionConfig {
    fn default() -> Self {
        Self {
            initial_backoff: Duration::from_millis(100),
            max_backoff: Duration::from_secs(30),
            backoff_multiplier: 2.0,
            max_failures: 5,
            circuit_open_duration: Duration::from_secs(60),
        }
    }
}

/// Manages reconnection attempts with exponential backoff
#[derive(Debug, Clone)]
pub struct ReconnectionManager {
    config: ReconnectionConfig,
    consecutive_failures: u32,
    circuit_open: bool,
    last_failure_time: Option<std::time::Instant>,
}

impl ReconnectionManager {
    /// Creates a new reconnection manager with default configuration
    pub fn new() -> Self {
        Self::with_config(ReconnectionConfig::default())
    }

    /// Creates a new reconnection manager with custom configuration
    pub fn with_config(config: ReconnectionConfig) -> Self {
        Self {
            config,
            consecutive_failures: 0,
            circuit_open: false,
            last_failure_time: None,
        }
    }

    /// Records a successful connection
    pub fn record_success(&mut self) {
        self.consecutive_failures = 0;
        self.circuit_open = false;
        self.last_failure_time = None;
    }

    /// Records a failed connection attempt
    pub fn record_failure(&mut self) {
        self.consecutive_failures += 1;
        self.last_failure_time = Some(std::time::Instant::now());

        if self.consecutive_failures >= self.config.max_failures {
            self.circuit_open = true;
        }
    }

    /// Checks if circuit breaker is open
    pub fn is_circuit_open(&mut self) -> bool {
        if !self.circuit_open {
            return false;
        }

        // Check if we should attempt to reset the circuit
        if let Some(last_failure) = self.last_failure_time {
            if last_failure.elapsed() >= self.config.circuit_open_duration {
                self.circuit_open = false;
                self.consecutive_failures = 0;
                return false;
            }
        }

        true
    }

    /// Calculates the next backoff duration with jitter
    pub fn next_backoff(&self) -> Duration {
        if self.consecutive_failures == 0 {
            return Duration::from_secs(0);
        }

        let base_backoff = self.config.initial_backoff.as_millis() as f64
            * self.config.backoff_multiplier.powi((self.consecutive_failures - 1) as i32);

        let capped_backoff = base_backoff.min(self.config.max_backoff.as_millis() as f64);

        // Add jitter: ±10% of the backoff duration
        let jitter = (capped_backoff * 0.1 * (rand::random::<f64>() - 0.5) * 2.0).abs();
        let final_backoff = capped_backoff + jitter;

        Duration::from_millis(final_backoff as u64)
    }

    /// Returns the current number of consecutive failures
    pub fn failure_count(&self) -> u32 {
        self.consecutive_failures
    }
}

impl Default for ReconnectionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_success_resets_failures() {
        let mut manager = ReconnectionManager::new();
        manager.record_failure();
        manager.record_failure();
        assert_eq!(manager.failure_count(), 2);

        manager.record_success();
        assert_eq!(manager.failure_count(), 0);
        assert!(!manager.is_circuit_open());
    }

    #[test]
    fn test_circuit_opens_after_max_failures() {
        let config = ReconnectionConfig {
            max_failures: 3,
            ..Default::default()
        };
        let mut manager = ReconnectionManager::with_config(config);

        manager.record_failure();
        manager.record_failure();
        assert!(!manager.is_circuit_open());

        manager.record_failure();
        assert!(manager.is_circuit_open());
    }

    #[test]
    fn test_backoff_increases_exponentially() {
        let mut manager = ReconnectionManager::new();

        manager.record_failure();
        let backoff1 = manager.next_backoff();

        manager.record_failure();
        let backoff2 = manager.next_backoff();

        // backoff2 should be roughly 2x backoff1 (with jitter)
        assert!(backoff2 > backoff1);
    }

    #[test]
    fn test_backoff_capped_at_max() {
        let config = ReconnectionConfig {
            initial_backoff: Duration::from_secs(1),
            max_backoff: Duration::from_secs(5),
            backoff_multiplier: 10.0,
            ..Default::default()
        };
        let mut manager = ReconnectionManager::with_config(config);

        for _ in 0..10 {
            manager.record_failure();
        }

        let backoff = manager.next_backoff();
        assert!(backoff <= Duration::from_secs(6)); // Allow for jitter
    }

    #[test]
    fn test_no_backoff_on_success() {
        let mut manager = ReconnectionManager::new();
        manager.record_failure();
        manager.record_success();

        assert_eq!(manager.next_backoff(), Duration::from_secs(0));
    }

    // --- Additional reconnection logic tests (#453) ---

    #[test]
    fn test_default_config_values() {
        let config = ReconnectionConfig::default();
        assert_eq!(config.initial_backoff, Duration::from_millis(100));
        assert_eq!(config.max_backoff, Duration::from_secs(30));
        assert_eq!(config.backoff_multiplier, 2.0);
        assert_eq!(config.max_failures, 5);
        assert_eq!(config.circuit_open_duration, Duration::from_secs(60));
    }

    #[test]
    fn test_new_manager_starts_clean() {
        let manager = ReconnectionManager::new();
        assert_eq!(manager.failure_count(), 0);
        // is_circuit_open takes &mut self so we need a mutable binding
        let mut manager = manager;
        assert!(!manager.is_circuit_open());
    }

    #[test]
    fn test_failure_count_increments() {
        let mut manager = ReconnectionManager::new();
        for i in 1..=4 {
            manager.record_failure();
            assert_eq!(manager.failure_count(), i);
        }
    }

    #[test]
    fn test_circuit_stays_closed_below_threshold() {
        let config = ReconnectionConfig {
            max_failures: 5,
            ..Default::default()
        };
        let mut manager = ReconnectionManager::with_config(config);

        for _ in 0..4 {
            manager.record_failure();
        }
        assert!(!manager.is_circuit_open());
    }

    #[test]
    fn test_circuit_opens_exactly_at_threshold() {
        let config = ReconnectionConfig {
            max_failures: 2,
            ..Default::default()
        };
        let mut manager = ReconnectionManager::with_config(config);

        manager.record_failure();
        assert!(!manager.is_circuit_open());
        manager.record_failure();
        assert!(manager.is_circuit_open());
    }

    #[test]
    fn test_success_after_circuit_open_resets() {
        let config = ReconnectionConfig {
            max_failures: 2,
            ..Default::default()
        };
        let mut manager = ReconnectionManager::with_config(config);

        manager.record_failure();
        manager.record_failure();
        assert!(manager.is_circuit_open());

        manager.record_success();
        assert!(!manager.is_circuit_open());
        assert_eq!(manager.failure_count(), 0);
    }

    #[test]
    fn test_circuit_resets_after_open_duration() {
        let config = ReconnectionConfig {
            max_failures: 1,
            circuit_open_duration: Duration::from_millis(1),
            ..Default::default()
        };
        let mut manager = ReconnectionManager::with_config(config);

        manager.record_failure();
        assert!(manager.is_circuit_open());

        // Sleep past the circuit_open_duration
        std::thread::sleep(Duration::from_millis(10));
        assert!(!manager.is_circuit_open(), "Circuit should auto-reset after open duration");
    }

    #[test]
    fn test_backoff_zero_before_any_failure() {
        let manager = ReconnectionManager::new();
        assert_eq!(manager.next_backoff(), Duration::from_secs(0));
    }

    #[test]
    fn test_backoff_respects_multiplier() {
        let config = ReconnectionConfig {
            initial_backoff: Duration::from_millis(100),
            max_backoff: Duration::from_secs(60),
            backoff_multiplier: 3.0,
            max_failures: 10,
            ..Default::default()
        };
        let mut manager = ReconnectionManager::with_config(config);

        manager.record_failure(); // failure 1 → base = 100ms
        let b1 = manager.next_backoff().as_millis();

        manager.record_failure(); // failure 2 → base = 300ms
        let b2 = manager.next_backoff().as_millis();

        // b2 should be roughly 3× b1 (within jitter tolerance)
        assert!(b2 > b1, "Second backoff should exceed first");
        assert!(b2 >= 270, "Second backoff should be near 300ms (±10% jitter)");
    }

    #[test]
    fn test_multiple_success_calls_are_idempotent() {
        let mut manager = ReconnectionManager::new();
        manager.record_failure();
        manager.record_success();
        manager.record_success();
        assert_eq!(manager.failure_count(), 0);
        assert!(!manager.is_circuit_open());
    }

    #[test]
    fn test_with_config_constructor() {
        let config = ReconnectionConfig {
            max_failures: 7,
            ..Default::default()
        };
        let mut manager = ReconnectionManager::with_config(config.clone());
        for _ in 0..6 {
            manager.record_failure();
        }
        assert!(!manager.is_circuit_open());
        manager.record_failure();
        assert!(manager.is_circuit_open());
    }

    #[test]
    fn test_default_trait_equals_new() {
        let a = ReconnectionManager::new();
        let b = ReconnectionManager::default();
        assert_eq!(a.failure_count(), b.failure_count());
    }
}
