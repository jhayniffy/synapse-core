//! Caching module with Redis-oriented input validation, rate limiting, and
//! webhook security.
//!
//! - [`validation`]    — key, value, TTL, and pattern checks before Redis I/O
//! - [`rate_limiting`] — in-process token bucket / sliding window limits
//! - [`webhook`]       — HMAC signature verification and replay protection

pub mod rate_limiting;
pub mod validation;
pub mod webhook;

pub use rate_limiting::RateLimiter;
pub use validation::{CacheValidator, ValidationError, MAX_KEY_LENGTH, MAX_VALUE_SIZE};
