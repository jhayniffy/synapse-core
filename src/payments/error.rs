//! Typed error variants for the payments / settlement module.
//!
//! [`PaymentError`] maps cleanly to [`crate::error::AppError`] so that
//! settlement logic can return rich, domain-specific errors while the HTTP
//! layer converts them to the correct status codes automatically.

use crate::error::AppError;
use thiserror::Error;

/// Domain errors that can occur during payment and settlement processing.
#[derive(Debug, Error, PartialEq)]
pub enum PaymentError {
    /// The supplied amount string is not a valid positive decimal, or it
    /// violates precision / range constraints.
    #[error("Invalid payment amount: {0}")]
    InvalidAmount(String),

    /// The amount is syntactically valid but falls below the operational
    /// minimum (dust-transaction guard).
    #[error("Amount below minimum: {0}")]
    AmountBelowMinimum(String),

    /// The supplied asset code is not recognised or not supported.
    #[error("Invalid asset code: {0}")]
    InvalidAssetCode(String),

    /// The supplied status value is not a member of the allowed set.
    #[error("Invalid settlement status: {0}")]
    InvalidStatus(String),

    /// The requested status transition is not permitted by the state machine.
    #[error("Invalid status transition: {0}")]
    InvalidTransition(String),

    /// A settlement with the same identity already exists.
    #[error("Settlement already exists: {0}")]
    AlreadyExists(String),

    /// A required settlement record could not be found.
    #[error("Settlement not found: {0}")]
    NotFound(String),

    /// An underlying database operation failed.
    #[error("Database error: {0}")]
    Database(String),
}

impl From<PaymentError> for AppError {
    fn from(err: PaymentError) -> Self {
        match err {
            PaymentError::InvalidAmount(msg) => AppError::InvalidTransactionAmount(msg),
            PaymentError::AmountBelowMinimum(msg) => AppError::AmountBelowMinimum(msg),
            PaymentError::InvalidAssetCode(msg) => AppError::BadRequest(msg),
            PaymentError::InvalidStatus(msg) => AppError::BadRequest(msg),
            PaymentError::InvalidTransition(msg) => AppError::InvalidStatusTransition(msg),
            PaymentError::AlreadyExists(msg) => AppError::SettlementAlreadyExists(msg),
            PaymentError::NotFound(msg) => AppError::NotFound(msg),
            PaymentError::Database(msg) => AppError::DatabaseError(msg),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use axum::response::IntoResponse;

    fn http_status(err: PaymentError) -> StatusCode {
        let app_err: AppError = err.into();
        app_err.into_response().status()
    }

    #[test]
    fn invalid_amount_maps_to_400() {
        assert_eq!(
            http_status(PaymentError::InvalidAmount("bad".into())),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn amount_below_minimum_maps_to_400() {
        assert_eq!(
            http_status(PaymentError::AmountBelowMinimum("too small".into())),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn invalid_transition_maps_to_400() {
        assert_eq!(
            http_status(PaymentError::InvalidTransition("pending -> voided".into())),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn already_exists_maps_to_409() {
        assert_eq!(
            http_status(PaymentError::AlreadyExists("s-1".into())),
            StatusCode::CONFLICT
        );
    }

    #[test]
    fn not_found_maps_to_404() {
        assert_eq!(
            http_status(PaymentError::NotFound("s-1".into())),
            StatusCode::NOT_FOUND
        );
    }

    #[test]
    fn database_error_maps_to_500() {
        assert_eq!(
            http_status(PaymentError::Database("conn refused".into())),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[test]
    fn error_display_includes_message() {
        let err = PaymentError::InvalidAmount("must be positive".into());
        assert!(err.to_string().contains("must be positive"));
    }
}
