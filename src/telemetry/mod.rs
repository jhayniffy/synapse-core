//! Telemetry module with input validation, reconnection logic, connection
//! pooling, data export, and graceful shutdown (opentelemetry tracing).

pub mod connection_pool;
pub mod data_export;
pub mod error_handling;
pub mod graceful_shutdown;
pub mod input_validation;
pub mod reconnection;

pub use connection_pool::{ConnectionPool, PoolConfig, PoolError};
pub use error_handling::{ErrorAction, ErrorHandler, TelemetryError, TelemetryResult};
pub use graceful_shutdown::{ShutdownCoordinator, ShutdownConfig, ShutdownMetrics, ShutdownPhase};
pub use input_validation::InputValidator;
pub use reconnection::ReconnectionManager;
