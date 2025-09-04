use chrono::{DateTime, Utc};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::env;
use std::str::FromStr;
use tracing::debug;

#[cfg(all(not(target_env = "msvc"), feature = "tikv-jemallocator"))]
use tikv_jemallocator::Jemalloc;

#[cfg(all(not(target_env = "msvc"), feature = "tikv-jemallocator"))]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

// ============================================================================
// NEW BATCH PROCESSING TYPES
// ============================================================================

/// Incoming transaction waiting to be batched
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::FromRow)]
pub struct IncomingTransaction {
    pub id: i32,
    pub amount: i32,
    pub included_in_batch_id: Option<i32>,
    pub created_at: DateTime<Utc>,
}

/// Batch of transactions with ZK proof
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProofBatch {
    pub id: i32,
    pub previous_counter_value: i64,
    pub final_counter_value: i64,
    pub transaction_ids: Vec<i32>,
    pub sindri_proof_id: Option<String>,
    pub proof_status: String, // pending, proven, failed
    pub created_at: DateTime<Utc>,
    pub proven_at: Option<DateTime<Utc>>,
}

/// ADS/Merkle tree state commitment for smart contract
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdsStateCommit {
    pub id: i32,
    pub batch_id: i32,
    pub merkle_root: Vec<u8>,
    pub created_at: DateTime<Utc>,
}

/// Current counter state with Merkle root
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CounterState {
    pub counter_value: i64,
    pub merkle_root: Option<Vec<u8>>,
    pub last_batch_id: Option<i32>,
}

/// Contract submission data (public/private split)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractSubmissionData {
    pub public: ContractPublicData,
    pub private: ContractPrivateData,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractPublicData {
    pub prev_merkle_root: String,
    pub new_merkle_root: String,
    pub zk_proof: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractPrivateData {
    pub prev_counter_value: i64,
    pub new_counter_value: i64,
    pub transactions: Vec<i32>,
}

// ============================================================================
// CCR DOMAIN TYPES (Credit and Compliance Requirements)
// ============================================================================

/// Identity/User record with KYC status and authentication
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::FromRow)]
pub struct Identity {
    pub id: uuid::Uuid,
    pub user_type: String, // investor, lender
    pub email: String,
    pub kyc_status: String, // not_submitted, pending, approved, rejected
    pub kyc_submitted_at: Option<DateTime<Utc>>,
    pub kyc_verified_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    // Authentication fields
    pub password_hash: Option<String>,
    pub last_login_at: Option<DateTime<Utc>>,
    pub email_verified: Option<bool>,
    pub email_verification_token: Option<String>,
    pub password_reset_token: Option<String>,
    pub password_reset_expires_at: Option<DateTime<Utc>>,
    // User profile fields
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub jurisdiction: Option<String>,
    // Wallet integration
    pub wallet_address: Option<String>,
}

/// Loan record with borrower/lender relationship
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct Loan {
    pub id: uuid::Uuid,
    pub borrower_id: uuid::Uuid,
    pub lender_id: uuid::Uuid,
    pub principal_amount: rust_decimal::Decimal,
    pub outstanding_balance: rust_decimal::Decimal,
    pub interest_rate_bps: i32, // Basis points
    pub loan_status: String,    // originated, active, paid_off, closed, defaulted
    pub originated_at: DateTime<Utc>,
    pub maturity_date: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Payment record for loan servicing
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct Payment {
    pub id: uuid::Uuid,
    pub loan_id: uuid::Uuid,
    pub payer_id: uuid::Uuid,
    pub amount: rust_decimal::Decimal,
    pub payment_type: String, // regular, principal, interest, penalty
    pub transaction_hash: Option<String>,
    pub processed_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

/// KYC statistics for admin dashboard
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KycStatistics {
    pub total_users: u64,
    pub approved_count: u64,
    pub pending_count: u64,
    pub rejected_count: u64,
    pub not_submitted_count: u64,
}

/// Account balance tracking
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct Balance {
    pub id: uuid::Uuid,
    pub identity_id: uuid::Uuid,
    pub balance_type: String, // available, locked, reserved
    pub amount: rust_decimal::Decimal,
    pub currency: String,
    pub updated_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

/// Loan transaction for zkVM processing
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::FromRow)]
pub struct LoanTransactionDb {
    pub id: uuid::Uuid,
    pub transaction_id_hash: Vec<u8>,
    pub user_id_hash: Vec<u8>,
    pub loan_id_hash: Option<Vec<u8>>,
    pub transaction_type: String,
    pub amount: rust_decimal::Decimal,
    pub identity_id: Option<uuid::Uuid>,
    pub loan_id: Option<uuid::Uuid>,
    pub payment_id: Option<uuid::Uuid>,
    pub included_in_batch_id: Option<i32>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

/// State snapshot for auditing
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct StateSnapshot {
    pub id: uuid::Uuid,
    pub snapshot_type: String, // manual, batch, scheduled
    pub batch_id: Option<i32>,
    pub merkle_root: Option<Vec<u8>>,
    pub total_loans_count: i32,
    pub total_outstanding_balance: rust_decimal::Decimal,
    pub total_paid_amount: rust_decimal::Decimal,
    pub active_users_count: i32,
    pub kyc_approved_count: i32,
    pub created_at: DateTime<Utc>,
}

/// USDC deposit tracking from smart contract events
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct UsdcDeposit {
    pub id: i32,
    pub user_address: String,
    pub amount: rust_decimal::Decimal,
    pub deposit_amount_usdc: Option<rust_decimal::Decimal>,
    pub transaction_hash: String,
    pub event_transaction_hash: String,
    pub block_number: i64,
    pub event_timestamp: i64,
    pub block_timestamp: Option<DateTime<Utc>>,
    pub log_index: i64,
    pub event_signature: String,
    pub status: String, // pending, confirmed, processed, failed
    pub confirmation_count: i32,
    pub required_confirmations: i32,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub processed_at: Option<DateTime<Utc>>,
}

/// User deposit balance aggregation
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct UserDepositBalance {
    pub id: i32,
    pub user_address: String,
    pub total_deposited: rust_decimal::Decimal,
    pub total_deposited_usdc: Option<rust_decimal::Decimal>,
    pub deposit_count: i32,
    pub first_deposit_at: Option<DateTime<Utc>>,
    pub last_deposit_at: Option<DateTime<Utc>>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// USDC withdrawal record from smart contract events
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct UsdcWithdrawal {
    pub id: i32,
    pub user_address: String,
    pub recipient_address: String,
    pub amount: rust_decimal::Decimal,
    pub withdrawal_amount_usdc: Option<rust_decimal::Decimal>,
    pub transaction_hash: String,
    pub event_transaction_hash: String,
    pub block_number: i64,
    pub event_timestamp: i64,
    pub block_timestamp: Option<DateTime<Utc>>,
    pub log_index: i64,
    pub event_signature: String,
    pub status: String, // pending, confirmed, processed, failed
    pub confirmation_count: i32,
    pub required_confirmations: i32,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub processed_at: Option<DateTime<Utc>>,
}

/// User withdrawal balance aggregation
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct UserWithdrawalBalance {
    pub id: i32,
    pub user_address: String,
    pub total_withdrawn: rust_decimal::Decimal,
    pub total_withdrawn_usdc: Option<rust_decimal::Decimal>,
    pub withdrawal_count: i32,
    pub first_withdrawal_at: Option<DateTime<Utc>>,
    pub last_withdrawal_at: Option<DateTime<Utc>>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// User-initiated withdrawal request (before smart contract execution)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct WithdrawalRequest {
    pub id: i32,
    pub user_id: uuid::Uuid,
    pub user_address: String,
    pub recipient_address: String,
    pub amount: rust_decimal::Decimal,
    pub amount_usdc: Option<rust_decimal::Decimal>,
    pub state: String, // pending, validated, submitted, confirmed, failed, cancelled
    pub kyc_validated: bool,
    pub balance_validated: bool,
    pub kyc_status_at_request: Option<String>,
    pub available_balance_at_request: Option<rust_decimal::Decimal>,
    pub transaction_hash: Option<String>,
    pub block_number: Option<i64>,
    pub confirmation_count: i32,
    pub required_confirmations: i32,
    pub error_message: Option<String>,
    pub validation_errors: Option<serde_json::Value>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub validated_at: Option<DateTime<Utc>>,
    pub submitted_at: Option<DateTime<Utc>>,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub failed_at: Option<DateTime<Utc>>,
    pub cancelled_at: Option<DateTime<Utc>>,
}

/// Withdrawal request processing history
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct WithdrawalRequestHistory {
    pub id: i32,
    pub withdrawal_request_id: i32,
    pub event_type: String, // kyc_validation, balance_validation, contract_submission, etc.
    pub old_state: Option<String>,
    pub new_state: Option<String>,
    pub success: bool,
    pub message: Option<String>,
    pub details: Option<serde_json::Value>,
    pub retry_count: i32,
    pub next_retry_at: Option<DateTime<Utc>>,
    pub created_at: Option<DateTime<Utc>>,
    pub processing_duration_ms: Option<i32>,
}

// ============================================================================
// DATABASE CONNECTION FUNCTIONS
// ============================================================================

/// Initialize the database connection with a specific URL
///
/// # Errors
/// Returns error if connection fails or migrations fail
pub async fn init_db_with_url(database_url: &str) -> Result<PgPool, sqlx::Error> {
    debug!("Connecting to PostgreSQL database...");

    // Configure pool with production settings
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(20)
        .min_connections(5)
        .acquire_timeout(std::time::Duration::from_secs(30))
        .idle_timeout(std::time::Duration::from_secs(600))
        .max_lifetime(std::time::Duration::from_secs(1800))
        .connect_with(sqlx::postgres::PgConnectOptions::from_str(database_url)?)
        .await?;

    // Run migrations
    run_migrations(&pool).await?;

    debug!("Database ready");
    Ok(pool)
}

/// Initialize the database connection
///
/// # Errors
/// Returns error if `DATABASE_URL` environment variable is not set, connection fails,
/// or migrations fail
pub async fn init_db() -> Result<PgPool, sqlx::Error> {
    let database_url = env::var("DATABASE_URL").map_err(|_| {
        sqlx::Error::Configuration("DATABASE_URL environment variable must be set".into())
    })?;

    init_db_with_url(&database_url).await
}

/// Run database migrations
///
/// # Errors
/// Returns error if migration execution fails
async fn run_migrations(pool: &PgPool) -> Result<(), sqlx::Error> {
    debug!("Running database migrations...");

    sqlx::migrate!().run(pool).await?;

    debug!("Migrations completed");
    Ok(())
}

// ============================================================================
// TRANSACTION FUNCTIONS
// ============================================================================

/// Submit a new transaction to the queue
///
/// # Errors
/// Returns error if database operation fails
pub async fn submit_transaction(
    pool: &PgPool,
    amount: i32,
) -> Result<IncomingTransaction, sqlx::Error> {
    debug!("Submitting transaction: amount={amount}");

    let row = sqlx::query!(
        "INSERT INTO incoming_transactions (amount) VALUES ($1) RETURNING id, amount, included_in_batch_id, created_at",
        amount
    )
    .fetch_one(pool)
    .await?;

    let transaction = IncomingTransaction {
        id: row.id,
        amount: row.amount,
        included_in_batch_id: row.included_in_batch_id,
        created_at: row.created_at.unwrap_or_else(|| Utc::now()),
    };

    debug!("Transaction submitted: id={}", transaction.id);
    Ok(transaction)
}

/// Get pending transactions (not yet batched)
///
/// # Errors
/// Returns error if database operation fails
pub async fn get_pending_transactions(
    pool: &PgPool,
) -> Result<Vec<IncomingTransaction>, sqlx::Error> {
    debug!("Getting pending transactions");

    let rows = sqlx::query!(
        "SELECT transaction_id as id, amount, created_at FROM get_unbatched_transactions(1000)"
    )
    .fetch_all(pool)
    .await?;

    let transactions: Vec<IncomingTransaction> = rows
        .into_iter()
        .map(|row| IncomingTransaction {
            id: row.id.unwrap_or(0),
            amount: row.amount.unwrap_or(0),
            included_in_batch_id: None,
            created_at: row.created_at.unwrap_or_else(|| Utc::now()),
        })
        .collect();

    debug!("Found {} pending transactions", transactions.len());
    Ok(transactions)
}

// ============================================================================
// BATCH FUNCTIONS
// ============================================================================

/// Create a new batch from pending transactions
///
/// # Errors
/// Returns error if database operation fails
pub async fn create_batch(
    pool: &PgPool,
    batch_size: Option<i32>,
) -> Result<Option<ProofBatch>, sqlx::Error> {
    let size = batch_size.unwrap_or(10);
    debug!("Creating batch with size: {size}");

    let batch_id: i32 = sqlx::query_scalar!("SELECT create_batch($1)", size)
        .fetch_one(pool)
        .await?
        .unwrap_or(0);

    if batch_id == 0 {
        debug!("No transactions to batch");
        return Ok(None);
    }

    // Get the created batch details
    get_batch_by_id(pool, batch_id).await.map(Some)
}

/// Get batch by ID
///
/// # Errors
/// Returns error if database operation fails or batch not found
pub async fn get_batch_by_id(pool: &PgPool, batch_id: i32) -> Result<ProofBatch, sqlx::Error> {
    debug!("Getting batch: id={batch_id}");

    let row = sqlx::query!(
        r"
        SELECT id, previous_counter_value, final_counter_value, transaction_ids, 
               sindri_proof_id, proof_status, created_at, proven_at
        FROM proof_batches 
        WHERE id = $1
        ",
        batch_id
    )
    .fetch_one(pool)
    .await?;

    let batch = ProofBatch {
        id: row.id,
        previous_counter_value: row.previous_counter_value,
        final_counter_value: row.final_counter_value,
        transaction_ids: row.transaction_ids,
        sindri_proof_id: row.sindri_proof_id,
        proof_status: row.proof_status.unwrap_or_else(|| "pending".to_string()),
        created_at: row.created_at.unwrap_or_else(|| Utc::now()),
        proven_at: row.proven_at,
    };

    debug!(
        "Found batch: id={}, status={}",
        batch.id, batch.proof_status
    );
    Ok(batch)
}

/// Get all batches (paginated)
///
/// # Errors
/// Returns error if database operation fails
pub async fn get_all_batches(
    pool: &PgPool,
    limit: Option<i32>,
) -> Result<Vec<ProofBatch>, sqlx::Error> {
    let limit_val = limit.unwrap_or(50);
    debug!("Getting batches with limit: {limit_val}");

    let rows = sqlx::query!(
        r"
        SELECT id, previous_counter_value, final_counter_value, transaction_ids,
               sindri_proof_id, proof_status, created_at, proven_at
        FROM proof_batches 
        ORDER BY id DESC 
        LIMIT $1
        ",
        limit_val as i64
    )
    .fetch_all(pool)
    .await?;

    let batches: Vec<ProofBatch> = rows
        .into_iter()
        .map(|row| ProofBatch {
            id: row.id,
            previous_counter_value: row.previous_counter_value,
            final_counter_value: row.final_counter_value,
            transaction_ids: row.transaction_ids,
            sindri_proof_id: row.sindri_proof_id,
            proof_status: row.proof_status.unwrap_or_else(|| "pending".to_string()),
            created_at: row.created_at.unwrap_or_else(|| Utc::now()),
            proven_at: row.proven_at,
        })
        .collect();

    debug!("Found {} batches", batches.len());
    Ok(batches)
}

/// Update batch with Sindri proof ID and status
///
/// # Errors
/// Returns error if database operation fails
pub async fn update_batch_proof(
    pool: &PgPool,
    batch_id: i32,
    proof_id: &str,
    status: &str,
) -> Result<(), sqlx::Error> {
    debug!("Updating batch {batch_id} with proof {proof_id}, status: {status}");

    let proven_at = if status == "proven" {
        Some(Utc::now())
    } else {
        None
    };

    sqlx::query!(
        r"
        UPDATE proof_batches 
        SET sindri_proof_id = $1, proof_status = $2, proven_at = $3
        WHERE id = $4
        ",
        proof_id,
        status,
        proven_at,
        batch_id
    )
    .execute(pool)
    .await?;

    debug!("Batch updated successfully");
    Ok(())
}

// ============================================================================
// STATE FUNCTIONS
// ============================================================================

/// Get current counter value
///
/// # Errors
/// Returns error if database operation fails
pub async fn get_current_counter_value(pool: &PgPool) -> Result<i64, sqlx::Error> {
    debug!("Getting current counter value");

    let value: i64 = sqlx::query_scalar!("SELECT get_current_counter_value()")
        .fetch_one(pool)
        .await?
        .unwrap_or(0);

    debug!("Current counter value: {value}");
    Ok(value)
}

/// Get current counter state with latest Merkle root
///
/// # Errors
/// Returns error if database operation fails
pub async fn get_current_state(pool: &PgPool) -> Result<CounterState, sqlx::Error> {
    debug!("Getting current counter state");

    let counter_value = get_current_counter_value(pool).await?;

    // Get latest proven batch and its Merkle root
    let row = sqlx::query!(
        r"
        SELECT pb.id, ads.merkle_root
        FROM proof_batches pb
        LEFT JOIN ads_state_commits ads ON pb.id = ads.batch_id
        WHERE pb.proof_status = 'proven'
        ORDER BY pb.id DESC
        LIMIT 1
        "
    )
    .fetch_optional(pool)
    .await?;

    let (last_batch_id, merkle_root) = match row {
        Some(row) => (Some(row.id), Some(row.merkle_root)),
        None => (None, None),
    };

    let state = CounterState {
        counter_value,
        merkle_root,
        last_batch_id,
    };

    debug!(
        "Current state: counter={}, batch_id={:?}",
        state.counter_value, state.last_batch_id
    );
    Ok(state)
}

// ============================================================================
// ADS/MERKLE TREE FUNCTIONS
// ============================================================================

/// Store Merkle root for a batch
///
/// # Errors
/// Returns error if database operation fails
pub async fn store_ads_state_commit(
    pool: &PgPool,
    batch_id: i32,
    merkle_root: &[u8],
) -> Result<AdsStateCommit, sqlx::Error> {
    debug!("Storing ADS state commit for batch {batch_id}");

    let row = sqlx::query!(
        r"
        INSERT INTO ads_state_commits (batch_id, merkle_root)
        VALUES ($1, $2)
        RETURNING id, batch_id, merkle_root, created_at
        ",
        batch_id,
        merkle_root
    )
    .fetch_one(pool)
    .await?;

    let commit = AdsStateCommit {
        id: row.id,
        batch_id: row.batch_id,
        merkle_root: row.merkle_root,
        created_at: row.created_at.unwrap_or_else(|| Utc::now()),
    };

    debug!("ADS state commit stored: id={}", commit.id);
    Ok(commit)
}

/// Get contract submission data for a batch (public/private split)
///
/// # Errors
/// Returns error if database operation fails or batch not found
pub async fn get_contract_submission_data(
    pool: &PgPool,
    batch_id: i32,
) -> Result<ContractSubmissionData, sqlx::Error> {
    debug!("Getting contract submission data for batch {batch_id}");

    // Get batch details
    let batch = get_batch_by_id(pool, batch_id).await?;

    // Get previous and new Merkle roots
    let prev_root = if batch.previous_counter_value == 0 {
        // Genesis state - use empty root
        "0x0000000000000000000000000000000000000000000000000000000000000000".to_string()
    } else {
        // Get previous batch's Merkle root
        let prev_row = sqlx::query!(
            r"
            SELECT ads.merkle_root
            FROM proof_batches pb
            JOIN ads_state_commits ads ON pb.id = ads.batch_id
            WHERE pb.final_counter_value = $1 AND pb.proof_status = 'proven'
            ORDER BY pb.id DESC
            LIMIT 1
            ",
            batch.previous_counter_value
        )
        .fetch_optional(pool)
        .await?;

        match prev_row {
            Some(row) => format!("0x{}", hex::encode(row.merkle_root)),
            None => {
                "0x0000000000000000000000000000000000000000000000000000000000000000".to_string()
            }
        }
    };

    // Get new Merkle root
    let new_row = sqlx::query!(
        "SELECT merkle_root FROM ads_state_commits WHERE batch_id = $1",
        batch_id
    )
    .fetch_optional(pool)
    .await?;

    let new_root = match new_row {
        Some(row) => format!("0x{}", hex::encode(row.merkle_root)),
        None => return Err(sqlx::Error::RowNotFound),
    };

    // Get transaction amounts for the batch
    let transaction_amounts: Vec<i32> = sqlx::query!(
        "SELECT amount FROM incoming_transactions WHERE id = ANY($1) ORDER BY id",
        &batch.transaction_ids
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|row| row.amount)
    .collect();

    let data = ContractSubmissionData {
        public: ContractPublicData {
            prev_merkle_root: prev_root,
            new_merkle_root: new_root,
            zk_proof: batch
                .sindri_proof_id
                .unwrap_or_else(|| "pending".to_string()),
        },
        private: ContractPrivateData {
            prev_counter_value: batch.previous_counter_value,
            new_counter_value: batch.final_counter_value,
            transactions: transaction_amounts,
        },
    };

    debug!("Contract submission data prepared for batch {batch_id}");
    Ok(data)
}

/// Get proven batches that haven't been posted to the smart contract yet
///
/// # Errors
/// Returns error if database operation fails
pub async fn get_proven_unposted_batches(
    pool: &PgPool,
    limit: Option<i32>,
) -> Result<Vec<ProofBatch>, sqlx::Error> {
    let limit = limit.unwrap_or(10);
    debug!("Getting proven unposted batches with limit: {limit}");

    let rows = sqlx::query!(
        r"
        SELECT id, previous_counter_value, final_counter_value, transaction_ids,
               sindri_proof_id, proof_status, created_at, proven_at
        FROM proof_batches
        WHERE proof_status = 'proven' 
          AND posted_to_contract = FALSE
          AND sindri_proof_id IS NOT NULL
        ORDER BY id ASC
        LIMIT $1
        ",
        limit as i64
    )
    .fetch_all(pool)
    .await?;

    let batches: Vec<ProofBatch> = rows
        .into_iter()
        .map(|row| ProofBatch {
            id: row.id,
            previous_counter_value: row.previous_counter_value,
            final_counter_value: row.final_counter_value,
            transaction_ids: row.transaction_ids,
            sindri_proof_id: row.sindri_proof_id,
            proof_status: row.proof_status.unwrap_or_else(|| "pending".to_string()),
            created_at: row.created_at.unwrap_or_else(|| Utc::now()),
            proven_at: row.proven_at,
        })
        .collect();

    debug!("Found {} proven unposted batches", batches.len());
    Ok(batches)
}

/// Mark a batch as posted to the smart contract
///
/// # Errors
/// Returns error if database operation fails
pub async fn mark_batch_posted_to_contract(
    pool: &PgPool,
    batch_id: i32,
) -> Result<(), sqlx::Error> {
    debug!("Marking batch {batch_id} as posted to contract");

    let result = sqlx::query!(
        r"
        UPDATE proof_batches
        SET posted_to_contract = TRUE,
            posted_to_contract_at = NOW()
        WHERE id = $1 AND posted_to_contract = FALSE
        ",
        batch_id
    )
    .execute(pool)
    .await?;

    if result.rows_affected() != 1 {
        return Err(sqlx::Error::RowNotFound);
    }

    debug!("Successfully marked batch {batch_id} as posted to contract");
    Ok(())
}

/// Mark a batch as posted to the smart contract with transaction hash
///
/// # Errors
/// Returns error if database operation fails
pub async fn mark_batch_posted_to_contract_with_tx_hash(
    pool: &PgPool,
    batch_id: i32,
    transaction_hash: &str,
) -> Result<(), sqlx::Error> {
    debug!("Marking batch {batch_id} as posted to contract with tx hash: {transaction_hash}");

    let result = sqlx::query!(
        r"
        UPDATE proof_batches
        SET posted_to_contract = TRUE,
            posted_to_contract_at = NOW(),
            transaction_hash = $2
        WHERE id = $1 AND posted_to_contract = FALSE
        ",
        batch_id,
        transaction_hash
    )
    .execute(pool)
    .await?;

    if result.rows_affected() != 1 {
        return Err(sqlx::Error::RowNotFound);
    }

    debug!("Successfully marked batch {batch_id} as posted to contract with tx hash");
    Ok(())
}

/// Check for batches that may have been posted to contract but not marked in database
/// This helps identify potential synchronization issues
///
/// # Errors
/// Returns error if database query fails
pub async fn get_potential_stuck_batches(
    pool: &PgPool,
    limit: Option<i32>,
) -> Result<Vec<ProofBatch>, sqlx::Error> {
    let limit = limit.unwrap_or(10);
    debug!("Checking for potential stuck batches (limit: {})", limit);

    // Get batches that have been proven for a while but are not marked as posted
    let records = sqlx::query!(
        r"
        SELECT id, previous_counter_value, final_counter_value, transaction_ids,
               sindri_proof_id, proof_status, created_at, proven_at
        FROM proof_batches
        WHERE proof_status = 'proven' 
          AND posted_to_contract = FALSE
          AND sindri_proof_id IS NOT NULL
          AND proven_at IS NOT NULL
          AND proven_at < NOW() - INTERVAL '10 minutes'  -- Proven more than 10 minutes ago
        ORDER BY proven_at ASC
        LIMIT $1
        ",
        limit as i64
    )
    .fetch_all(pool)
    .await?;

    let batches: Vec<ProofBatch> = records
        .into_iter()
        .map(|row| ProofBatch {
            id: row.id,
            previous_counter_value: row.previous_counter_value,
            final_counter_value: row.final_counter_value,
            transaction_ids: row.transaction_ids,
            sindri_proof_id: row.sindri_proof_id,
            proof_status: row.proof_status.unwrap_or_else(|| "pending".to_string()),
            created_at: row.created_at.unwrap_or_else(|| Utc::now()),
            proven_at: row.proven_at,
        })
        .collect();

    if !batches.is_empty() {
        debug!("Found {} potentially stuck batches", batches.len());
    }

    Ok(batches)
}

// ============================================================================
// CCR DOMAIN DATABASE FUNCTIONS
// ============================================================================

/// Create a new identity (user registration)
pub async fn create_identity(
    pool: &PgPool,
    user_type: &str,
    email: &str,
) -> Result<Identity, sqlx::Error> {
    let id = uuid::Uuid::new_v4();
    let now = chrono::Utc::now();

    sqlx::query!(
        r#"
        INSERT INTO identities (id, user_type, email, created_at, updated_at, email_verified)
        VALUES ($1, $2, $3, $4, $4, $5)
        "#,
        id,
        user_type,
        email,
        now,
        false // email_verified starts as false
    )
    .execute(pool)
    .await?;

    let identity = Identity {
        id,
        user_type: user_type.to_string(),
        email: email.to_string(),
        kyc_status: "not_submitted".to_string(),
        kyc_submitted_at: None,
        kyc_verified_at: None,
        created_at: now,
        updated_at: now,
        password_hash: None,
        last_login_at: None,
        email_verified: Some(false),
        email_verification_token: None,
        password_reset_token: None,
        password_reset_expires_at: None,
        first_name: None,
        last_name: None,
        jurisdiction: None,
        wallet_address: None,
    };

    Ok(identity)
}

/// Create identity with password (for registration)
pub async fn create_identity_with_password(
    pool: &PgPool,
    user_type: &str,
    email: &str,
    password_hash: &str,
) -> Result<Identity, sqlx::Error> {
    let id = uuid::Uuid::new_v4();
    let now = chrono::Utc::now();

    sqlx::query!(
        r#"
        INSERT INTO identities (id, user_type, email, password_hash, created_at, updated_at, email_verified)
        VALUES ($1, $2, $3, $4, $5, $5, $6)
        "#,
        id,
        user_type,
        email,
        password_hash,
        now,
        false // email_verified starts as false
    )
    .execute(pool)
    .await?;

    let identity = Identity {
        id,
        user_type: user_type.to_string(),
        email: email.to_string(),
        kyc_status: "not_submitted".to_string(),
        kyc_submitted_at: None,
        kyc_verified_at: None,
        created_at: now,
        updated_at: now,
        password_hash: Some(password_hash.to_string()),
        last_login_at: None,
        email_verified: Some(false),
        email_verification_token: None,
        password_reset_token: None,
        password_reset_expires_at: None,
        first_name: None,
        last_name: None,
        jurisdiction: None,
        wallet_address: None,
    };

    Ok(identity)
}

/// Register or re-register user with soft delete handling
/// If email exists and user is deleted, undelete them with new password
/// If email exists and user is not deleted, return error
/// If email doesn't exist, create new user
pub async fn register_or_reregister_user(
    pool: &PgPool,
    user_type: &str,
    email: &str,
    password_hash: &str,
) -> Result<RegisterResult, sqlx::Error> {
    // First, check if email already exists
    let existing_user = sqlx::query!(
        "SELECT id, deleted_at FROM identities WHERE email = $1",
        email
    )
    .fetch_optional(pool)
    .await?;

    match existing_user {
        Some(user) if user.deleted_at.is_some() => {
            // User exists but is deleted - undelete them with new password
            let now = chrono::Utc::now();
            sqlx::query!(
                r#"
                UPDATE identities
                SET user_type = $1,
                    password_hash = $2,
                    deleted_at = NULL,
                    updated_at = $3,
                    email_verified = $4,
                    last_login_at = NULL,
                    email_verification_token = NULL,
                    password_reset_token = NULL,
                    password_reset_expires_at = NULL
                WHERE id = $5
                "#,
                user_type,
                password_hash,
                now,
                false, // email_verified
                user.id
            )
            .execute(pool)
            .await?;

            // Return the existing identity (now undeleted)
            let identity = sqlx::query!(
                r#"
                SELECT id, user_type, email, kyc_status, kyc_submitted_at, kyc_verified_at,
                       created_at as "created_at!", updated_at as "updated_at!",
                       password_hash, last_login_at, email_verified, email_verification_token,
                       password_reset_token, password_reset_expires_at, wallet_address,
                       first_name, last_name, jurisdiction
                FROM identities
                WHERE id = $1
                "#,
                user.id
            )
            .fetch_one(pool)
            .await?;

            let identity_struct = Identity {
                id: identity.id,
                user_type: identity.user_type,
                email: identity.email,
                kyc_status: identity.kyc_status,
                kyc_submitted_at: identity.kyc_submitted_at,
                kyc_verified_at: identity.kyc_verified_at,
                created_at: identity.created_at,
                updated_at: identity.updated_at,
                password_hash: identity.password_hash,
                last_login_at: identity.last_login_at,
                email_verified: identity.email_verified,
                email_verification_token: identity.email_verification_token,
                password_reset_token: identity.password_reset_token,
                password_reset_expires_at: identity.password_reset_expires_at,
                first_name: identity.first_name,
                last_name: identity.last_name,
                jurisdiction: identity.jurisdiction,
                wallet_address: identity.wallet_address,
            };

            Ok(RegisterResult::Reregistered(identity_struct))
        }
        Some(_) => {
            // User exists and is not deleted - cannot register
            Ok(RegisterResult::AlreadyExists)
        }
        None => {
            // Email doesn't exist - create new user
            let id = uuid::Uuid::new_v4();
            let now = chrono::Utc::now();

            sqlx::query!(
                r#"
                INSERT INTO identities (id, user_type, email, password_hash, created_at, updated_at, email_verified)
                VALUES ($1, $2, $3, $4, $5, $5, $6)
                "#,
                id,
                user_type,
                email,
                password_hash,
                now,
                false // email_verified starts as false
            )
            .execute(pool)
            .await?;

            let identity = Identity {
                id,
                user_type: user_type.to_string(),
                email: email.to_string(),
                kyc_status: "not_submitted".to_string(),
                kyc_submitted_at: None,
                kyc_verified_at: None,
                created_at: now,
                updated_at: now,
                password_hash: Some(password_hash.to_string()),
                last_login_at: None,
                email_verified: Some(false),
                email_verification_token: None,
                password_reset_token: None,
                password_reset_expires_at: None,
                first_name: None,
                last_name: None,
                jurisdiction: None,
                wallet_address: None,
            };

            Ok(RegisterResult::Created(identity))
        }
    }
}

/// Result type for user registration operations
#[derive(Debug)]
pub enum RegisterResult {
    Created(Identity),
    Reregistered(Identity),
    AlreadyExists,
}

/// Get identity by ID
pub async fn get_identity_by_id(
    pool: &PgPool,
    id: uuid::Uuid,
) -> Result<Option<Identity>, sqlx::Error> {
    let row = sqlx::query!(
        r#"
        SELECT id, user_type, email, kyc_status, kyc_submitted_at, kyc_verified_at,
               created_at as "created_at!", updated_at as "updated_at!",
               password_hash, last_login_at, email_verified, email_verification_token,
               password_reset_token, password_reset_expires_at, wallet_address,
               first_name, last_name, jurisdiction
        FROM identities
        WHERE id = $1 AND deleted_at IS NULL
        "#,
        id
    )
    .fetch_optional(pool)
    .await?;

    let identity = row.map(|row| Identity {
        id: row.id,
        user_type: row.user_type,
        email: row.email,
        kyc_status: row.kyc_status,
        kyc_submitted_at: row.kyc_submitted_at,
        kyc_verified_at: row.kyc_verified_at,
        created_at: row.created_at,
        updated_at: row.updated_at,
        password_hash: row.password_hash,
        last_login_at: row.last_login_at,
        email_verified: row.email_verified,
        email_verification_token: row.email_verification_token,
        password_reset_token: row.password_reset_token,
        password_reset_expires_at: row.password_reset_expires_at,
        first_name: row.first_name,
        last_name: row.last_name,
        jurisdiction: row.jurisdiction,
        wallet_address: row.wallet_address,
    });

    Ok(identity)
}

/// Get identity by email
pub async fn get_identity_by_email(
    pool: &PgPool,
    email: &str,
) -> Result<Option<Identity>, sqlx::Error> {
    let row = sqlx::query!(
        r#"
        SELECT id, user_type, email, kyc_status, kyc_submitted_at, kyc_verified_at,
               created_at as "created_at!", updated_at as "updated_at!",
               password_hash, last_login_at, email_verified, email_verification_token,
               password_reset_token, password_reset_expires_at, wallet_address,
               first_name, last_name, jurisdiction
        FROM identities
        WHERE email = $1 AND deleted_at IS NULL
        "#,
        email
    )
    .fetch_optional(pool)
    .await?;

    let identity = row.map(|row| Identity {
        id: row.id,
        user_type: row.user_type,
        email: row.email,
        kyc_status: row.kyc_status,
        kyc_submitted_at: row.kyc_submitted_at,
        kyc_verified_at: row.kyc_verified_at,
        created_at: row.created_at,
        updated_at: row.updated_at,
        password_hash: row.password_hash,
        last_login_at: row.last_login_at,
        email_verified: row.email_verified,
        email_verification_token: row.email_verification_token,
        password_reset_token: row.password_reset_token,
        password_reset_expires_at: row.password_reset_expires_at,
        first_name: row.first_name,
        last_name: row.last_name,
        jurisdiction: row.jurisdiction,
        wallet_address: row.wallet_address,
    });

    Ok(identity)
}

/// List identities with optional filtering
pub async fn list_identities(
    pool: &PgPool,
    limit: Option<i64>,
    offset: Option<i64>,
    _kyc_status: Option<&str>,
) -> Result<Vec<Identity>, sqlx::Error> {
    // Simplified version - can add filtering later
    let rows = sqlx::query!(
        r#"
        SELECT id, user_type, email, kyc_status, kyc_submitted_at, kyc_verified_at,
               created_at as "created_at!", updated_at as "updated_at!",
               password_hash, last_login_at, email_verified, email_verification_token,
               password_reset_token, password_reset_expires_at, wallet_address,
               first_name, last_name, jurisdiction
        FROM identities
        ORDER BY created_at DESC
        LIMIT $1 OFFSET $2
        "#,
        limit.unwrap_or(100),
        offset.unwrap_or(0)
    )
    .fetch_all(pool)
    .await?;

    let identities = rows
        .into_iter()
        .map(|row| Identity {
            id: row.id,
            user_type: row.user_type,
            email: row.email,
            kyc_status: row.kyc_status,
            kyc_submitted_at: row.kyc_submitted_at,
            kyc_verified_at: row.kyc_verified_at,
            created_at: row.created_at,
            updated_at: row.updated_at,
            password_hash: row.password_hash,
            last_login_at: row.last_login_at,
            email_verified: row.email_verified,
            email_verification_token: row.email_verification_token,
            password_reset_token: row.password_reset_token,
            password_reset_expires_at: row.password_reset_expires_at,
            first_name: row.first_name,
            last_name: row.last_name,
            jurisdiction: row.jurisdiction,
            wallet_address: row.wallet_address,
        })
        .collect();

    Ok(identities)
}

/// Submit KYC data for identity
pub async fn submit_kyc_data(
    pool: &PgPool,
    identity_id: uuid::Uuid,
    _document_type: &str,
    _document_hash: &str,
    _jurisdiction: Option<&str>,
) -> Result<Identity, sqlx::Error> {
    let now = Utc::now();

    // TODO: Replace this automatic approval with proper KYC validation service
    // This should integrate with:
    // - Government ID verification services (e.g., Jumio, Onfido)
    // - AML/KYC compliance checks
    // - Regulatory compliance workflows
    // - Manual review processes for high-value accounts
    //
    // For now, we automatically approve all submissions for demo purposes
    sqlx::query!(
        r#"
        UPDATE identities
        SET kyc_status = 'approved',
            kyc_submitted_at = $2,
            kyc_verified_at = $2,
            updated_at = $2
        WHERE id = $1
        "#,
        identity_id,
        now
    )
    .execute(pool)
    .await?;

    // Fetch the updated identity
    get_identity_by_id(pool, identity_id)
        .await?
        .ok_or_else(|| sqlx::Error::RowNotFound)
}

/// List identities with pending KYC status for admin review
pub async fn list_pending_kyc_applications(
    pool: &PgPool,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<Identity>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"
        SELECT id, user_type, email, kyc_status, kyc_submitted_at, kyc_verified_at,
               created_at as "created_at!", updated_at as "updated_at!",
               password_hash, last_login_at, email_verified, email_verification_token,
               password_reset_token, password_reset_expires_at, wallet_address,
               first_name, last_name, jurisdiction
        FROM identities
        WHERE kyc_status = 'pending'
        ORDER BY kyc_submitted_at ASC
        LIMIT $1 OFFSET $2
        "#,
        limit.unwrap_or(50),
        offset.unwrap_or(0)
    )
    .fetch_all(pool)
    .await?;

    let identities = rows
        .into_iter()
        .map(|row| Identity {
            id: row.id,
            user_type: row.user_type,
            email: row.email,
            kyc_status: row.kyc_status,
            kyc_submitted_at: row.kyc_submitted_at,
            kyc_verified_at: row.kyc_verified_at,
            created_at: row.created_at,
            updated_at: row.updated_at,
            password_hash: row.password_hash,
            last_login_at: row.last_login_at,
            email_verified: row.email_verified,
            email_verification_token: row.email_verification_token,
            password_reset_token: row.password_reset_token,
            password_reset_expires_at: row.password_reset_expires_at,
            first_name: row.first_name,
            last_name: row.last_name,
            jurisdiction: row.jurisdiction,
            wallet_address: row.wallet_address,
        })
        .collect();

    Ok(identities)
}

/// Get KYC statistics for admin dashboard
pub async fn get_kyc_statistics(pool: &PgPool) -> Result<KycStatistics, sqlx::Error> {
    let row = sqlx::query!(
        r#"
        SELECT
            COUNT(*) as total_users,
            COUNT(CASE WHEN kyc_status = 'approved' THEN 1 END) as approved_count,
            COUNT(CASE WHEN kyc_status = 'pending' THEN 1 END) as pending_count,
            COUNT(CASE WHEN kyc_status = 'rejected' THEN 1 END) as rejected_count,
            COUNT(CASE WHEN kyc_status = 'not_submitted' THEN 1 END) as not_submitted_count
        FROM identities
        "#,
    )
    .fetch_one(pool)
    .await?;

    Ok(KycStatistics {
        total_users: row.total_users.unwrap_or(0) as u64,
        approved_count: row.approved_count.unwrap_or(0) as u64,
        pending_count: row.pending_count.unwrap_or(0) as u64,
        rejected_count: row.rejected_count.unwrap_or(0) as u64,
        not_submitted_count: row.not_submitted_count.unwrap_or(0) as u64,
    })
}

/// Update KYC status (admin only)
pub async fn update_kyc_status(
    pool: &PgPool,
    identity_id: uuid::Uuid,
    status: &str,
    _notes: Option<&str>,
) -> Result<Identity, sqlx::Error> {
    let now = Utc::now();

    // If approving KYC, set verified timestamp
    let verified_at = if status == "approved" {
        Some(now)
    } else {
        None
    };

    sqlx::query!(
        r#"
        UPDATE identities
        SET kyc_status = $2,
            kyc_verified_at = $3,
            updated_at = $4
        WHERE id = $1
        "#,
        identity_id,
        status,
        verified_at,
        now
    )
    .execute(pool)
    .await?;

    // Fetch the updated identity
    get_identity_by_id(pool, identity_id)
        .await?
        .ok_or_else(|| sqlx::Error::RowNotFound)
}

/// Update identity profile information
pub async fn update_identity_profile(
    pool: &PgPool,
    identity_id: uuid::Uuid,
    email: Option<&str>,
    user_type: Option<&str>,
    jurisdiction: Option<&str>,
    first_name: Option<&str>,
    last_name: Option<&str>,
    wallet_address: Option<&str>,
) -> Result<Identity, sqlx::Error> {
    let now = Utc::now();

    // Update fields individually for simplicity

    // For now, use a simplified approach with individual queries
    if let Some(e) = email {
        sqlx::query!(
            r#"
            UPDATE identities 
            SET email = $2,
                updated_at = $3
            WHERE id = $1
            "#,
            identity_id,
            e,
            now
        )
        .execute(pool)
        .await?;
    }

    if let Some(ut) = user_type {
        sqlx::query!(
            r#"
            UPDATE identities
            SET user_type = $2,
                updated_at = $3
            WHERE id = $1
            "#,
            identity_id,
            ut,
            now
        )
        .execute(pool)
        .await?;
    }

    // Always update jurisdiction since we want to allow clearing it
    sqlx::query!(
        r#"
        UPDATE identities
        SET jurisdiction = $2,
            updated_at = $3
        WHERE id = $1
        "#,
        identity_id,
        jurisdiction,
        now
    )
    .execute(pool)
    .await?;

    if let Some(first_name_val) = first_name {
        sqlx::query!(
            r#"
            UPDATE identities
            SET first_name = $2,
                updated_at = $3
            WHERE id = $1
            "#,
            identity_id,
            first_name_val,
            now
        )
        .execute(pool)
        .await?;
    }

    if let Some(last_name_val) = last_name {
        sqlx::query!(
            r#"
            UPDATE identities
            SET last_name = $2,
                updated_at = $3
            WHERE id = $1
            "#,
            identity_id,
            last_name_val,
            now
        )
        .execute(pool)
        .await?;
    }

    if let Some(wa) = wallet_address {
        sqlx::query!(
            r#"
            UPDATE identities
            SET wallet_address = $2,
                updated_at = $3
            WHERE id = $1
            "#,
            identity_id,
            wa,
            now
        )
        .execute(pool)
        .await?;
    }

    // Always update timestamp
    sqlx::query!(
        r#"
        UPDATE identities 
        SET updated_at = $2
        WHERE id = $1
        "#,
        identity_id,
        now
    )
    .execute(pool)
    .await?;

    // Fetch the updated identity
    get_identity_by_id(pool, identity_id)
        .await?
        .ok_or_else(|| sqlx::Error::RowNotFound)
}

/// Delete identity by ID
pub async fn delete_identity(pool: &PgPool, identity_id: uuid::Uuid) -> Result<bool, sqlx::Error> {
    let result = sqlx::query!(
        r#"
        DELETE FROM identities
        WHERE id = $1
        "#,
        identity_id
    )
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

/// Create a new loan
pub async fn create_loan(
    pool: &PgPool,
    borrower_id: uuid::Uuid,
    lender_id: uuid::Uuid,
    principal_amount: rust_decimal::Decimal,
    interest_rate_bps: i32,
    maturity_date: Option<DateTime<Utc>>,
) -> Result<Loan, sqlx::Error> {
    let id = uuid::Uuid::new_v4();
    let now = Utc::now();

    sqlx::query!(
        r#"
        INSERT INTO loans (id, borrower_id, lender_id, principal_amount, outstanding_balance, interest_rate_bps, maturity_date, originated_at, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $4, $5, $6, $7, $7, $7)
        "#,
        id,
        borrower_id,
        lender_id,
        principal_amount,
        interest_rate_bps,
        maturity_date,
        now
    )
    .execute(pool)
    .await?;

    let loan = Loan {
        id,
        borrower_id,
        lender_id,
        principal_amount,
        outstanding_balance: principal_amount,
        interest_rate_bps,
        loan_status: "originated".to_string(),
        originated_at: now,
        maturity_date,
        created_at: now,
        updated_at: now,
    };

    Ok(loan)
}

/// Get loan by ID
pub async fn get_loan_by_id(pool: &PgPool, id: uuid::Uuid) -> Result<Option<Loan>, sqlx::Error> {
    let row = sqlx::query!(
        r#"
        SELECT 
            id,
            borrower_id,
            lender_id,
            principal_amount,
            outstanding_balance,
            interest_rate_bps,
            loan_status,
            originated_at,
            maturity_date,
            created_at,
            updated_at
        FROM loans 
        WHERE id = $1
        "#,
        id
    )
    .fetch_optional(pool)
    .await?;

    match row {
        Some(row) => Ok(Some(Loan {
            id: row.id,
            borrower_id: row.borrower_id,
            lender_id: row.lender_id,
            principal_amount: row.principal_amount,
            outstanding_balance: row.outstanding_balance,
            interest_rate_bps: row.interest_rate_bps,
            loan_status: row.loan_status,
            originated_at: row.originated_at.unwrap_or_else(|| chrono::Utc::now()),
            maturity_date: row.maturity_date,
            created_at: row.created_at.unwrap_or_else(|| chrono::Utc::now()),
            updated_at: row.updated_at.unwrap_or_else(|| chrono::Utc::now()),
        })),
        None => Ok(None),
    }
}

/// List loans with optional filtering
pub async fn list_loans(
    pool: &PgPool,
    limit: Option<i64>,
    offset: Option<i64>,
    status: Option<&str>,
    borrower_id: Option<uuid::Uuid>,
    lender_id: Option<uuid::Uuid>,
) -> Result<Vec<Loan>, sqlx::Error> {
    // Use a unified query with optional filtering
    let rows = sqlx::query!(
        r#"
        SELECT 
            id,
            borrower_id,
            lender_id,
            principal_amount,
            outstanding_balance,
            interest_rate_bps,
            loan_status,
            originated_at,
            maturity_date,
            created_at,
            updated_at
        FROM loans
        WHERE ($1::text IS NULL OR loan_status = $1)
          AND ($2::uuid IS NULL OR borrower_id = $2)
          AND ($3::uuid IS NULL OR lender_id = $3)
        ORDER BY created_at DESC
        LIMIT $4 OFFSET $5
        "#,
        status,
        borrower_id,
        lender_id,
        limit.unwrap_or(100),
        offset.unwrap_or(0)
    )
    .fetch_all(pool)
    .await?;

    let result: Vec<Loan> = rows
        .into_iter()
        .map(|row| Loan {
            id: row.id,
            borrower_id: row.borrower_id,
            lender_id: row.lender_id,
            principal_amount: row.principal_amount,
            outstanding_balance: row.outstanding_balance,
            interest_rate_bps: row.interest_rate_bps,
            loan_status: row.loan_status,
            originated_at: row.originated_at.unwrap_or_else(|| chrono::Utc::now()),
            maturity_date: row.maturity_date,
            created_at: row.created_at.unwrap_or_else(|| chrono::Utc::now()),
            updated_at: row.updated_at.unwrap_or_else(|| chrono::Utc::now()),
        })
        .collect();

    Ok(result)
}

/// Update loan details
pub async fn update_loan(
    pool: &PgPool,
    id: uuid::Uuid,
    principal_amount: Option<rust_decimal::Decimal>,
    outstanding_balance: Option<rust_decimal::Decimal>,
    interest_rate_bps: Option<i32>,
    loan_status: Option<&str>,
    maturity_date: Option<Option<DateTime<Utc>>>, // Option<Option<T>> to handle setting to NULL
) -> Result<Option<Loan>, sqlx::Error> {
    let now = Utc::now();

    // First check if the loan exists
    let existing_loan = get_loan_by_id(pool, id).await?;
    if existing_loan.is_none() {
        return Ok(None);
    }
    let existing_loan = existing_loan.unwrap();

    // Update only the provided fields
    let new_principal = principal_amount.unwrap_or(existing_loan.principal_amount);
    let new_outstanding = outstanding_balance.unwrap_or(existing_loan.outstanding_balance);
    let new_rate = interest_rate_bps.unwrap_or(existing_loan.interest_rate_bps);
    let new_status = loan_status.unwrap_or(&existing_loan.loan_status);
    let new_maturity = maturity_date.unwrap_or(existing_loan.maturity_date);

    sqlx::query!(
        r#"
        UPDATE loans 
        SET 
            principal_amount = $2,
            outstanding_balance = $3,
            interest_rate_bps = $4,
            loan_status = $5,
            maturity_date = $6,
            updated_at = $7
        WHERE id = $1
        "#,
        id,
        new_principal,
        new_outstanding,
        new_rate,
        new_status,
        new_maturity,
        now
    )
    .execute(pool)
    .await?;

    // Return the updated loan
    get_loan_by_id(pool, id).await
}

/// Record a payment - Simple implementation for now
pub async fn create_payment(
    pool: &PgPool,
    loan_id: uuid::Uuid,
    payer_id: uuid::Uuid,
    amount: rust_decimal::Decimal,
    payment_type: &str,
    transaction_hash: Option<&str>,
) -> Result<Payment, sqlx::Error> {
    let id = uuid::Uuid::new_v4();
    let now = Utc::now();

    sqlx::query!(
        r#"
        INSERT INTO payments (id, loan_id, payer_id, amount, payment_type, transaction_hash, processed_at, created_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $7)
        "#,
        id,
        loan_id,
        payer_id,
        amount,
        payment_type,
        transaction_hash,
        now
    )
    .execute(pool)
    .await?;

    let payment = Payment {
        id,
        loan_id,
        payer_id,
        amount,
        payment_type: payment_type.to_string(),
        transaction_hash: transaction_hash.map(|s| s.to_string()),
        processed_at: now,
        created_at: now,
    };

    Ok(payment)
}

/// Get payments for a loan - Simple implementation for now
pub async fn get_payments_by_loan(
    _pool: &PgPool,
    _loan_id: uuid::Uuid,
    _limit: Option<i64>,
    _offset: Option<i64>,
) -> Result<Vec<Payment>, sqlx::Error> {
    // For now, return empty - can be implemented later
    Ok(vec![])
}

/// Get balance for an identity - Simple implementation for now
pub async fn get_balance(
    _pool: &PgPool,
    _identity_id: uuid::Uuid,
    _balance_type: &str,
    _currency: &str,
) -> Result<Option<Balance>, sqlx::Error> {
    // For now, return None - can be implemented later
    Ok(None)
}

/// Update balance - Simple implementation for now
pub async fn update_balance(
    _pool: &PgPool,
    identity_id: uuid::Uuid,
    balance_type: &str,
    currency: &str,
    new_amount: rust_decimal::Decimal,
) -> Result<Balance, sqlx::Error> {
    let id = uuid::Uuid::new_v4();
    let now = Utc::now();

    let balance = Balance {
        id,
        identity_id,
        balance_type: balance_type.to_string(),
        amount: new_amount,
        currency: currency.to_string(),
        updated_at: now,
        created_at: now,
    };

    Ok(balance)
}

/// Create a state snapshot - Simple implementation for now
pub async fn create_state_snapshot(
    _pool: &PgPool,
    snapshot_type: &str,
    batch_id: Option<i32>,
) -> Result<StateSnapshot, sqlx::Error> {
    let id = uuid::Uuid::new_v4();
    let now = Utc::now();

    let snapshot = StateSnapshot {
        id,
        snapshot_type: snapshot_type.to_string(),
        batch_id,
        merkle_root: None,
        total_loans_count: 0,
        total_outstanding_balance: rust_decimal::Decimal::ZERO,
        total_paid_amount: rust_decimal::Decimal::ZERO,
        active_users_count: 0,
        kyc_approved_count: 0,
        created_at: now,
    };

    Ok(snapshot)
}

// ============================================================================
// AUTHENTICATION HELPER FUNCTIONS
// ============================================================================

/// Update last login timestamp for a user
pub async fn update_last_login(pool: &PgPool, user_id: uuid::Uuid) -> Result<(), sqlx::Error> {
    let now = Utc::now();

    sqlx::query!(
        r#"
        UPDATE identities 
        SET last_login_at = $1, updated_at = $1
        WHERE id = $2
        "#,
        now,
        user_id
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Set password for a user (for password updates)
pub async fn update_user_password(
    pool: &PgPool,
    user_id: uuid::Uuid,
    password_hash: &str,
) -> Result<(), sqlx::Error> {
    let now = Utc::now();

    sqlx::query!(
        r#"
        UPDATE identities 
        SET password_hash = $1, updated_at = $2
        WHERE id = $3
        "#,
        password_hash,
        now,
        user_id
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Set email verification token for a user
pub async fn set_email_verification_token(
    pool: &PgPool,
    user_id: uuid::Uuid,
    token: &str,
) -> Result<(), sqlx::Error> {
    let now = Utc::now();

    sqlx::query!(
        r#"
        UPDATE identities 
        SET email_verification_token = $1, updated_at = $2
        WHERE id = $3
        "#,
        token,
        now,
        user_id
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Verify email address using verification token
pub async fn verify_email_with_token(pool: &PgPool, token: &str) -> Result<bool, sqlx::Error> {
    let now = Utc::now();

    let result = sqlx::query!(
        r#"
        UPDATE identities 
        SET email_verified = true, email_verification_token = NULL, updated_at = $1
        WHERE email_verification_token = $2
          AND created_at > $3  -- Token expires after 7 days
        "#,
        now,
        token,
        now - chrono::Duration::days(7)
    )
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

/// Set password reset token with expiration
pub async fn set_password_reset_token(
    pool: &PgPool,
    email: &str,
    token: &str,
    expires_in_hours: i64,
) -> Result<bool, sqlx::Error> {
    let now = Utc::now();
    let expires_at = now + chrono::Duration::hours(expires_in_hours);

    let result = sqlx::query!(
        r#"
        UPDATE identities 
        SET password_reset_token = $1, 
            password_reset_expires_at = $2, 
            updated_at = $3
        WHERE email = $4
        "#,
        token,
        expires_at,
        now,
        email
    )
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

/// Reset password using reset token
pub async fn reset_password_with_token(
    pool: &PgPool,
    token: &str,
    new_password_hash: &str,
) -> Result<bool, sqlx::Error> {
    let now = Utc::now();

    let result = sqlx::query!(
        r#"
        UPDATE identities 
        SET password_hash = $1, 
            password_reset_token = NULL, 
            password_reset_expires_at = NULL,
            updated_at = $2
        WHERE password_reset_token = $3
          AND password_reset_expires_at > $2
        "#,
        new_password_hash,
        now,
        token
    )
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

/// Get identity by password reset token (if valid)
pub async fn get_identity_by_reset_token(
    pool: &PgPool,
    token: &str,
) -> Result<Option<Identity>, sqlx::Error> {
    let now = Utc::now();

    let row = sqlx::query!(
        r#"
        SELECT id, user_type, email, kyc_status, kyc_submitted_at, kyc_verified_at,
               created_at as "created_at!", updated_at as "updated_at!",
               password_hash, last_login_at, email_verified, email_verification_token,
               password_reset_token, password_reset_expires_at, wallet_address,
               first_name, last_name, jurisdiction
        FROM identities
        WHERE password_reset_token = $1
          AND password_reset_expires_at > $2
        "#,
        token,
        now
    )
    .fetch_optional(pool)
    .await?;

    let identity = row.map(|row| Identity {
        id: row.id,
        user_type: row.user_type,
        email: row.email,
        kyc_status: row.kyc_status,
        kyc_submitted_at: row.kyc_submitted_at,
        kyc_verified_at: row.kyc_verified_at,
        created_at: row.created_at,
        updated_at: row.updated_at,
        password_hash: row.password_hash,
        last_login_at: row.last_login_at,
        email_verified: row.email_verified,
        email_verification_token: row.email_verification_token,
        password_reset_token: row.password_reset_token,
        password_reset_expires_at: row.password_reset_expires_at,
        first_name: row.first_name,
        last_name: row.last_name,
        jurisdiction: row.jurisdiction,
        wallet_address: row.wallet_address,
    });

    Ok(identity)
}

// ============================================================================
// INVITE CODE FUNCTIONS
// ============================================================================

/// Check if an invite code is valid and available for use
///
/// # Errors
/// Returns error if database operation fails
/// Invite code information structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InviteCodeInfo {
    pub is_valid: bool,
    pub user_type: Option<String>, // 'admin' or 'regular'
    pub expires_at: Option<DateTime<Utc>>,
    pub description: Option<String>,
}

pub async fn check_invite_code(pool: &PgPool, code: &str) -> Result<bool, sqlx::Error> {
    debug!("Checking invite code validity: code={}", code);

    let is_valid: bool = sqlx::query_scalar!("SELECT is_invite_code_valid($1)", code)
        .fetch_one(pool)
        .await?
        .unwrap_or(false);

    debug!("Invite code {} is valid: {}", code, is_valid);
    Ok(is_valid)
}

/// Get detailed invite code information including user_type
pub async fn get_invite_code_info(
    pool: &PgPool,
    code: &str,
) -> Result<InviteCodeInfo, sqlx::Error> {
    debug!("Getting invite code info: code={}", code);

    let result = sqlx::query!(
        "SELECT is_valid, user_type, expires_at, description FROM get_invite_code_info($1)",
        code
    )
    .fetch_one(pool)
    .await?;

    let invite_info = InviteCodeInfo {
        is_valid: result.is_valid.unwrap_or(false),
        user_type: result.user_type,
        expires_at: result.expires_at,
        description: result.description,
    };

    debug!("Invite code {} info: {:?}", code, invite_info);
    Ok(invite_info)
}

/// Use an invite code (marks it as used and decrements available uses)
pub async fn use_invite_code(
    pool: &PgPool,
    code: &str,
    user_id: Option<uuid::Uuid>,
) -> Result<bool, sqlx::Error> {
    debug!("Using invite code: code={}, user_id={:?}", code, user_id);

    let result = sqlx::query!("SELECT * FROM use_invite_code($1, $2)", code, user_id)
        .fetch_optional(pool)
        .await?;

    let used_successfully = result.is_some();
    debug!(
        "Invite code {} used successfully: {}",
        code, used_successfully
    );
    Ok(used_successfully)
}

/// Generate a new invite code
pub async fn generate_invite_code(
    pool: &PgPool,
    created_by: uuid::Uuid,
    max_uses: Option<i32>,
    expires_in_hours: Option<i32>,
    user_type: Option<String>, // 'admin' or 'regular'
) -> Result<String, sqlx::Error> {
    let user_type = user_type.unwrap_or_else(|| "regular".to_string());
    debug!(
        "Generating new invite code for user: {} (user_type: {})",
        created_by, user_type
    );

    // Validate user_type
    if user_type != "admin" && user_type != "regular" {
        return Err(sqlx::Error::Protocol(format!(
            "Invalid user_type '{}'. Must be 'admin' or 'regular'",
            user_type
        )));
    }

    // Generate a secure random code
    let code = generate_secure_invite_code();
    let max_uses = max_uses.unwrap_or(1);
    let expires_at = if let Some(hours) = expires_in_hours {
        Some(chrono::Utc::now() + chrono::Duration::hours(hours as i64))
    } else {
        // Default to 7 days
        Some(chrono::Utc::now() + chrono::Duration::days(7))
    };

    sqlx::query!(
        r#"
        INSERT INTO invite_codes (code, created_by, max_uses, expires_at, is_active, user_type) 
        VALUES ($1, $2, $3, $4, true, $5)
        "#,
        code,
        created_by,
        max_uses,
        expires_at,
        user_type
    )
    .execute(pool)
    .await?;

    debug!(
        "Generated invite code: {} for user: {} ({})",
        code, created_by, user_type
    );
    Ok(code)
}

/// Generate a secure random invite code
fn generate_secure_invite_code() -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    const CODE_LENGTH: usize = 6;

    let mut rng = rand::rng();
    (0..CODE_LENGTH)
        .map(|_| {
            let idx = rng.random_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

// ============================================================================
// USDC DEPOSIT MANAGEMENT FUNCTIONS
// ============================================================================

/// Create a new USDC deposit record
pub async fn create_usdc_deposit(
    pool: &PgPool,
    user_address: &str,
    amount: rust_decimal::Decimal,
    transaction_hash: &str,
    event_transaction_hash: &str,
    block_number: i64,
    event_timestamp: i64,
    log_index: i64,
) -> Result<UsdcDeposit, sqlx::Error> {
    let deposit = sqlx::query_as!(
        UsdcDeposit,
        r#"
        INSERT INTO usdc_deposits (
            user_address, amount, transaction_hash, event_transaction_hash,
            block_number, event_timestamp, log_index, event_signature
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        RETURNING 
            id, user_address, amount, deposit_amount_usdc, transaction_hash,
            event_transaction_hash, block_number, event_timestamp, block_timestamp,
            log_index, event_signature, status, confirmation_count, required_confirmations,
            created_at, updated_at, processed_at
        "#,
        user_address,
        amount,
        transaction_hash,
        event_transaction_hash,
        block_number,
        event_timestamp,
        log_index,
        "0x..." // Default event signature
    )
    .fetch_one(pool)
    .await?;

    Ok(deposit)
}

/// Update deposit status and confirmation count
pub async fn update_deposit_status(
    pool: &PgPool,
    deposit_id: i32,
    status: &str,
    confirmation_count: i32,
    block_timestamp: Option<DateTime<Utc>>,
) -> Result<UsdcDeposit, sqlx::Error> {
    let should_set_processed = status == "processed" || status == "confirmed";

    let deposit = sqlx::query_as!(
        UsdcDeposit,
        r#"
        UPDATE usdc_deposits 
        SET status = $2, confirmation_count = $3, block_timestamp = $4,
            processed_at = CASE WHEN $5 THEN NOW() ELSE processed_at END,
            updated_at = NOW()
        WHERE id = $1
        RETURNING 
            id, user_address, amount, deposit_amount_usdc, transaction_hash,
            event_transaction_hash, block_number, event_timestamp, block_timestamp,
            log_index, event_signature, status, confirmation_count, required_confirmations,
            created_at, updated_at, processed_at
        "#,
        deposit_id,
        status,
        confirmation_count,
        block_timestamp,
        should_set_processed
    )
    .fetch_one(pool)
    .await?;

    Ok(deposit)
}

/// Get deposits by user address
pub async fn get_deposits_by_user_address(
    pool: &PgPool,
    user_address: &str,
) -> Result<Vec<UsdcDeposit>, sqlx::Error> {
    let deposits = sqlx::query_as!(
        UsdcDeposit,
        r#"
        SELECT 
            id, user_address, amount, deposit_amount_usdc, transaction_hash,
            event_transaction_hash, block_number, event_timestamp, block_timestamp,
            log_index, event_signature, status, confirmation_count, required_confirmations,
            created_at, updated_at, processed_at
        FROM usdc_deposits
        WHERE user_address = $1
        ORDER BY created_at DESC
        "#,
        user_address
    )
    .fetch_all(pool)
    .await?;

    Ok(deposits)
}

/// Get deposit by transaction hash
pub async fn get_deposit_by_tx_hash(
    pool: &PgPool,
    tx_hash: &str,
) -> Result<Option<UsdcDeposit>, sqlx::Error> {
    let deposit = sqlx::query_as!(
        UsdcDeposit,
        r#"
        SELECT 
            id, user_address, amount, deposit_amount_usdc, transaction_hash,
            event_transaction_hash, block_number, event_timestamp, block_timestamp,
            log_index, event_signature, status, confirmation_count, required_confirmations,
            created_at, updated_at, processed_at
        FROM usdc_deposits
        WHERE transaction_hash = $1 OR event_transaction_hash = $1
        "#,
        tx_hash
    )
    .fetch_optional(pool)
    .await?;

    Ok(deposit)
}

/// Get user deposit balance by wallet address
pub async fn get_user_deposit_balance(
    pool: &PgPool,
    user_address: &str,
) -> Result<Option<UserDepositBalance>, sqlx::Error> {
    let balance = sqlx::query_as!(
        UserDepositBalance,
        r#"
        SELECT 
            id, user_address, total_deposited, total_deposited_usdc,
            deposit_count, first_deposit_at, last_deposit_at, created_at, updated_at
        FROM user_deposit_balances
        WHERE user_address = $1
        "#,
        user_address
    )
    .fetch_optional(pool)
    .await?;

    Ok(balance)
}

/// Get user balance from user_balances table by user_id
pub async fn get_user_balance_by_id(
    pool: &PgPool,
    user_id: uuid::Uuid,
) -> Result<Option<rust_decimal::Decimal>, sqlx::Error> {
    let result = sqlx::query!(
        r#"
        SELECT available_balance
        FROM user_balances
        WHERE user_id = $1 AND currency = 'USDC'
        "#,
        user_id
    )
    .fetch_optional(pool)
    .await?;

    Ok(result.map(|row| row.available_balance))
}

/// Get user balance from user_balances table by wallet address
pub async fn get_user_balance_by_address(
    pool: &PgPool,
    wallet_address: &str,
) -> Result<Option<rust_decimal::Decimal>, sqlx::Error> {
    let result = sqlx::query!(
        r#"
        SELECT ub.available_balance
        FROM user_balances ub
        JOIN identities i ON ub.user_id = i.id
        WHERE i.wallet_address = $1 AND ub.currency = 'USDC'
        "#,
        wallet_address
    )
    .fetch_optional(pool)
    .await?;

    Ok(result.map(|row| row.available_balance))
}

/// Update user balance after confirmed deposit
pub async fn update_user_balance_from_deposit(
    pool: &PgPool,
    user_id: uuid::Uuid,
    deposit_amount: rust_decimal::Decimal,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        UPDATE user_balances 
        SET available_balance = available_balance + $2,
            lifetime_earnings = lifetime_earnings + $2,
            last_updated_at = NOW()
        WHERE user_id = $1 AND currency = 'USDC'
        "#,
        user_id,
        deposit_amount
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Get user ID by wallet address
pub async fn get_user_id_by_wallet_address(
    pool: &PgPool,
    wallet_address: &str,
) -> Result<Option<uuid::Uuid>, sqlx::Error> {
    let result = sqlx::query!(
        r#"
        SELECT id
        FROM identities
        WHERE wallet_address = $1
        "#,
        wallet_address
    )
    .fetch_optional(pool)
    .await?;

    Ok(result.map(|row| row.id))
}

/// Transactional version: Update deposit status within a transaction
pub async fn update_deposit_status_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    deposit_id: i32,
    status: &str,
    confirmation_count: i32,
    block_timestamp: Option<DateTime<Utc>>,
) -> Result<UsdcDeposit, sqlx::Error> {
    let should_set_processed = status == "processed" || status == "confirmed";

    let deposit = sqlx::query_as!(
        UsdcDeposit,
        r#"
        UPDATE usdc_deposits 
        SET status = $2, confirmation_count = $3, block_timestamp = $4,
            processed_at = CASE WHEN $5 THEN NOW() ELSE processed_at END,
            updated_at = NOW()
        WHERE id = $1
        RETURNING 
            id, user_address, amount, deposit_amount_usdc, transaction_hash,
            event_transaction_hash, block_number, event_timestamp, block_timestamp,
            log_index, event_signature, status, confirmation_count, required_confirmations,
            created_at, updated_at, processed_at
        "#,
        deposit_id,
        status,
        confirmation_count,
        block_timestamp,
        should_set_processed
    )
    .fetch_one(&mut **tx)
    .await?;

    Ok(deposit)
}

/// Transactional version: Update user balance from deposit within a transaction  
pub async fn update_user_balance_from_deposit_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: uuid::Uuid,
    deposit_amount: rust_decimal::Decimal,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        UPDATE user_balances 
        SET available_balance = available_balance + $2,
            lifetime_earnings = lifetime_earnings + $2,
            last_updated_at = NOW()
        WHERE user_id = $1 AND currency = 'USDC'
        "#,
        user_id,
        deposit_amount
    )
    .execute(&mut **tx)
    .await?;

    Ok(())
}

// ============================================================================
// USDC WITHDRAWAL FUNCTIONS
// ============================================================================

/// Create a new USDC withdrawal record
pub async fn create_withdrawal(
    pool: &PgPool,
    user_address: &str,
    recipient_address: &str,
    amount: rust_decimal::Decimal,
    transaction_hash: &str,
    event_transaction_hash: &str,
    block_number: i64,
    event_timestamp: i64,
    log_index: i64,
    required_confirmations: i32,
) -> Result<UsdcWithdrawal, sqlx::Error> {
    let withdrawal = sqlx::query_as!(
        UsdcWithdrawal,
        r#"
        INSERT INTO usdc_withdrawals (
            user_address, recipient_address, amount, transaction_hash, event_transaction_hash,
            block_number, event_timestamp, log_index, required_confirmations,
            status, confirmation_count, created_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'pending', 0, NOW())
        ON CONFLICT (event_transaction_hash, log_index)
        DO UPDATE SET updated_at = NOW()
        RETURNING 
            id, user_address, recipient_address, amount, withdrawal_amount_usdc, transaction_hash,
            event_transaction_hash, block_number, event_timestamp, block_timestamp,
            log_index, event_signature, status, confirmation_count, required_confirmations,
            created_at, updated_at, processed_at
        "#,
        user_address,
        recipient_address,
        amount,
        transaction_hash,
        event_transaction_hash,
        block_number,
        event_timestamp,
        log_index,
        required_confirmations
    )
    .fetch_one(pool)
    .await?;

    Ok(withdrawal)
}

/// Update withdrawal status and confirmation count
pub async fn update_withdrawal_status(
    pool: &PgPool,
    withdrawal_id: i32,
    status: &str,
    confirmation_count: i32,
    block_timestamp: Option<DateTime<Utc>>,
) -> Result<UsdcWithdrawal, sqlx::Error> {
    let should_set_processed = status == "processed" || status == "confirmed";

    let withdrawal = sqlx::query_as!(
        UsdcWithdrawal,
        r#"
        UPDATE usdc_withdrawals 
        SET status = $2, confirmation_count = $3, block_timestamp = $4,
            processed_at = CASE WHEN $5 THEN NOW() ELSE processed_at END,
            updated_at = NOW()
        WHERE id = $1
        RETURNING 
            id, user_address, recipient_address, amount, withdrawal_amount_usdc, transaction_hash,
            event_transaction_hash, block_number, event_timestamp, block_timestamp,
            log_index, event_signature, status, confirmation_count, required_confirmations,
            created_at, updated_at, processed_at
        "#,
        withdrawal_id,
        status,
        confirmation_count,
        block_timestamp,
        should_set_processed
    )
    .fetch_one(pool)
    .await?;

    Ok(withdrawal)
}

/// Transactional version: Update withdrawal status within a transaction
pub async fn update_withdrawal_status_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    withdrawal_id: i32,
    status: &str,
    confirmation_count: i32,
    block_timestamp: Option<DateTime<Utc>>,
) -> Result<UsdcWithdrawal, sqlx::Error> {
    let should_set_processed = status == "processed" || status == "confirmed";

    let withdrawal = sqlx::query_as!(
        UsdcWithdrawal,
        r#"
        UPDATE usdc_withdrawals 
        SET status = $2, confirmation_count = $3, block_timestamp = $4,
            processed_at = CASE WHEN $5 THEN NOW() ELSE processed_at END,
            updated_at = NOW()
        WHERE id = $1
        RETURNING 
            id, user_address, recipient_address, amount, withdrawal_amount_usdc, transaction_hash,
            event_transaction_hash, block_number, event_timestamp, block_timestamp,
            log_index, event_signature, status, confirmation_count, required_confirmations,
            created_at, updated_at, processed_at
        "#,
        withdrawal_id,
        status,
        confirmation_count,
        block_timestamp,
        should_set_processed
    )
    .fetch_one(&mut **tx)
    .await?;

    Ok(withdrawal)
}

/// Get withdrawals by user address
pub async fn get_withdrawals_by_user_address(
    pool: &PgPool,
    user_address: &str,
) -> Result<Vec<UsdcWithdrawal>, sqlx::Error> {
    let withdrawals = sqlx::query_as!(
        UsdcWithdrawal,
        r#"
        SELECT 
            id, user_address, recipient_address, amount, withdrawal_amount_usdc, transaction_hash,
            event_transaction_hash, block_number, event_timestamp, block_timestamp,
            log_index, event_signature, status, confirmation_count, required_confirmations,
            created_at, updated_at, processed_at
        FROM usdc_withdrawals
        WHERE user_address = $1
        ORDER BY created_at DESC
        "#,
        user_address
    )
    .fetch_all(pool)
    .await?;

    Ok(withdrawals)
}

/// Get withdrawal by transaction hash
pub async fn get_withdrawal_by_tx_hash(
    pool: &PgPool,
    tx_hash: &str,
) -> Result<Option<UsdcWithdrawal>, sqlx::Error> {
    let withdrawal = sqlx::query_as!(
        UsdcWithdrawal,
        r#"
        SELECT 
            id, user_address, recipient_address, amount, withdrawal_amount_usdc, transaction_hash,
            event_transaction_hash, block_number, event_timestamp, block_timestamp,
            log_index, event_signature, status, confirmation_count, required_confirmations,
            created_at, updated_at, processed_at
        FROM usdc_withdrawals
        WHERE transaction_hash = $1 OR event_transaction_hash = $1
        "#,
        tx_hash
    )
    .fetch_optional(pool)
    .await?;

    Ok(withdrawal)
}

/// Get user withdrawal balance by wallet address
pub async fn get_user_withdrawal_balance(
    pool: &PgPool,
    user_address: &str,
) -> Result<Option<UserWithdrawalBalance>, sqlx::Error> {
    let balance = sqlx::query_as!(
        UserWithdrawalBalance,
        r#"
        SELECT 
            id, user_address, total_withdrawn, total_withdrawn_usdc, withdrawal_count,
            first_withdrawal_at, last_withdrawal_at, created_at, updated_at
        FROM user_withdrawal_balances
        WHERE user_address = $1
        "#,
        user_address
    )
    .fetch_optional(pool)
    .await?;

    Ok(balance)
}

/// Update user balance after withdrawal (debit)
pub async fn update_user_balance_from_withdrawal(
    pool: &PgPool,
    user_id: uuid::Uuid,
    withdrawal_amount: rust_decimal::Decimal,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        UPDATE user_balances 
        SET available_balance = available_balance - $2,
            last_updated_at = NOW()
        WHERE user_id = $1 AND currency = 'USDC'
        "#,
        user_id,
        withdrawal_amount
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Transactional version: Update user balance from withdrawal within a transaction
pub async fn update_user_balance_from_withdrawal_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: uuid::Uuid,
    withdrawal_amount: rust_decimal::Decimal,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        UPDATE user_balances 
        SET available_balance = available_balance - $2,
            last_updated_at = NOW()
        WHERE user_id = $1 AND currency = 'USDC'
        "#,
        user_id,
        withdrawal_amount
    )
    .execute(&mut **tx)
    .await?;

    Ok(())
}

// ============================================================================
// WITHDRAWAL REQUEST FUNCTIONS
// ============================================================================

/// Create a new withdrawal request
pub async fn create_withdrawal_request(
    pool: &PgPool,
    user_id: uuid::Uuid,
    user_address: &str,
    recipient_address: &str,
    amount: rust_decimal::Decimal,
    kyc_status_at_request: &str,
    available_balance_at_request: rust_decimal::Decimal,
) -> Result<WithdrawalRequest, sqlx::Error> {
    let request = sqlx::query_as!(
        WithdrawalRequest,
        r#"
        INSERT INTO withdrawal_requests (
            user_id, user_address, recipient_address, amount, state,
            kyc_status_at_request, available_balance_at_request,
            created_at, updated_at
        ) VALUES ($1, $2, $3, $4, 'pending', $5, $6, NOW(), NOW())
        RETURNING 
            id, user_id, user_address, recipient_address, amount, amount_usdc, 
            state, kyc_validated, balance_validated, kyc_status_at_request,
            available_balance_at_request, transaction_hash, block_number,
            confirmation_count, required_confirmations, error_message,
            validation_errors, created_at, updated_at, validated_at,
            submitted_at, confirmed_at, failed_at, cancelled_at
        "#,
        user_id,
        user_address,
        recipient_address,
        amount,
        kyc_status_at_request,
        available_balance_at_request
    )
    .fetch_one(pool)
    .await?;

    Ok(request)
}

/// Get withdrawal request by ID
pub async fn get_withdrawal_request_by_id(
    pool: &PgPool,
    request_id: i32,
) -> Result<WithdrawalRequest, sqlx::Error> {
    let request = sqlx::query_as!(
        WithdrawalRequest,
        r#"
        SELECT 
            id, user_id, user_address, recipient_address, amount, amount_usdc,
            state, kyc_validated, balance_validated, kyc_status_at_request,
            available_balance_at_request, transaction_hash, block_number,
            confirmation_count, required_confirmations, error_message,
            validation_errors, created_at, updated_at, validated_at,
            submitted_at, confirmed_at, failed_at, cancelled_at
        FROM withdrawal_requests
        WHERE id = $1
        "#,
        request_id
    )
    .fetch_one(pool)
    .await?;

    Ok(request)
}

/// Get withdrawal requests by user address
pub async fn get_withdrawal_requests_by_user_address(
    pool: &PgPool,
    user_address: &str,
) -> Result<Vec<WithdrawalRequest>, sqlx::Error> {
    let requests = sqlx::query_as!(
        WithdrawalRequest,
        r#"
        SELECT 
            id, user_id, user_address, recipient_address, amount, amount_usdc,
            state, kyc_validated, balance_validated, kyc_status_at_request,
            available_balance_at_request, transaction_hash, block_number,
            confirmation_count, required_confirmations, error_message,
            validation_errors, created_at, updated_at, validated_at,
            submitted_at, confirmed_at, failed_at, cancelled_at
        FROM withdrawal_requests
        WHERE user_address = $1
        ORDER BY created_at DESC
        "#,
        user_address
    )
    .fetch_all(pool)
    .await?;

    Ok(requests)
}

/// Get withdrawal requests by user ID
pub async fn get_withdrawal_requests_by_user_id(
    pool: &PgPool,
    user_id: uuid::Uuid,
) -> Result<Vec<WithdrawalRequest>, sqlx::Error> {
    let requests = sqlx::query_as!(
        WithdrawalRequest,
        r#"
        SELECT 
            id, user_id, user_address, recipient_address, amount, amount_usdc,
            state, kyc_validated, balance_validated, kyc_status_at_request,
            available_balance_at_request, transaction_hash, block_number,
            confirmation_count, required_confirmations, error_message,
            validation_errors, created_at, updated_at, validated_at,
            submitted_at, confirmed_at, failed_at, cancelled_at
        FROM withdrawal_requests
        WHERE user_id = $1
        ORDER BY created_at DESC
        "#,
        user_id
    )
    .fetch_all(pool)
    .await?;

    Ok(requests)
}

/// Update withdrawal request state
pub async fn update_withdrawal_request_state(
    pool: &PgPool,
    request_id: i32,
    new_state: &str,
    error_message: Option<&str>,
) -> Result<WithdrawalRequest, sqlx::Error> {
    let updated_at = chrono::Utc::now();

    // Set failed_at or cancelled_at based on state
    let failed_at = if new_state == "failed" {
        Some(updated_at)
    } else {
        None
    };
    let cancelled_at = if new_state == "cancelled" {
        Some(updated_at)
    } else {
        None
    };

    let request = sqlx::query_as!(
        WithdrawalRequest,
        r#"
        UPDATE withdrawal_requests
        SET state = $2,
            error_message = $3,
            updated_at = $4,
            failed_at = COALESCE($5, failed_at),
            cancelled_at = COALESCE($6, cancelled_at)
        WHERE id = $1
        RETURNING 
            id, user_id, user_address, recipient_address, amount, amount_usdc,
            state, kyc_validated, balance_validated, kyc_status_at_request,
            available_balance_at_request, transaction_hash, block_number,
            confirmation_count, required_confirmations, error_message,
            validation_errors, created_at, updated_at, validated_at,
            submitted_at, confirmed_at, failed_at, cancelled_at
        "#,
        request_id,
        new_state,
        error_message,
        updated_at,
        failed_at,
        cancelled_at
    )
    .fetch_one(pool)
    .await?;

    Ok(request)
}

/// Update withdrawal request validation status
pub async fn update_withdrawal_request_validation(
    pool: &PgPool,
    request_id: i32,
    kyc_validated: bool,
    balance_validated: bool,
    validation_errors: Option<serde_json::Value>,
) -> Result<WithdrawalRequest, sqlx::Error> {
    let validated_at = chrono::Utc::now();
    let new_state = if kyc_validated && balance_validated {
        "validated"
    } else {
        "failed"
    };

    let failed_at = if new_state == "failed" {
        Some(validated_at)
    } else {
        None
    };

    let request = sqlx::query_as!(
        WithdrawalRequest,
        r#"
        UPDATE withdrawal_requests
        SET kyc_validated = $2,
            balance_validated = $3,
            validation_errors = $4,
            state = $5,
            validated_at = $6,
            updated_at = $6,
            failed_at = COALESCE($7, failed_at)
        WHERE id = $1
        RETURNING 
            id, user_id, user_address, recipient_address, amount, amount_usdc,
            state, kyc_validated, balance_validated, kyc_status_at_request,
            available_balance_at_request, transaction_hash, block_number,
            confirmation_count, required_confirmations, error_message,
            validation_errors, created_at, updated_at, validated_at,
            submitted_at, confirmed_at, failed_at, cancelled_at
        "#,
        request_id,
        kyc_validated,
        balance_validated,
        validation_errors,
        new_state,
        validated_at,
        failed_at
    )
    .fetch_one(pool)
    .await?;

    Ok(request)
}

/// Update withdrawal request with transaction details (when submitted to contract)
pub async fn update_withdrawal_request_submitted(
    pool: &PgPool,
    request_id: i32,
    transaction_hash: &str,
    block_number: Option<i64>,
) -> Result<WithdrawalRequest, sqlx::Error> {
    let submitted_at = chrono::Utc::now();

    let request = sqlx::query_as!(
        WithdrawalRequest,
        r#"
        UPDATE withdrawal_requests
        SET state = 'submitted',
            transaction_hash = $2,
            block_number = $3,
            submitted_at = $4,
            updated_at = $4
        WHERE id = $1
        RETURNING 
            id, user_id, user_address, recipient_address, amount, amount_usdc,
            state, kyc_validated, balance_validated, kyc_status_at_request,
            available_balance_at_request, transaction_hash, block_number,
            confirmation_count, required_confirmations, error_message,
            validation_errors, created_at, updated_at, validated_at,
            submitted_at, confirmed_at, failed_at, cancelled_at
        "#,
        request_id,
        transaction_hash,
        block_number,
        submitted_at
    )
    .fetch_one(pool)
    .await?;

    Ok(request)
}

/// Update withdrawal request confirmation status
pub async fn update_withdrawal_request_confirmations(
    pool: &PgPool,
    request_id: i32,
    confirmation_count: i32,
) -> Result<WithdrawalRequest, sqlx::Error> {
    let updated_at = chrono::Utc::now();
    let confirmed_at = if confirmation_count >= 12 {
        Some(updated_at)
    } else {
        None
    };
    let new_state = if confirmation_count >= 12 {
        "confirmed"
    } else {
        "submitted"
    };

    let request = sqlx::query_as!(
        WithdrawalRequest,
        r#"
        UPDATE withdrawal_requests
        SET confirmation_count = $2,
            state = $3,
            confirmed_at = COALESCE($4, confirmed_at),
            updated_at = $5
        WHERE id = $1
        RETURNING 
            id, user_id, user_address, recipient_address, amount, amount_usdc,
            state, kyc_validated, balance_validated, kyc_status_at_request,
            available_balance_at_request, transaction_hash, block_number,
            confirmation_count, required_confirmations, error_message,
            validation_errors, created_at, updated_at, validated_at,
            submitted_at, confirmed_at, failed_at, cancelled_at
        "#,
        request_id,
        confirmation_count,
        new_state,
        confirmed_at,
        updated_at
    )
    .fetch_one(pool)
    .await?;

    Ok(request)
}

/// Get pending withdrawal requests (those needing validation or processing)
pub async fn get_pending_withdrawal_requests(
    pool: &PgPool,
    limit: Option<i32>,
) -> Result<Vec<WithdrawalRequest>, sqlx::Error> {
    let limit = limit.unwrap_or(100) as i64;

    let requests = sqlx::query_as!(
        WithdrawalRequest,
        r#"
        SELECT 
            id, user_id, user_address, recipient_address, amount, amount_usdc,
            state, kyc_validated, balance_validated, kyc_status_at_request,
            available_balance_at_request, transaction_hash, block_number,
            confirmation_count, required_confirmations, error_message,
            validation_errors, created_at, updated_at, validated_at,
            submitted_at, confirmed_at, failed_at, cancelled_at
        FROM withdrawal_requests
        WHERE state IN ('pending', 'validated')
        ORDER BY created_at ASC
        LIMIT $1
        "#,
        limit
    )
    .fetch_all(pool)
    .await?;

    Ok(requests)
}

// ============================================================================
// LIQUIDITY POOL FUNCTIONS
// ============================================================================

/// Database model for liquidity pools
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::FromRow)]
pub struct LiquidityPool {
    pub id: uuid::Uuid,
    pub investor_id: uuid::Uuid,
    pub name: String,
    pub total_deposits: rust_decimal::Decimal,
    pub available_liquidity: rust_decimal::Decimal,
    pub utilized_liquidity: rust_decimal::Decimal,
    pub max_ltv_ratio: rust_decimal::Decimal,
    pub max_interest_rate: rust_decimal::Decimal,
    pub repayment_period_days: i32,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Database model for liquidity pool audit records
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::FromRow)]
pub struct LiquidityPoolAudit {
    pub id: uuid::Uuid,
    pub pool_id: uuid::Uuid,
    pub operation_type: String,
    pub old_values: Option<serde_json::Value>,
    pub new_values: Option<serde_json::Value>,
    pub operator_id: Option<uuid::Uuid>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

/// Request structure for creating a liquidity pool
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateLiquidityPoolRequest {
    pub investor_id: uuid::Uuid,
    pub name: String,
    pub max_ltv_ratio: rust_decimal::Decimal,
    pub max_interest_rate: rust_decimal::Decimal,
    pub repayment_period_days: i32,
}

/// Pool matching criteria for auto-deployment
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PoolMatchingCriteria {
    pub required_liquidity: rust_decimal::Decimal,
    pub max_ltv: rust_decimal::Decimal,
    pub max_rate: rust_decimal::Decimal,
    pub min_repayment_days: i32,
}

/// Validate pool parameters
pub fn validate_pool_parameters(
    max_ltv_ratio: rust_decimal::Decimal,
    max_interest_rate: rust_decimal::Decimal,
    repayment_period_days: i32,
) -> Result<(), String> {
    use std::str::FromStr;
    
    // LTV ratio validation: 10% to 95%
    let min_ltv = rust_decimal::Decimal::from_str("0.1000").unwrap();
    let max_ltv = rust_decimal::Decimal::from_str("0.9500").unwrap();
    if max_ltv_ratio < min_ltv || max_ltv_ratio > max_ltv {
        return Err(format!(
            "LTV ratio must be between 10% and 95%, got: {}%", 
            max_ltv_ratio * rust_decimal::Decimal::from(100)
        ));
    }
    
    // Interest rate validation: 1% to 50%
    let min_rate = rust_decimal::Decimal::from_str("0.0100").unwrap();
    let max_rate = rust_decimal::Decimal::from_str("0.5000").unwrap();
    if max_interest_rate < min_rate || max_interest_rate > max_rate {
        return Err(format!(
            "Interest rate must be between 1% and 50%, got: {}%",
            max_interest_rate * rust_decimal::Decimal::from(100)
        ));
    }
    
    // Repayment period validation: 7 days to 1 year
    if repayment_period_days < 7 || repayment_period_days > 365 {
        return Err(format!(
            "Repayment period must be between 7 and 365 days, got: {} days",
            repayment_period_days
        ));
    }
    
    Ok(())
}

/// Create a new liquidity pool
pub async fn create_liquidity_pool(
    pool: &PgPool,
    request: &CreateLiquidityPoolRequest,
) -> Result<LiquidityPool, sqlx::Error> {
    // Validate parameters first
    if let Err(_validation_error) = validate_pool_parameters(
        request.max_ltv_ratio,
        request.max_interest_rate,
        request.repayment_period_days,
    ) {
        return Err(sqlx::Error::PoolClosed); // Use a generic error for validation
    }
    let liquidity_pool = sqlx::query_as!(
        LiquidityPool,
        r#"
        INSERT INTO liquidity_pools (
            investor_id, name, max_ltv_ratio, max_interest_rate, 
            repayment_period_days, status
        ) VALUES ($1, $2, $3, $4, $5, 'ACTIVE')
        RETURNING 
            id, investor_id, name, total_deposits, available_liquidity, 
            utilized_liquidity, max_ltv_ratio, max_interest_rate,
            repayment_period_days, status, 
            created_at as "created_at!: DateTime<Utc>", 
            updated_at as "updated_at!: DateTime<Utc>"
        "#,
        request.investor_id,
        request.name,
        request.max_ltv_ratio,
        request.max_interest_rate,
        request.repayment_period_days
    )
    .fetch_one(pool)
    .await?;

    Ok(liquidity_pool)
}

/// Get liquidity pool by ID
pub async fn get_liquidity_pool_by_id(
    pool: &PgPool,
    pool_id: uuid::Uuid,
) -> Result<LiquidityPool, sqlx::Error> {
    let liquidity_pool = sqlx::query_as!(
        LiquidityPool,
        r#"
        SELECT 
            id, investor_id, name, total_deposits, available_liquidity,
            utilized_liquidity, max_ltv_ratio, max_interest_rate,
            repayment_period_days, status,
            created_at as "created_at!: DateTime<Utc>",
            updated_at as "updated_at!: DateTime<Utc>"
        FROM liquidity_pools
        WHERE id = $1
        "#,
        pool_id
    )
    .fetch_one(pool)
    .await?;

    Ok(liquidity_pool)
}

/// Get liquidity pools by investor ID
pub async fn get_liquidity_pools_by_investor(
    pool: &PgPool,
    investor_id: uuid::Uuid,
) -> Result<Vec<LiquidityPool>, sqlx::Error> {
    let pools = sqlx::query_as!(
        LiquidityPool,
        r#"
        SELECT 
            id, investor_id, name, total_deposits, available_liquidity,
            utilized_liquidity, max_ltv_ratio, max_interest_rate,
            repayment_period_days, status,
            created_at as "created_at!: DateTime<Utc>",
            updated_at as "updated_at!: DateTime<Utc>"
        FROM liquidity_pools
        WHERE investor_id = $1
        ORDER BY created_at DESC
        "#,
        investor_id
    )
    .fetch_all(pool)
    .await?;

    Ok(pools)
}

/// Find matching liquidity pools for auto-deployment
pub async fn find_matching_liquidity_pools(
    pool: &PgPool,
    criteria: &PoolMatchingCriteria,
) -> Result<Vec<LiquidityPool>, sqlx::Error> {
    let pools = sqlx::query_as!(
        LiquidityPool,
        r#"
        SELECT 
            id, investor_id, name, total_deposits, available_liquidity,
            utilized_liquidity, max_ltv_ratio, max_interest_rate,
            repayment_period_days, status,
            created_at as "created_at!: DateTime<Utc>",
            updated_at as "updated_at!: DateTime<Utc>"
        FROM liquidity_pools
        WHERE status = 'ACTIVE'
            AND available_liquidity >= $1
            AND max_ltv_ratio >= $2
            AND max_interest_rate >= $3
            AND repayment_period_days >= $4
        ORDER BY max_interest_rate ASC, available_liquidity DESC
        "#,
        criteria.required_liquidity,
        criteria.max_ltv,
        criteria.max_rate,
        criteria.min_repayment_days
    )
    .fetch_all(pool)
    .await?;

    Ok(pools)
}

/// Update liquidity pool status
pub async fn update_liquidity_pool_status(
    pool: &PgPool,
    pool_id: uuid::Uuid,
    new_status: &str,
) -> Result<LiquidityPool, sqlx::Error> {
    let liquidity_pool = sqlx::query_as!(
        LiquidityPool,
        r#"
        UPDATE liquidity_pools
        SET status = $2, updated_at = NOW()
        WHERE id = $1
        RETURNING 
            id, investor_id, name, total_deposits, available_liquidity,
            utilized_liquidity, max_ltv_ratio, max_interest_rate,
            repayment_period_days, status,
            created_at as "created_at!: DateTime<Utc>",
            updated_at as "updated_at!: DateTime<Utc>"
        "#,
        pool_id,
        new_status
    )
    .fetch_one(pool)
    .await?;

    Ok(liquidity_pool)
}
