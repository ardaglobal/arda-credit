use chrono::{DateTime, Utc};
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
    };

    Ok(identity)
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
               password_reset_token, password_reset_expires_at
        FROM identities
        WHERE id = $1
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
               password_reset_token, password_reset_expires_at
        FROM identities
        WHERE email = $1
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
               password_reset_token, password_reset_expires_at
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

    sqlx::query!(
        r#"
        UPDATE identities 
        SET kyc_status = 'pending',
            kyc_submitted_at = $2,
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
    _jurisdiction: Option<&str>,
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

/// Get loan by ID - Simple implementation for now
pub async fn get_loan_by_id(_pool: &PgPool, _id: uuid::Uuid) -> Result<Option<Loan>, sqlx::Error> {
    // For now, return None - can be implemented later
    Ok(None)
}

/// List loans - Simple implementation for now
pub async fn list_loans(
    _pool: &PgPool,
    _limit: Option<i64>,
    _offset: Option<i64>,
    _status: Option<&str>,
    _borrower_id: Option<uuid::Uuid>,
    _lender_id: Option<uuid::Uuid>,
) -> Result<Vec<Loan>, sqlx::Error> {
    // For now, return empty - can be implemented later
    Ok(vec![])
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
               password_reset_token, password_reset_expires_at
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
pub async fn check_invite_code(pool: &PgPool, code: &str) -> Result<bool, sqlx::Error> {
    debug!("Checking invite code validity: code={}", code);

    let is_valid: bool = sqlx::query_scalar!("SELECT is_invite_code_valid($1)", code)
        .fetch_one(pool)
        .await?
        .unwrap_or(false);

    debug!("Invite code {} is valid: {}", code, is_valid);
    Ok(is_valid)
}
