//! Error handling for telemetry operations.
//!
//! Provides comprehensive error types and handling strategies for
//! OpenTelemetry tracing operations.

use std::fmt;

/// Errors that can occur during telemetry operations
#[derive(Debug, thiserror::Error)]
pub enum TelemetryError {
    /// Error initializing the tracer provider
    #[error("Failed to initialize tracer: {0}")]
    InitializationError(String),

    /// Error configuring the OTLP exporter
    #[error("Failed to configure OTLP exporter: {0}")]
    ExporterConfigError(String),

    /// Error during span export
    #[error("Failed to export spans: {0}")]
    ExportError(String),

    /// Error during tracer shutdown
    #[error("Failed to shutdown tracer: {0}")]
    ShutdownError(String),

    /// Invalid endpoint configuration
    #[error("Invalid endpoint: {0}")]
    InvalidEndpoint(String),

    /// Connection error to telemetry backend
    #[error("Connection error: {0}")]
    ConnectionError(String),

    /// Validation error for telemetry input
    #[error("Validation error: {0}")]
    ValidationError(#[from] super::input_validation::ValidationError),

    /// Circuit breaker is open, rejecting requests
    #[error("Circuit breaker open: too many consecutive failures")]
    CircuitBreakerOpen,

    /// Timeout during telemetry operation
    #[error("Operation timed out after {0:?}")]
    Timeout(std::time::Duration),
}

/// Result type for telemetry operations
pub type TelemetryResult<T> = Result<T, TelemetryError>;

/// Error handler for telemetry operations
#[derive(Debug, Clone)]
pub struct ErrorHandler {
    /// Whether to fail fast on errors or continue with degraded functionality
    fail_fast: bool,
    /// Maximum number of errors to tolerate before failing
    error_threshold: usize,
    /// Current error count
    error_count: usize,
}

impl ErrorHandler {
    /// Creates a new error handler with default settings
    pub fn new() -> Self {
        Self {
            fail_fast: false,
            error_threshold: 10,
            error_count: 0,
        }
    }

    /// Creates an error handler that fails immediately on any error
    pub fn fail_fast() -> Self {
        Self {
            fail_fast: true,
            error_threshold: 1,
            error_count: 0,
        }
    }

    /// Creates an error handler with custom threshold
    pub fn with_threshold(threshold: usize) -> Self {
        Self {
            fail_fast: false,
            error_threshold: threshold,
            error_count: 0,
        }
    }

    /// Records an error and determines if operation should continue
    pub fn handle_error(&mut self, error: &TelemetryError) -> ErrorAction {
        self.error_count += 1;

        // Log the error
        tracing::warn!(
            error = %error,
            error_count = self.error_count,
            threshold = self.error_threshold,
            "Telemetry error occurred"
        );

        // Determine action based on error type and configuration
        match error {
            TelemetryError::CircuitBreakerOpen => {
                // Circuit breaker errors should always stop attempts
                ErrorAction::Stop
            }
            TelemetryError::ValidationError(_) => {
                // Validation errors indicate bad input, should fail fast
                ErrorAction::FailFast
            }
            _ => {
                if self.fail_fast || self.error_count >= self.error_threshold {
                    ErrorAction::Stop
                } else {
                    ErrorAction::Continue
                }
            }
        }
    }

    /// Resets the error count after successful operation
    pub fn reset(&mut self) {
        self.error_count = 0;
    }

    /// Returns the current error count
    pub fn error_count(&self) -> usize {
        self.error_count
    }

    /// Checks if error threshold has been exceeded
    pub fn threshold_exceeded(&self) -> bool {
        self.error_count >= self.error_threshold
    }
}

impl Default for ErrorHandler {
    fn default() -> Self {
        Self::new()
    }
}

/// Action to take after handling an error
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorAction {
    /// Continue operation despite error
    Continue,
    /// Stop operation gracefully
    Stop,
    /// Fail immediately
    FailFast,
}

/// Converts OpenTelemetry errors to TelemetryError
impl From<opentelemetry::trace::TraceError> for TelemetryError {
    fn from(error: opentelemetry::trace::TraceError) -> Self {
        TelemetryError::ExportError(error.to_string())
    }
}

/// Formats telemetry errors for user-friendly display
impl fmt::Display for ErrorAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorAction::Continue => write!(f, "continue"),
            ErrorAction::Stop => write!(f, "stop"),
            ErrorAction::FailFast => write!(f, "fail_fast"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_handler_default() {
        let handler = ErrorHandler::new();
        assert_eq!(handler.error_count(), 0);
        assert!(!handler.threshold_exceeded());
    }

    #[test]
    fn test_error_handler_fail_fast() {
        let mut handler = ErrorHandler::fail_fast();
        let error = TelemetryError::ExportError("test".to_string());
        let action = handler.handle_error(&error);
        assert_eq!(action, ErrorAction::Stop);
    }

    #[test]
    fn test_error_handler_threshold() {
        let mut handler = ErrorHandler::with_threshold(3);
        let error = TelemetryError::ExportError("test".to_string());

        // First two errors should continue
        assert_eq!(handler.handle_error(&error), ErrorAction::Continue);
        assert_eq!(handler.handle_error(&error), ErrorAction::Continue);

        // Third error should stop
        assert_eq!(handler.handle_error(&error), ErrorAction::Stop);
        assert!(handler.threshold_exceeded());
    }

    #[test]
    fn test_error_handler_reset() {
        let mut handler = ErrorHandler::with_threshold(5);
        let error = TelemetryError::ExportError("test".to_string());

        handler.handle_error(&error);
        handler.handle_error(&error);
        assert_eq!(handler.error_count(), 2);

        handler.reset();
        assert_eq!(handler.error_count(), 0);
        assert!(!handler.threshold_exceeded());
    }

    #[test]
    fn test_circuit_breaker_error_always_stops() {
        let mut handler = ErrorHandler::new();
        let error = TelemetryError::CircuitBreakerOpen;
        let action = handler.handle_error(&error);
        assert_eq!(action, ErrorAction::Stop);
    }

    #[test]
    fn test_validation_error_fails_fast() {
        let mut handler = ErrorHandler::new();
        let validation_error =
            super::super::input_validation::ValidationError::EmptyValue("test".to_string());
        let error = TelemetryError::ValidationError(validation_error);
        let action = handler.handle_error(&error);
        assert_eq!(action, ErrorAction::FailFast);
    }

    #[test]
    fn test_error_types() {
        let init_err = TelemetryError::InitializationError("init failed".to_string());
        assert!(init_err.to_string().contains("initialize tracer"));

        let export_err = TelemetryError::ExporterConfigError("config failed".to_string());
        assert!(export_err.to_string().contains("configure OTLP"));

        let conn_err = TelemetryError::ConnectionError("connection failed".to_string());
        assert!(conn_err.to_string().contains("Connection error"));
    }

    #[test]
    fn test_timeout_error() {
        let timeout = std::time::Duration::from_secs(30);
        let error = TelemetryError::Timeout(timeout);
        assert!(error.to_string().contains("timed out"));
    }
}
