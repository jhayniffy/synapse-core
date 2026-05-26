/// Metrics collection for authentication operations.
/// Tracks authentication attempts, failures, and performance metrics.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Authentication metrics collector
#[derive(Debug, Clone)]
pub struct AuthMetrics {
    /// Total number of authentication attempts
    total_attempts: Arc<AtomicU64>,
    /// Number of successful authentications
    successful_auths: Arc<AtomicU64>,
    /// Number of failed authentications
    failed_auths: Arc<AtomicU64>,
    /// Number of validation errors
    validation_errors: Arc<AtomicU64>,
}

impl AuthMetrics {
    /// Creates a new metrics collector
    pub fn new() -> Self {
        Self {
            total_attempts: Arc::new(AtomicU64::new(0)),
            successful_auths: Arc::new(AtomicU64::new(0)),
            failed_auths: Arc::new(AtomicU64::new(0)),
            validation_errors: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Records an authentication attempt
    pub fn record_attempt(&self) {
        self.total_attempts.fetch_add(1, Ordering::Relaxed);
    }

    /// Records a successful authentication
    pub fn record_success(&self) {
        self.successful_auths.fetch_add(1, Ordering::Relaxed);
    }

    /// Records a failed authentication
    pub fn record_failure(&self) {
        self.failed_auths.fetch_add(1, Ordering::Relaxed);
    }

    /// Records a validation error
    pub fn record_validation_error(&self) {
        self.validation_errors.fetch_add(1, Ordering::Relaxed);
    }

    /// Gets the total number of authentication attempts
    pub fn total_attempts(&self) -> u64 {
        self.total_attempts.load(Ordering::Relaxed)
    }

    /// Gets the number of successful authentications
    pub fn successful_auths(&self) -> u64 {
        self.successful_auths.load(Ordering::Relaxed)
    }

    /// Gets the number of failed authentications
    pub fn failed_auths(&self) -> u64 {
        self.failed_auths.load(Ordering::Relaxed)
    }

    /// Gets the number of validation errors
    pub fn validation_errors(&self) -> u64 {
        self.validation_errors.load(Ordering::Relaxed)
    }

    /// Gets the success rate as a percentage (0-100)
    pub fn success_rate(&self) -> f64 {
        let total = self.total_attempts();
        if total == 0 {
            return 0.0;
        }
        (self.successful_auths() as f64 / total as f64) * 100.0
    }

    /// Resets all metrics to zero
    pub fn reset(&self) {
        self.total_attempts.store(0, Ordering::Relaxed);
        self.successful_auths.store(0, Ordering::Relaxed);
        self.failed_auths.store(0, Ordering::Relaxed);
        self.validation_errors.store(0, Ordering::Relaxed);
    }
}

impl Default for AuthMetrics {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_creation() {
        let metrics = AuthMetrics::new();
        assert_eq!(metrics.total_attempts(), 0);
        assert_eq!(metrics.successful_auths(), 0);
        assert_eq!(metrics.failed_auths(), 0);
        assert_eq!(metrics.validation_errors(), 0);
    }

    #[test]
    fn test_record_attempt() {
        let metrics = AuthMetrics::new();
        metrics.record_attempt();
        assert_eq!(metrics.total_attempts(), 1);
    }

    #[test]
    fn test_record_success() {
        let metrics = AuthMetrics::new();
        metrics.record_attempt();
        metrics.record_success();
        assert_eq!(metrics.total_attempts(), 1);
        assert_eq!(metrics.successful_auths(), 1);
    }

    #[test]
    fn test_record_failure() {
        let metrics = AuthMetrics::new();
        metrics.record_attempt();
        metrics.record_failure();
        assert_eq!(metrics.total_attempts(), 1);
        assert_eq!(metrics.failed_auths(), 1);
    }

    #[test]
    fn test_record_validation_error() {
        let metrics = AuthMetrics::new();
        metrics.record_validation_error();
        assert_eq!(metrics.validation_errors(), 1);
    }

    #[test]
    fn test_success_rate() {
        let metrics = AuthMetrics::new();
        metrics.record_attempt();
        metrics.record_success();
        metrics.record_attempt();
        metrics.record_failure();
        assert_eq!(metrics.success_rate(), 50.0);
    }

    #[test]
    fn test_success_rate_zero_attempts() {
        let metrics = AuthMetrics::new();
        assert_eq!(metrics.success_rate(), 0.0);
    }

    #[test]
    fn test_reset() {
        let metrics = AuthMetrics::new();
        metrics.record_attempt();
        metrics.record_success();
        metrics.record_validation_error();
        metrics.reset();
        assert_eq!(metrics.total_attempts(), 0);
        assert_eq!(metrics.successful_auths(), 0);
        assert_eq!(metrics.validation_errors(), 0);
    }

    #[test]
    fn test_metrics_clone() {
        let metrics = AuthMetrics::new();
        metrics.record_attempt();
        let cloned = metrics.clone();
        assert_eq!(cloned.total_attempts(), 1);
        cloned.record_success();
        assert_eq!(metrics.successful_auths(), 1);
    }
}
