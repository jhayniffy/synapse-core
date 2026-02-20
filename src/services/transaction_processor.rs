use crate::db::models::{Transaction, TransactionDlq};
use crate::error::AppError;
use sqlx::PgPool;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info, warn};
use uuid::Uuid;

const MAX_RETRIES: u32 = 3;
const BASE_DELAY_MS: u64 = 100;

pub struct TransactionProcessor {
    pool: PgPool,
}

impl TransactionProcessor {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn process_transaction(&self, tx_id: Uuid) -> Result<(), AppError> {
        let mut retry_count = 0;

        loop {
            match self.try_process(tx_id).await {
                Ok(_) => {
                    info!("Transaction {} processed successfully", tx_id);
                    return Ok(());
                }
                Err(e) if Self::is_transient_error(&e) && retry_count < MAX_RETRIES => {
                    retry_count += 1;
                    let delay = Self::calculate_backoff(retry_count);
                    warn!(
                        "Transient error processing transaction {}: {}. Retry {}/{} after {}ms",
                        tx_id, e, retry_count, MAX_RETRIES, delay
                    );
                    sleep(Duration::from_millis(delay)).await;
                }
                Err(e) => {
                    error!("Failed to process transaction {} after {} retries: {}", tx_id, retry_count, e);
                    self.move_to_dlq(tx_id, &e.to_string(), retry_count).await?;
                    return Err(e);
                }
            }
        }
    }

    async fn try_process(&self, tx_id: Uuid) -> Result<(), AppError> {
        let tx = sqlx::query_as::<_, Transaction>("SELECT * FROM transactions WHERE id = $1")
            .bind(tx_id)
            .fetch_one(&self.pool)
            .await?;

        // Placeholder for actual processing logic
        if tx.status == "pending" {
            sqlx::query("UPDATE transactions SET status = 'processing', updated_at = NOW() WHERE id = $1")
                .bind(tx_id)
                .execute(&self.pool)
                .await?;

            // Simulate processing
            sqlx::query("UPDATE transactions SET status = 'completed', updated_at = NOW() WHERE id = $1")
                .bind(tx_id)
                .execute(&self.pool)
                .await?;
        }

        Ok(())
    }

    async fn move_to_dlq(&self, tx_id: Uuid, error_reason: &str, retry_count: u32) -> Result<(), AppError> {
        let tx = sqlx::query_as::<_, Transaction>("SELECT * FROM transactions WHERE id = $1")
            .bind(tx_id)
            .fetch_one(&self.pool)
            .await?;

        sqlx::query(
            r#"
            INSERT INTO transaction_dlq (
                transaction_id, stellar_account, amount, asset_code,
                anchor_transaction_id, error_reason, stack_trace, retry_count, original_created_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#
        )
        .bind(tx.id)
        .bind(&tx.stellar_account)
        .bind(&tx.amount)
        .bind(&tx.asset_code)
        .bind(&tx.anchor_transaction_id)
        .bind(error_reason)
        .bind(std::backtrace::Backtrace::force_capture().to_string())
        .bind(retry_count as i32)
        .bind(tx.created_at)
        .execute(&self.pool)
        .await?;

        sqlx::query("UPDATE transactions SET status = 'dlq', updated_at = NOW() WHERE id = $1")
            .bind(tx_id)
            .execute(&self.pool)
            .await?;

        info!("Transaction {} moved to DLQ", tx_id);
        Ok(())
    }

    pub async fn requeue_dlq(&self, dlq_id: Uuid) -> Result<(), AppError> {
        let dlq_entry = sqlx::query_as::<_, TransactionDlq>("SELECT * FROM transaction_dlq WHERE id = $1")
            .bind(dlq_id)
            .fetch_one(&self.pool)
            .await?;

        sqlx::query("UPDATE transactions SET status = 'pending', updated_at = NOW() WHERE id = $1")
            .bind(dlq_entry.transaction_id)
            .execute(&self.pool)
            .await?;

        sqlx::query("DELETE FROM transaction_dlq WHERE id = $1")
            .bind(dlq_id)
            .execute(&self.pool)
            .await?;

        info!("DLQ entry {} requeued for transaction {}", dlq_id, dlq_entry.transaction_id);
        Ok(())
    }

    fn is_transient_error(error: &AppError) -> bool {
        matches!(error, AppError::Database(sqlx::Error::PoolTimedOut) | AppError::Database(sqlx::Error::Io(_)))
    }

    fn calculate_backoff(retry_count: u32) -> u64 {
        BASE_DELAY_MS * 2_u64.pow(retry_count - 1)
    }
}
