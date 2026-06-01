//! Telemetry module — OpenTelemetry tracing, connection pooling, webhook handlers,
//! input validation, reconnection logic, health checks, and metrics optimization.
//!
//! All error paths are designed to degrade gracefully without panicking.

pub mod connection_pool;
pub mod data_export;
pub mod error_handling;
pub mod health_checks;
pub mod input_validation;
pub mod metrics_optimization;
pub mod reconnection;
pub mod webhook;

pub use connection_pool::{ConnectionPool, PoolConfig};
pub use data_export::{DataExportService, ExportBatch, ExportConfig, TelemetryRecord};
pub use error_handling::{ErrorAction, ErrorHandler, TelemetryError, TelemetryResult};
pub use health_checks::{HealthCheckConfig, HealthCheckManager, HealthCheckResult};
pub use input_validation::InputValidator;
pub use metrics_optimization::{CardinalityLimiter, MetricsInstruments};
pub use reconnection::ReconnectionManager;
pub use webhook::{TelemetryWebhookHandler, WebhookPayload, WebhookResult};
