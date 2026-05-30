# Payments Error Handling Documentation

## Overview

This document provides comprehensive documentation for error handling within the Payments module, specifically focusing on settlement logic in `synapse-core`.

## Architecture

The Payments module uses a centralized error handling approach through the `AppError` enum defined in `src/error.rs`. Settlement-specific errors are handled through dedicated error codes and state machine validation.

## Settlement Error Codes

### ERR_SETTLEMENT_001: Invalid Settlement Amount
- **HTTP Status**: 400 Bad Request
- **Description**: Settlement amount validation failed
- **Common Causes**:
  - Negative settlement amounts
  - Zero-value settlements
  - Amount precision exceeds supported decimal places
- **Resolution**: Ensure settlement amounts are positive and within valid ranges

### ERR_SETTLEMENT_002: Settlement Already Exists
- **HTTP Status**: 409 Conflict
- **Description**: Attempted to create a duplicate settlement
- **Common Causes**:
  - Idempotency key collision
  - Race condition in settlement creation
  - Retry of already-processed settlement request
- **Resolution**: Check existing settlements before creating new ones

## Settlement State Machine

The settlement service implements a strict state machine to prevent invalid state transitions:

```
completed → pending_review
pending_review → disputed | voided | completed
disputed → adjusted | voided
adjusted → completed
```

### Valid Transitions

| From State | To State | Description |
|------------|----------|-------------|
| completed | pending_review | Settlement flagged for manual review |
| pending_review | disputed | Discrepancy found during review |
| pending_review | voided | Settlement cancelled during review |
| pending_review | completed | Review completed, settlement confirmed |
| disputed | adjusted | Settlement amount corrected |
| disputed | voided | Settlement cancelled due to dispute |
| adjusted | completed | Adjusted settlement finalized |

### Invalid Transition Handling

When an invalid state transition is attempted, the system returns:
- **Error Code**: ERR_BAD_REQUEST_001
- **HTTP Status**: 400 Bad Request
- **Message**: "invalid transition: {from_state} -> {to_state}"

**Example**:
```rust
// Attempting to transition from "completed" to "voided" directly
// This will fail because it's not a valid transition
service.update_status(id, "voided", None, None, "admin").await
// Returns: AppError::BadRequest("invalid transition: completed -> voided")
```

## Error Handling in Settlement Service

### Database Transaction Management

The `SettlementService` uses PostgreSQL transactions to ensure atomicity:

```rust
let mut tx = self.pool.begin().await?;
// ... perform settlement operations
tx.commit().await?;
```

**Error Scenarios**:
1. **Transaction Begin Failure**: Returns `AppError::DatabaseError`
2. **Query Execution Failure**: Rolls back transaction, returns `AppError::DatabaseError`
3. **Commit Failure**: Returns `AppError::DatabaseError` with rollback

### Batch Processing Error Handling

Settlements are processed in batches (default: 10,000 transactions per batch). Errors during batch processing:

1. **Partial Batch Failure**: Entire transaction is rolled back
2. **Batch Size Validation**: Configurable via `max_batch_size`
3. **Minimum Transaction Count**: Settlements below `min_tx_count` are skipped (not errors)

**Example**:
```rust
// If only 3 transactions exist but min_tx_count is 5
// The settlement is skipped with an info log, not an error
if unsettled.len() < self.min_tx_count {
    tx.rollback().await?;
    return Ok(vec![]);
}
```

### Status Update Error Handling

The `update_status` method validates transitions before database operations:

```rust
pub async fn update_status(
    &self,
    id: Uuid,
    new_status: &str,
    reason: Option<&str>,
    new_total: Option<&BigDecimal>,
    actor: &str,
) -> Result<Settlement, AppError>
```

**Error Cases**:
1. **Settlement Not Found**: Returns `AppError::NotFound`
2. **Invalid Transition**: Returns `AppError::BadRequest`
3. **Database Error**: Returns `AppError::DatabaseError`

## API Error Responses

All settlement errors return structured JSON responses:

```json
{
  "error": "Human-readable error message",
  "code": "ERR_SETTLEMENT_001",
  "status": 400,
  "timestamp": "2026-05-29T10:30:00Z",
  "detail": "Actionable detail message",
  "docs_url": "/errors#ERR_SETTLEMENT_001"
}
```

## Handler-Level Error Handling

### List Settlements (`GET /settlements`)

**Error Scenarios**:
- Invalid cursor format → `AppError::BadRequest`
- Database query failure → `AppError::DatabaseError` (500)

### Get Settlement (`GET /settlements/{id}`)

**Error Scenarios**:
- Settlement not found → `AppError::NotFound` (404)
- Database query failure → `AppError::DatabaseError` (500)

### Update Settlement Status (`PATCH /admin/settlements/{id}/status`)

**Error Scenarios**:
- Invalid `new_total` format → `AppError::BadRequest` (400)
- Settlement not found → `AppError::NotFound` (404)
- Invalid state transition → `AppError::BadRequest` (400)
- Database error → `AppError::DatabaseError` (500)

## Security Considerations

### Input Validation

1. **Settlement ID**: Validated as UUID format
2. **Status Values**: Validated against allowed state machine transitions
3. **Amount Values**: Validated as positive BigDecimal values
4. **Actor Field**: Sanitized to prevent injection attacks

### Authorization

- Settlement status updates require admin privileges
- Audit logging tracks all status changes with actor information

## Monitoring and Observability

### Logging

Settlement operations emit structured logs:

```rust
tracing::info!(
    asset = %asset_code,
    settlement_id = %saved.id,
    batch = batch_idx + 1,
    total_batches = batch_count,
    tx_count,
    total_amount = %total_amount,
    "Settlement batch created"
);
```

### Error Logging

Errors are logged with context:

```rust
tracing::error!(
    "Failed to settle asset {:?}: {:?}",
    asset_code,
    error
);
```

## Recovery Procedures

### Failed Settlement Recovery

1. **Identify Failed Settlement**: Check logs for `DatabaseError` entries
2. **Verify Transaction State**: Ensure transactions are not partially settled
3. **Retry Settlement**: Re-run settlement for the affected asset
4. **Manual Intervention**: If retry fails, use admin endpoints to manually adjust

### Disputed Settlement Resolution

1. **Transition to `pending_review`**: Flag settlement for investigation
2. **Investigate Discrepancy**: Review transaction details and amounts
3. **Resolution Paths**:
   - **Correct**: Transition back to `completed`
   - **Adjust**: Transition to `adjusted` with corrected amount
   - **Cancel**: Transition to `voided` to release transactions

### Voided Settlement Handling

When a settlement is voided:
- All associated transactions have their `settlement_id` set to `NULL`
- Transactions become eligible for future settlements
- Audit log records the void action and actor

## Testing Error Scenarios

### Unit Tests

The settlement service includes comprehensive error handling tests:

```rust
#[tokio::test]
async fn test_settle_error_handling() {
    // Tests database error propagation
}

#[tokio::test]
async fn test_invalid_transition() {
    // Tests state machine validation
}
```

### Integration Tests

Integration tests cover end-to-end error scenarios:

```rust
#[tokio::test]
async fn test_settlement_dispute_review_resolution_flow() {
    // Tests complete dispute resolution workflow
}

#[tokio::test]
async fn test_voided_settlement_releases_transactions() {
    // Tests transaction release on void
}
```

## Best Practices

1. **Always Use Transactions**: Wrap settlement operations in database transactions
2. **Validate Before Mutating**: Check state transitions before database updates
3. **Log Contextual Information**: Include asset codes, settlement IDs, and amounts in logs
4. **Handle Partial Failures**: Ensure rollback on any batch processing error
5. **Audit All Changes**: Log all status transitions with actor information
6. **Use Structured Errors**: Return consistent error codes for programmatic handling
7. **Cache Invalidation**: Invalidate relevant caches after successful settlements

## Related Documentation

- [Error Catalog](./error-catalog.md) - Complete list of error codes
- [Settlement Architecture](./architecture.md#settlements) - Settlement system design
- [State Machine](./state-machine.md) - Detailed state machine documentation
- [API Reference](./api-reference.md#settlements) - Settlement API endpoints

## Troubleshooting

### Common Issues

**Issue**: Settlement fails with "invalid transition" error
- **Cause**: Attempting unsupported state transition
- **Solution**: Review state machine diagram and use valid transition path

**Issue**: Settlement skipped with "below minimum" log
- **Cause**: Transaction count below `min_tx_count` threshold
- **Solution**: Wait for more transactions or adjust `min_tx_count` configuration

**Issue**: Database error during settlement
- **Cause**: Connection failure, constraint violation, or deadlock
- **Solution**: Check database logs, verify schema, retry operation

**Issue**: Settlement amount mismatch
- **Cause**: Concurrent transaction updates during settlement
- **Solution**: Use `pending_review` → `adjusted` workflow to correct

## Configuration

Settlement behavior is configurable:

```rust
SettlementService::with_config(
    pool,
    max_batch_size: 10_000,  // Maximum transactions per settlement
    min_tx_count: 1,          // Minimum transactions required
)
```

## Conclusion

The Payments module implements robust error handling through:
- Centralized error types with stable error codes
- State machine validation for settlement transitions
- Transactional integrity for all settlement operations
- Comprehensive logging and audit trails
- Structured error responses for API consumers

For questions or issues, refer to the [runbook](./runbook.md) or contact the platform team.
