//! Rate limiting implementation using Redis.
//!
//! Provides token bucket and sliding window rate limiting strategies
//! with configurable limits and time windows.

use std::time::Duration;

/// Rate limiting configuration
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Maximum number of requests allowed
    pub max_requests: u32,
    /// Time window for the rate limit
    pub window: Duration,
    /// Strategy to use for rate limiting
    pub strategy: RateLimitStrategy,
}

/// Rate limiting strategies
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateLimitStrategy {
    /// Token bucket algorithm
    TokenBucket,
    /// Sliding window algorithm
    SlidingWindow,
}

/// Cache-level metrics for rate limiting operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CacheMetrics {
    acquired_requests: u64,
    rejected_requests: u64,
    refill_events: u64,
}

impl CacheMetrics {
    /// Creates a new metrics collector.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a successful token acquisition.
    pub fn record_acquired(&mut self) {
        self.acquired_requests = self.acquired_requests.saturating_add(1);
    }

    /// Record a rejected request due to rate limiting.
    pub fn record_rejected(&mut self) {
        self.rejected_requests = self.rejected_requests.saturating_add(1);
    }

    /// Record a refill event when tokens are replenished.
    pub fn record_refill(&mut self) {
        self.refill_events = self.refill_events.saturating_add(1);
    }

    /// Returns the number of acquired requests.
    pub fn acquired_requests(&self) -> u64 {
        self.acquired_requests
    }

    /// Returns the number of rejected requests.
    pub fn rejected_requests(&self) -> u64 {
        self.rejected_requests
    }

    /// Returns the number of refill events.
    pub fn refill_events(&self) -> u64 {
        self.refill_events
    }
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_requests: 100,
            window: Duration::from_secs(60),
            strategy: RateLimitStrategy::TokenBucket,
        }
    }
}

/// Rate limiter for controlling request rates
#[derive(Debug, Clone)]
pub struct RateLimiter {
    config: RateLimitConfig,
    tokens: u32,
    last_refill: std::time::Instant,
    metrics: CacheMetrics,
}

impl RateLimiter {
    /// Creates a new rate limiter with default configuration
    pub fn new() -> Self {
        Self::with_config(RateLimitConfig::default())
    }

    /// Creates a new rate limiter with custom configuration
    pub fn with_config(config: RateLimitConfig) -> Self {
        Self {
            config,
            tokens: config.max_requests,
            last_refill: std::time::Instant::now(),
            metrics: CacheMetrics::new(),
        }
    }

    /// Returns read-only cache metrics for this limiter.
    pub fn metrics(&self) -> &CacheMetrics {
        &self.metrics
    }

    /// Attempts to acquire a token for a request
    ///
    /// Returns `true` if a token was available, `false` otherwise
    pub fn try_acquire(&mut self) -> bool {
        self.refill_tokens();

        if self.tokens > 0 {
            self.tokens -= 1;
            self.metrics.record_acquired();
            true
        } else {
            self.metrics.record_rejected();
            false
        }
    }

    /// Attempts to acquire multiple tokens
    ///
    /// Returns `true` if enough tokens were available, `false` otherwise
    pub fn try_acquire_batch(&mut self, count: u32) -> bool {
        self.refill_tokens();

        if self.tokens >= count {
            self.tokens -= count;
            self.metrics.record_acquired();
            true
        } else {
            self.metrics.record_rejected();
            false
        }
    }

    /// Returns the number of available tokens
    pub fn available_tokens(&mut self) -> u32 {
        self.refill_tokens();
        self.tokens
    }

    /// Returns the time until the next token is available
    pub fn time_until_available(&mut self) -> Option<Duration> {
        if self.try_acquire() {
            return Some(Duration::from_secs(0));
        }

        let elapsed = self.last_refill.elapsed();
        if elapsed >= self.config.window {
            return Some(Duration::from_secs(0));
        }

        Some(self.config.window - elapsed)
    }

    /// Refills tokens based on elapsed time
    fn refill_tokens(&mut self) {
        let elapsed = self.last_refill.elapsed();

        if elapsed >= self.config.window {
            self.tokens = self.config.max_requests;
            self.last_refill = std::time::Instant::now();
            self.metrics.record_refill();
        } else {
            // Calculate tokens to add based on elapsed time
            let refill_rate = self.config.max_requests as f64 / self.config.window.as_secs_f64();
            let tokens_to_add = (elapsed.as_secs_f64() * refill_rate) as u32;

            if tokens_to_add > 0 {
                self.tokens = (self.tokens + tokens_to_add).min(self.config.max_requests);
                self.last_refill = std::time::Instant::now();
                self.metrics.record_refill();
            }
        }
    }

    /// Resets the rate limiter to initial state
    pub fn reset(&mut self) {
        self.tokens = self.config.max_requests;
        self.last_refill = std::time::Instant::now();
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_acquire_token() {
        let mut limiter = RateLimiter::new();
        assert!(limiter.try_acquire());
    }

    #[test]
    fn test_exhaust_tokens() {
        let config = RateLimitConfig {
            max_requests: 3,
            window: Duration::from_secs(60),
            strategy: RateLimitStrategy::TokenBucket,
        };
        let mut limiter = RateLimiter::with_config(config);

        assert!(limiter.try_acquire());
        assert!(limiter.try_acquire());
        assert!(limiter.try_acquire());
        assert!(!limiter.try_acquire());
    }

    #[test]
    fn test_acquire_batch() {
        let config = RateLimitConfig {
            max_requests: 10,
            window: Duration::from_secs(60),
            strategy: RateLimitStrategy::TokenBucket,
        };
        let mut limiter = RateLimiter::with_config(config);

        assert!(limiter.try_acquire_batch(5));
        assert!(limiter.try_acquire_batch(5));
        assert!(!limiter.try_acquire_batch(1));
    }

    #[test]
    fn test_available_tokens() {
        let config = RateLimitConfig {
            max_requests: 5,
            window: Duration::from_secs(60),
            strategy: RateLimitStrategy::TokenBucket,
        };
        let mut limiter = RateLimiter::with_config(config);

        assert_eq!(limiter.available_tokens(), 5);
        limiter.try_acquire();
        assert_eq!(limiter.available_tokens(), 4);
    }

    #[test]
    fn test_reset() {
        let config = RateLimitConfig {
            max_requests: 5,
            window: Duration::from_secs(60),
            strategy: RateLimitStrategy::TokenBucket,
        };
        let mut limiter = RateLimiter::with_config(config);

        limiter.try_acquire();
        limiter.try_acquire();
        assert_eq!(limiter.available_tokens(), 3);

        limiter.reset();
        assert_eq!(limiter.available_tokens(), 5);
    }

    #[test]
    fn test_metrics_record_acquire_and_reject() {
        let config = RateLimitConfig {
            max_requests: 1,
            window: Duration::from_secs(60),
            strategy: RateLimitStrategy::TokenBucket,
        };
        let mut limiter = RateLimiter::with_config(config);

        assert!(limiter.try_acquire());
        assert_eq!(limiter.metrics().acquired_requests(), 1);
        assert_eq!(limiter.metrics().rejected_requests(), 0);

        assert!(!limiter.try_acquire());
        assert_eq!(limiter.metrics().acquired_requests(), 1);
        assert_eq!(limiter.metrics().rejected_requests(), 1);
    }

    #[test]
    fn test_metrics_record_batch_reject() {
        let config = RateLimitConfig {
            max_requests: 3,
            window: Duration::from_secs(60),
            strategy: RateLimitStrategy::TokenBucket,
        };
        let mut limiter = RateLimiter::with_config(config);

        assert!(limiter.try_acquire_batch(2));
        assert_eq!(limiter.metrics().acquired_requests(), 1);
        assert_eq!(limiter.metrics().rejected_requests(), 0);

        assert!(!limiter.try_acquire_batch(2));
        assert_eq!(limiter.metrics().acquired_requests(), 1);
        assert_eq!(limiter.metrics().rejected_requests(), 1);
    }

    #[test]
    fn test_metrics_refill_event() {
        let config = RateLimitConfig {
            max_requests: 1,
            window: Duration::from_secs(1),
            strategy: RateLimitStrategy::TokenBucket,
        };
        let mut limiter = RateLimiter::with_config(config);

        assert!(limiter.try_acquire());
        assert_eq!(limiter.available_tokens(), 0);

        std::thread::sleep(Duration::from_secs(1));
        assert!(limiter.available_tokens() > 0);
        assert!(limiter.metrics().refill_events() > 0);
    }

    #[test]
    fn test_time_until_available() {
        let config = RateLimitConfig {
            max_requests: 1,
            window: Duration::from_secs(60),
            strategy: RateLimitStrategy::TokenBucket,
        };
        let mut limiter = RateLimiter::with_config(config);

        limiter.try_acquire();
        let time_until = limiter.time_until_available();
        assert!(time_until.is_some());
        assert!(time_until.unwrap() > Duration::from_secs(0));
    }
}
