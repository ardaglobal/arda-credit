use chrono::{DateTime, Utc};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::env;
use std::str::FromStr;
use tracing::{debug, instrument};

use crate::error::DbError;

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
    pub kyc_investor_type: Option<KycInvestorType>, // QIB, QP, AI - only for investors
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    // Authentication fields (magic link system)
    pub last_login_at: Option<DateTime<Utc>>,
    pub email_verified: Option<bool>,
    pub email_verification_token: Option<String>,
    // User profile fields
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub jurisdiction: Option<String>,
    // Wallet integration
    pub wallet_address: Option<String>,
}

/// Magic link for email-based authentication
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::FromRow)]
pub struct MagicLink {
    pub id: uuid::Uuid,
    pub user_id: uuid::Uuid,
    pub token_hash: Vec<u8>, // BYTEA maps to Vec<u8>
    pub state: String,
    pub redirect_uri: Option<String>,
    pub client_ip: Option<String>, // Client IP address for security logging
    pub user_agent: Option<String>,
    pub expires_at: DateTime<Utc>,
    pub consumed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// Session for cookie-based authentication after magic link verification
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::FromRow)]
pub struct Session {
    pub id: uuid::Uuid,
    pub user_id: uuid::Uuid,
    pub session_token: String,
    pub expires_at: DateTime<Utc>,
    pub client_ip: Option<String>,
    pub user_agent: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_accessed_at: DateTime<Utc>,
}

/// Investor profile for enhanced investor account functionality
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct InvestorProfile {
    pub id: uuid::Uuid,
    pub user_id: uuid::Uuid,
    pub accredited_investor: bool,
    pub investment_capacity: Option<rust_decimal::Decimal>, // maximum investment amount in USDC
    pub regulatory_jurisdiction: String,                    // e.g., 'US', 'EU', 'UK'
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// KYC investor type enumeration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "kyc_investor_type", rename_all = "UPPERCASE")]
pub enum KycInvestorType {
    QIB, // Qualified Institutional Buyer
    QP,  // Qualified Purchaser
    AI,  // Accredited Investor
}

impl std::fmt::Display for KycInvestorType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::QIB => write!(f, "QIB"),
            Self::QP => write!(f, "QP"),
            Self::AI => write!(f, "AI"),
        }
    }
}

impl std::str::FromStr for KycInvestorType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "QIB" => Ok(Self::QIB),
            "QP" => Ok(Self::QP),
            "AI" => Ok(Self::AI),
            _ => Err(format!("Invalid KYC investor type: {}", s)),
        }
    }
}

/// Request to create investor profile
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateInvestorProfileRequest {
    pub accredited_investor: Option<bool>,
    pub investment_capacity: Option<rust_decimal::Decimal>,
    pub regulatory_jurisdiction: Option<String>,
}

/// Request to update investor profile
#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateInvestorProfileRequest {
    pub accredited_investor: Option<bool>,
    pub investment_capacity: Option<rust_decimal::Decimal>,
    pub regulatory_jurisdiction: Option<String>,
}

// ============================================================================
// MERCHANT TYPES
// ============================================================================

/// Merchant account for business credit applications
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct Merchant {
    pub id: uuid::Uuid,
    pub user_id: uuid::Uuid,
    pub legal_name: String,
    pub dba: Option<String>, // Doing Business As name
    pub tax_id: Option<String>,
    pub mcc: Option<String>, // Merchant Category Code
    pub country: String,
    pub kyc_status: String, // NOT_SUBMITTED, PENDING, APPROVED, REJECTED
    pub business_type: Option<String>, // LLC, CORP, SOLE_PROP, PARTNERSHIP, OTHER
    pub borough: Option<String>, // Geographic region for lending limits
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Merchant profile for business details and credit assessment
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct MerchantProfile {
    pub id: uuid::Uuid,
    pub merchant_id: uuid::Uuid,
    pub industry: Option<String>,
    pub years_in_business: Option<i32>,
    pub employee_count: Option<i32>,
    pub monthly_revenue_estimate: Option<rust_decimal::Decimal>,
    pub business_description: Option<String>,
    pub profile_completed: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Business type enumeration for merchants
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BusinessType {
    LLC,
    CORP,
    SoleProp,
    Partnership,
    Other,
}

impl std::fmt::Display for BusinessType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LLC => write!(f, "LLC"),
            Self::CORP => write!(f, "CORP"),
            Self::SoleProp => write!(f, "SOLE_PROP"),
            Self::Partnership => write!(f, "PARTNERSHIP"),
            Self::Other => write!(f, "OTHER"),
        }
    }
}

impl std::str::FromStr for BusinessType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "LLC" => Ok(Self::LLC),
            "CORP" => Ok(Self::CORP),
            "SOLE_PROP" => Ok(Self::SoleProp),
            "PARTNERSHIP" => Ok(Self::Partnership),
            "OTHER" => Ok(Self::Other),
            _ => Err(format!("Invalid business type: {}", s)),
        }
    }
}

/// Request to register a new merchant account
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateMerchantRequest {
    pub legal_name: String,
    pub dba: Option<String>,
    pub tax_id: Option<String>,
    pub mcc: Option<String>,
    pub country: String,
    pub business_type: Option<String>,
}

/// Request to create merchant profile
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateMerchantProfileRequest {
    pub industry: Option<String>,
    pub years_in_business: Option<i32>,
    pub employee_count: Option<i32>,
    pub monthly_revenue_estimate: Option<rust_decimal::Decimal>,
    pub business_description: Option<String>,
}

/// Request to update merchant profile
#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateMerchantProfileRequest {
    pub legal_name: Option<String>,
    pub dba: Option<String>,
    pub mcc: Option<String>,
    pub country: Option<String>,
    pub business_type: Option<String>,
    pub industry: Option<String>,
    pub years_in_business: Option<i32>,
    pub employee_count: Option<i32>,
    pub monthly_revenue_estimate: Option<rust_decimal::Decimal>,
    pub business_description: Option<String>,
}

/// Deal record with originator/investor relationship (converted from Loan)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct Deal {
    // was Loan
    pub id: uuid::Uuid,
    pub originator_id: uuid::Uuid, // was borrower_id
    pub investor_id: uuid::Uuid,   // was lender_id
    pub principal_amount: rust_decimal::Decimal,
    pub outstanding_balance: rust_decimal::Decimal,
    pub interest_rate_bps: i32, // Basis points
    pub deal_status: String,    // originated, active, paid_off, closed, defaulted (was loan_status)
    pub originated_at: DateTime<Utc>,
    pub maturity_date: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub custom_deal_id: Option<String>, // Optional user-defined identifier for external system mapping
    pub status: String,                 // draft, active, closed, cancelled
    pub industry: Option<String>,       // Industry sector for the deal
    pub document_ids: Option<serde_json::Value>, // JSON array of document UUIDs

    // Marketplace-friendly fields
    pub title: Option<String>,
    pub description: Option<String>,
    pub risk_level: Option<String>,
    pub asset_class: Option<String>,
    pub region: Option<String>,
    pub minimum_investment: Option<rust_decimal::Decimal>,
}

/// Deal payment record (converted from Payment)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct DealPayment {
    // was Payment
    pub id: uuid::Uuid,
    pub deal_id: uuid::Uuid, // was loan_id
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

/// Deal transaction for zkVM processing
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::FromRow)]
pub struct DealTransactionDb {
    pub id: uuid::Uuid,
    pub transaction_id_hash: Vec<u8>,
    pub user_id_hash: Vec<u8>,
    pub deal_id_hash: Option<Vec<u8>>,
    pub transaction_type: String,
    pub amount: rust_decimal::Decimal,
    pub identity_id: Option<uuid::Uuid>,
    pub deal_id: Option<uuid::Uuid>,
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
#[instrument(skip(pool), fields(
    operation = "submit_transaction",
    table = "incoming_transactions",
    layer = "database",
    amount = %amount
), level = "info")]
pub async fn submit_transaction(
    pool: &PgPool,
    amount: i32,
) -> Result<IncomingTransaction, sqlx::Error> {
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
#[instrument(
    skip(pool),
    fields(
        operation = "select",
        table = "incoming_transactions",
        layer = "database"
    ),
    level = "info"
)]
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
#[instrument(skip(pool), fields(
    operation = "create_batch",
    table = "batches",
    layer = "database",
    batch_size = %batch_size.unwrap_or(10)
), level = "info")]
pub async fn create_batch(
    pool: &PgPool,
    batch_size: Option<i32>,
) -> Result<Option<ProofBatch>, sqlx::Error> {
    let size = batch_size.unwrap_or(10);

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
#[instrument(skip(pool), fields(
    operation = "select",
    table = "proof_batches",
    layer = "database",
    batch_id = %batch_id
), level = "info")]
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
#[instrument(skip(pool), fields(
    operation = "select",
    table = "proof_batches",
    layer = "database",
    limit = ?limit
), level = "info")]
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
#[instrument(skip(pool), fields(
    operation = "update_batch_proof",
    table = "batches",
    layer = "database",
    batch_id = %batch_id,
    proof_id = %proof_id,
    status = %status
), level = "info")]
pub async fn update_batch_proof(
    pool: &PgPool,
    batch_id: i32,
    proof_id: &str,
    status: &str,
) -> Result<(), sqlx::Error> {
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
#[instrument(
    skip(pool),
    fields(operation = "select", table = "processor_state", layer = "database"),
    level = "info"
)]
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
#[instrument(skip(pool, merkle_root), fields(
    operation = "store_ads_state_commit",
    table = "ads_state_commits",
    layer = "database",
    batch_id = %batch_id,
    merkle_root_len = %merkle_root.len()
), level = "info")]
pub async fn store_ads_state_commit(
    pool: &PgPool,
    batch_id: i32,
    merkle_root: &[u8],
) -> Result<AdsStateCommit, sqlx::Error> {
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
#[instrument(skip(pool), fields(
    operation = "select",
    table = "proof_batches",
    layer = "database",
    batch_id = %batch_id
), level = "info")]
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
#[instrument(skip(pool), fields(
    operation = "create_identity",
    table = "identities",
    layer = "database",
    user_type = %user_type,
    email = %email
), level = "info")]
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
        kyc_investor_type: None,
        created_at: now,
        updated_at: now,
        last_login_at: None,
        email_verified: Some(false),
        email_verification_token: None,
        first_name: None,
        last_name: None,
        jurisdiction: None,
        wallet_address: None,
    };

    Ok(identity)
}

/// DEPRECATED: Password authentication has been replaced with magic link authentication
/// This function is kept for backwards compatibility but always returns an error
pub async fn create_identity_with_password(
    _pool: &PgPool,
    _user_type: &str,
    _email: &str,
    _password_hash: &str,
) -> Result<Identity, sqlx::Error> {
    Err(sqlx::Error::Configuration(
        "Password authentication has been disabled. Use magic link authentication instead.".into(),
    ))
}

/// Register or re-register user with soft delete handling
/// If email exists and user is deleted, undelete them with new password
/// If email exists and user is not deleted, return error
/// If email doesn't exist, create new user
pub async fn register_or_reregister_user(
    pool: &PgPool,
    user_type: &str,
    email: &str,
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
                    deleted_at = NULL,
                    updated_at = $2,
                    email_verified = $3,
                    last_login_at = NULL,
                    email_verification_token = NULL
                WHERE id = $4
                "#,
                user_type,
                now,
                false, // email_verified
                user.id
            )
            .execute(pool)
            .await?;

            // Return the existing identity (now undeleted)
            let identity = sqlx::query!(
                r#"
                SELECT id, user_type, email, kyc_status, kyc_submitted_at, kyc_verified_at, kyc_investor_type as "kyc_investor_type?: KycInvestorType",
                       created_at as "created_at!", updated_at as "updated_at!",
                       last_login_at, email_verified, email_verification_token,
                       wallet_address, first_name, last_name, jurisdiction
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
                kyc_investor_type: identity.kyc_investor_type,
                created_at: identity.created_at,
                updated_at: identity.updated_at,
                last_login_at: identity.last_login_at,
                email_verified: identity.email_verified,
                email_verification_token: identity.email_verification_token,
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
                kyc_investor_type: None,
                created_at: now,
                updated_at: now,
                last_login_at: None,
                email_verified: Some(false),
                email_verification_token: None,
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
#[instrument(skip(pool), fields(
    operation = "get_identity_by_id",
    table = "identities",
    layer = "database",
    user_id = %id
), level = "info")]
pub async fn get_identity_by_id(
    pool: &PgPool,
    id: uuid::Uuid,
) -> Result<Option<Identity>, sqlx::Error> {
    let row = sqlx::query!(
        r#"
        SELECT id, user_type, email, kyc_status, kyc_submitted_at, kyc_verified_at, kyc_investor_type as "kyc_investor_type?: KycInvestorType",
               created_at as "created_at!", updated_at as "updated_at!",
               last_login_at, email_verified, email_verification_token,
               wallet_address, first_name, last_name, jurisdiction
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
        kyc_investor_type: row.kyc_investor_type,
        created_at: row.created_at,
        updated_at: row.updated_at,
        last_login_at: row.last_login_at,
        email_verified: row.email_verified,
        email_verification_token: row.email_verification_token,
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
        SELECT id, user_type, email, kyc_status, kyc_submitted_at, kyc_verified_at, kyc_investor_type as "kyc_investor_type?: KycInvestorType",
               created_at as "created_at!", updated_at as "updated_at!",
               last_login_at, email_verified, email_verification_token,
               wallet_address,
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
        kyc_investor_type: row.kyc_investor_type,
        created_at: row.created_at,
        updated_at: row.updated_at,
        last_login_at: row.last_login_at,
        email_verified: row.email_verified,
        email_verification_token: row.email_verification_token,
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
        SELECT id, user_type, email, kyc_status, kyc_submitted_at, kyc_verified_at, kyc_investor_type as "kyc_investor_type?: KycInvestorType",
               created_at as "created_at!", updated_at as "updated_at!",
               last_login_at, email_verified, email_verification_token,
               wallet_address,
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
            kyc_investor_type: row.kyc_investor_type,
            created_at: row.created_at,
            updated_at: row.updated_at,
            last_login_at: row.last_login_at,
            email_verified: row.email_verified,
            email_verification_token: row.email_verification_token,
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
    kyc_investor_type: Option<&str>,
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
            kyc_investor_type = $3::kyc_investor_type,
            updated_at = $2
        WHERE id = $1
        "#,
        identity_id,
        now,
        kyc_investor_type as _
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
        SELECT id, user_type, email, kyc_status, kyc_submitted_at, kyc_verified_at, kyc_investor_type as "kyc_investor_type?: KycInvestorType",
               created_at as "created_at!", updated_at as "updated_at!",
               last_login_at, email_verified, email_verification_token,
               wallet_address,
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
            kyc_investor_type: row.kyc_investor_type,
            created_at: row.created_at,
            updated_at: row.updated_at,
            last_login_at: row.last_login_at,
            email_verified: row.email_verified,
            email_verification_token: row.email_verification_token,
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

/// Create a new deal (was create_loan)
pub async fn create_deal(
    // was create_loan
    pool: &PgPool,
    originator_id: uuid::Uuid, // was borrower_id
    investor_id: uuid::Uuid,   // was lender_id
    principal_amount: rust_decimal::Decimal,
    interest_rate_bps: i32,
    maturity_date: Option<DateTime<Utc>>,
    deal_status: Option<&str>, // Deal status: "originated", "active", "paid_off", "closed", "defaulted" (was loan_status)
    custom_deal_id: Option<&str>,
    status: Option<&str>,
    industry: Option<&str>,
    document_ids: Option<&serde_json::Value>,
    title: Option<&str>,
    description: Option<&str>,
    risk_level: Option<&str>,
    asset_class: Option<&str>,
    region: Option<&str>,
    minimum_investment: Option<rust_decimal::Decimal>,
) -> Result<Deal, sqlx::Error> {
    // was Loan
    let id = uuid::Uuid::new_v4();
    let now = Utc::now();
    let default_json = serde_json::Value::Array(vec![]);
    let doc_ids = document_ids.unwrap_or(&default_json);

    sqlx::query!(
        r#"
        INSERT INTO deals (
            id, originator_id, investor_id, principal_amount, outstanding_balance, interest_rate_bps, deal_status, maturity_date, originated_at, created_at, updated_at,
            custom_deal_id, status, industry, document_ids,
            title, description, risk_level, asset_class, region, minimum_investment
        )
        VALUES (
            $1, $2, $3, $4, $4, $5, $6, $7, $8, $8, $8,
            $9, $10, $11, $12,
            $13, $14, $15, $16, $17, $18
        )
        "#,  // was loans, borrower_id, lender_id
        id,
        originator_id,  // was borrower_id
        investor_id,    // was lender_id
        principal_amount,
        interest_rate_bps,
        deal_status.unwrap_or("originated"),
        maturity_date,
        now,
        custom_deal_id,
        status.unwrap_or("draft"),
        industry,
        doc_ids,
        title,
        description,
        risk_level,
        asset_class,
        region,
        minimum_investment
    )
    .execute(pool)
    .await?;

    let deal = Deal {
        // was Loan
        id,
        originator_id, // was borrower_id
        investor_id,   // was lender_id
        principal_amount,
        outstanding_balance: principal_amount,
        interest_rate_bps,
        deal_status: deal_status.unwrap_or("originated").to_string(),
        originated_at: now,
        maturity_date,
        created_at: now,
        updated_at: now,
        custom_deal_id: custom_deal_id.map(|s| s.to_string()),
        status: status.unwrap_or("draft").to_string(),
        industry: industry.map(|s| s.to_string()),
        document_ids: Some(doc_ids.clone()),
        title: title.map(|s| s.to_string()),
        description: description.map(|s| s.to_string()),
        risk_level: risk_level.map(|s| s.to_string()),
        asset_class: asset_class.map(|s| s.to_string()),
        region: region.map(|s| s.to_string()),
        minimum_investment,
    };

    Ok(deal)
}

/// Get deal by ID (was get_loan_by_id)
pub async fn get_deal_by_id(pool: &PgPool, id: uuid::Uuid) -> Result<Option<Deal>, sqlx::Error> {
    // was Loan
    let row = sqlx::query!(
        r#"
        SELECT
            id,
            originator_id,    -- was borrower_id
            investor_id,      -- was lender_id
            principal_amount,
            outstanding_balance,
            interest_rate_bps,
            deal_status,
            originated_at,
            maturity_date,
            created_at,
            updated_at,
            custom_deal_id,
            status,
            industry,
            document_ids,
            title,
            description,
            risk_level,
            asset_class,
            region,
            minimum_investment
        FROM deals  -- was loans
        WHERE id = $1
        "#,
        id
    )
    .fetch_optional(pool)
    .await?;

    match row {
        Some(row) => Ok(Some(Deal {
            // was Loan
            id: row.id,
            originator_id: row.originator_id, // was borrower_id
            investor_id: row.investor_id,     // was lender_id
            principal_amount: row.principal_amount,
            outstanding_balance: row.outstanding_balance,
            interest_rate_bps: row.interest_rate_bps,
            deal_status: row.deal_status.clone(),
            originated_at: row.originated_at.unwrap_or_else(|| chrono::Utc::now()),
            maturity_date: row.maturity_date,
            created_at: row.created_at.unwrap_or_else(|| chrono::Utc::now()),
            updated_at: row.updated_at.unwrap_or_else(|| chrono::Utc::now()),
            custom_deal_id: row.custom_deal_id,
            status: row.status.unwrap_or_else(|| "draft".to_string()),
            industry: row.industry,
            document_ids: row.document_ids,
            title: row.title,
            description: row.description,
            risk_level: row.risk_level,
            asset_class: row.asset_class,
            region: row.region,
            minimum_investment: row.minimum_investment,
        })),
        None => Ok(None),
    }
}

/// List deals with optional filtering (was list_loans)
pub async fn list_deals(
    // was list_loans
    pool: &PgPool,
    limit: Option<i64>,
    offset: Option<i64>,
    status: Option<&str>,
    originator_id: Option<uuid::Uuid>, // was borrower_id
    investor_id: Option<uuid::Uuid>,   // was lender_id
    industry: Option<&str>,
) -> Result<Vec<Deal>, sqlx::Error> {
    // was Loan
    // Use a unified query with optional filtering
    let rows = sqlx::query!(
        r#"
        SELECT
            id,
            originator_id,    -- was borrower_id
            investor_id,      -- was lender_id
            principal_amount,
            outstanding_balance,
            interest_rate_bps,
            deal_status,
            originated_at,
            maturity_date,
            created_at,
            updated_at,
            custom_deal_id,
            status,
            industry,
            document_ids,
            title,
            description,
            risk_level,
            asset_class,
            region,
            minimum_investment
        FROM deals  -- was loans
        WHERE ($1::text IS NULL OR deal_status = $1)
          AND ($2::uuid IS NULL OR originator_id = $2)  -- was borrower_id
          AND ($3::uuid IS NULL OR investor_id = $3)    -- was lender_id
          AND ($4::text IS NULL OR industry = $4)
        ORDER BY created_at DESC
        LIMIT $5 OFFSET $6
        "#,
        status,
        originator_id, // was borrower_id
        investor_id,   // was lender_id
        industry,
        limit.unwrap_or(100),
        offset.unwrap_or(0)
    )
    .fetch_all(pool)
    .await?;

    let result: Vec<Deal> = rows // was Loan
        .into_iter()
        .map(|row| Deal {
            // was Loan
            id: row.id,
            originator_id: row.originator_id, // was borrower_id
            investor_id: row.investor_id,     // was lender_id
            principal_amount: row.principal_amount,
            outstanding_balance: row.outstanding_balance,
            interest_rate_bps: row.interest_rate_bps,
            deal_status: row.deal_status.clone(),
            originated_at: row.originated_at.unwrap_or_else(|| chrono::Utc::now()),
            maturity_date: row.maturity_date,
            created_at: row.created_at.unwrap_or_else(|| chrono::Utc::now()),
            updated_at: row.updated_at.unwrap_or_else(|| chrono::Utc::now()),
            custom_deal_id: row.custom_deal_id,
            status: row.status.unwrap_or_else(|| "draft".to_string()),
            industry: row.industry,
            document_ids: row.document_ids,
            title: row.title,
            description: row.description,
            risk_level: row.risk_level,
            asset_class: row.asset_class,
            region: row.region,
            minimum_investment: row.minimum_investment,
        })
        .collect();

    Ok(result)
}

/// Get deals by user ID (user can be either originator or investor)
pub async fn get_deals_by_user_id(
    pool: &PgPool,
    user_id: uuid::Uuid,
    limit: Option<i64>,
    offset: Option<i64>,
    status: Option<&str>,
) -> Result<Vec<Deal>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"
        SELECT
            id,
            originator_id,
            investor_id,
            principal_amount,
            outstanding_balance,
            interest_rate_bps,
            deal_status,
            originated_at,
            maturity_date,
            created_at,
            updated_at,
            custom_deal_id,
            status,
            industry,
            document_ids,
            title,
            description,
            risk_level,
            asset_class,
            region,
            minimum_investment
        FROM deals
        WHERE (originator_id = $1 OR investor_id = $1)
          AND ($2::text IS NULL OR deal_status = $2)
        ORDER BY created_at DESC
        LIMIT $3 OFFSET $4
        "#,
        user_id,
        status,
        limit.unwrap_or(100),
        offset.unwrap_or(0)
    )
    .fetch_all(pool)
    .await?;

    let result: Vec<Deal> = rows
        .into_iter()
        .map(|row| Deal {
            id: row.id,
            originator_id: row.originator_id,
            investor_id: row.investor_id,
            principal_amount: row.principal_amount,
            outstanding_balance: row.outstanding_balance,
            interest_rate_bps: row.interest_rate_bps,
            deal_status: row.deal_status,
            originated_at: row.originated_at.unwrap_or_else(|| chrono::Utc::now()),
            maturity_date: row.maturity_date,
            created_at: row.created_at.unwrap_or_else(|| chrono::Utc::now()),
            updated_at: row.updated_at.unwrap_or_else(|| chrono::Utc::now()),
            custom_deal_id: row.custom_deal_id,
            status: row.status.unwrap_or_else(|| "draft".to_string()),
            industry: row.industry,
            document_ids: row.document_ids,
            title: row.title,
            description: row.description,
            risk_level: row.risk_level,
            asset_class: row.asset_class,
            region: row.region,
            minimum_investment: row.minimum_investment,
        })
        .collect();

    Ok(result)
}

/// Update deal details (was update_loan)
pub async fn update_deal(
    // was update_loan
    pool: &PgPool,
    id: uuid::Uuid,
    principal_amount: Option<rust_decimal::Decimal>,
    outstanding_balance: Option<rust_decimal::Decimal>,
    interest_rate_bps: Option<i32>,
    deal_status: Option<&str>,
    maturity_date: Option<Option<DateTime<Utc>>>, // Option<Option<T>> to handle setting to NULL
    custom_deal_id: Option<Option<&str>>,         // Option<Option<T>> to handle setting to NULL
    title: Option<Option<&str>>,                  // Option<Option<T>> to handle setting to NULL
    description: Option<Option<&str>>,            // Option<Option<T>> to handle setting to NULL
    risk_level: Option<Option<&str>>,             // Option<Option<T>> to handle setting to NULL
    asset_class: Option<Option<&str>>,            // Option<Option<T>> to handle setting to NULL
    region: Option<Option<&str>>,                 // Option<Option<T>> to handle setting to NULL
    minimum_investment: Option<Option<rust_decimal::Decimal>>, // Option<Option<T>> to handle setting to NULL
) -> Result<Option<Deal>, sqlx::Error> {
    // was Loan
    let now = Utc::now();

    // First check if the deal exists (was loan)
    let existing_deal = get_deal_by_id(pool, id).await?; // was get_loan_by_id
    if existing_deal.is_none() {
        return Ok(None);
    }
    let existing_deal = existing_deal.unwrap(); // was existing_loan

    // Update only the provided fields
    let new_principal = principal_amount.unwrap_or(existing_deal.principal_amount);
    let new_outstanding = outstanding_balance.unwrap_or(existing_deal.outstanding_balance);
    let new_rate = interest_rate_bps.unwrap_or(existing_deal.interest_rate_bps);
    let new_status = deal_status.unwrap_or(&existing_deal.deal_status);
    let new_maturity = maturity_date.unwrap_or(existing_deal.maturity_date);
    let new_custom_deal_id = match custom_deal_id {
        Some(Some(value)) => Some(value.to_string()),
        Some(None) => None,
        None => existing_deal.custom_deal_id,
    };
    let new_title = match title {
        Some(Some(value)) => Some(value.to_string()),
        Some(None) => None,
        None => existing_deal.title,
    };
    let new_description = match description {
        Some(Some(value)) => Some(value.to_string()),
        Some(None) => None,
        None => existing_deal.description,
    };
    let new_risk_level = match risk_level {
        Some(Some(value)) => Some(value.to_string()),
        Some(None) => None,
        None => existing_deal.risk_level,
    };
    let new_asset_class = match asset_class {
        Some(Some(value)) => Some(value.to_string()),
        Some(None) => None,
        None => existing_deal.asset_class,
    };
    let new_region = match region {
        Some(Some(value)) => Some(value.to_string()),
        Some(None) => None,
        None => existing_deal.region,
    };
    let new_minimum_investment = match minimum_investment {
        Some(Some(value)) => Some(value),
        Some(None) => None,
        None => existing_deal.minimum_investment,
    };

    sqlx::query!(
        r#"
        UPDATE deals  -- was loans
        SET
            principal_amount = $2,
            outstanding_balance = $3,
            interest_rate_bps = $4,
            deal_status = $5,
            maturity_date = $6,
            custom_deal_id = $8,
            title = $9,
            description = $10,
            risk_level = $11,
            asset_class = $12,
            region = $13,
            minimum_investment = $14,
            updated_at = $7
        WHERE id = $1
        "#,
        id,
        new_principal,
        new_outstanding,
        new_rate,
        new_status,
        new_maturity,
        now,
        new_custom_deal_id,
        new_title,
        new_description,
        new_risk_level,
        new_asset_class,
        new_region,
        new_minimum_investment
    )
    .execute(pool)
    .await?;

    // Return the updated deal (was loan)
    get_deal_by_id(pool, id).await // was get_loan_by_id
}

/// Record a deal payment - Simple implementation for now (was create_payment)
pub async fn create_deal_payment(
    // was create_payment
    pool: &PgPool,
    deal_id: uuid::Uuid, // was loan_id
    payer_id: uuid::Uuid,
    amount: rust_decimal::Decimal,
    payment_type: &str,
    transaction_hash: Option<&str>,
) -> Result<DealPayment, sqlx::Error> {
    // was Payment
    let id = uuid::Uuid::new_v4();
    let now = Utc::now();

    sqlx::query!(
        r#"
        INSERT INTO deal_payments (id, deal_id, payer_id, amount, payment_type, transaction_hash, processed_at, created_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $7)
        "#,  // was payments table with loan_id column
        id,
        deal_id,
        payer_id,
        amount,
        payment_type,
        transaction_hash,
        now
    )
    .execute(pool)
    .await?;

    let payment = DealPayment {
        // was Payment
        id,
        deal_id, // was loan_id
        payer_id,
        amount,
        payment_type: payment_type.to_string(),
        transaction_hash: transaction_hash.map(|s| s.to_string()),
        processed_at: now,
        created_at: now,
    };

    Ok(payment)
}

/// Get payments for a deal - Simple implementation for now (was get_payments_by_loan)
pub async fn get_payments_by_deal(
    // was get_payments_by_loan
    _pool: &PgPool,
    _deal_id: uuid::Uuid, // was _loan_id
    _limit: Option<i64>,
    _offset: Option<i64>,
) -> Result<Vec<DealPayment>, sqlx::Error> {
    // was Payment
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

/// DEPRECATED: Password authentication has been replaced with magic link authentication
/// This function is kept for backwards compatibility but always returns an error
pub async fn update_user_password(
    _pool: &PgPool,
    _user_id: uuid::Uuid,
    _password_hash: &str,
) -> Result<(), sqlx::Error> {
    Err(sqlx::Error::Configuration(
        "Password authentication has been disabled. Use magic link authentication instead.".into(),
    ))
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

/// DEPRECATED: Password authentication has been replaced with magic link authentication
/// This function is kept for backwards compatibility but always returns false
pub async fn set_password_reset_token(
    _pool: &PgPool,
    _email: &str,
    _token: &str,
    _expires_in_hours: i64,
) -> Result<bool, sqlx::Error> {
    // Password reset is no longer supported - return false to indicate failure
    Ok(false)
}

/// DEPRECATED: Password authentication has been replaced with magic link authentication
/// This function is kept for backwards compatibility but always returns false
pub async fn reset_password_with_token(
    _pool: &PgPool,
    _token: &str,
    _new_password_hash: &str,
) -> Result<bool, sqlx::Error> {
    // Password reset is no longer supported - return false to indicate failure
    Ok(false)
}

/// DEPRECATED: Password authentication has been replaced with magic link authentication
/// This function is kept for backwards compatibility but always returns None
pub async fn get_identity_by_reset_token(
    _pool: &PgPool,
    _token: &str,
) -> Result<Option<Identity>, sqlx::Error> {
    // Password reset tokens are no longer valid - return None
    Ok(None)
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
    pub user_type: Option<String>, // 'admin', 'investor', or 'originator'
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
    user_type: Option<String>, // 'admin', 'investor', or 'originator'
) -> Result<String, sqlx::Error> {
    let user_type = user_type.unwrap_or_else(|| "investor".to_string());
    debug!(
        "Generating new invite code for user: {} (user_type: {})",
        created_by, user_type
    );

    // Validate user_type
    if !["admin", "investor", "originator"].contains(&user_type.as_str()) {
        return Err(sqlx::Error::Protocol(format!(
            "Invalid user_type '{}'. Must be 'admin', 'investor', or 'originator'",
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

/// Get user's available balance for withdrawals (deposits minus pending/approved withdrawals)
pub async fn get_user_available_balance(
    pool: &PgPool,
    user_address: &str,
) -> Result<Option<UserDepositBalance>, sqlx::Error> {
    let balance = sqlx::query_as!(
        UserDepositBalance,
        r#"
        SELECT
            udb.id,
            udb.user_address,
            (udb.total_deposited - COALESCE(pending_withdrawals.pending_amount, 0)) as "total_deposited!: rust_decimal::Decimal",
            ((udb.total_deposited - COALESCE(pending_withdrawals.pending_amount, 0))::numeric / 1000000::numeric) as total_deposited_usdc,
            udb.deposit_count,
            udb.first_deposit_at,
            udb.last_deposit_at,
            udb.created_at,
            udb.updated_at
        FROM user_deposit_balances udb
        LEFT JOIN (
            SELECT
                user_address,
                SUM(amount) as pending_amount
            FROM withdrawal_requests
            WHERE user_address = $1
              AND state IN ('pending', 'approved', 'submitted')
            GROUP BY user_address
        ) pending_withdrawals ON udb.user_address = pending_withdrawals.user_address
        WHERE udb.user_address = $1
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
    // Always keep state as "pending" regardless of validation results for now
    let new_state = "pending";

    // No failed_at timestamp since we're keeping all requests as pending
    let failed_at: Option<chrono::DateTime<chrono::Utc>> = None;

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

/// Get admin-approved withdrawal requests ready for execution
pub async fn get_validated_pending_withdrawals(
    pool: &PgPool,
    limit: Option<i32>,
) -> Result<Vec<WithdrawalRequest>, DbError> {
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
        WHERE state = 'approved'
            AND kyc_validated = true
            AND balance_validated = true
            AND cancelled_at IS NULL
            AND failed_at IS NULL
        ORDER BY created_at ASC
        LIMIT $1
        "#,
        limit
    )
    .fetch_all(pool)
    .await?;

    Ok(requests)
}

/// Mark a withdrawal as submitted (called by withdrawal executor)
pub async fn mark_withdrawal_submitted(
    pool: &PgPool,
    request_id: i32,
    transaction_hash: &str,
    block_number: Option<i64>,
) -> Result<WithdrawalRequest, DbError> {
    update_withdrawal_request_submitted(pool, request_id, transaction_hash, block_number)
        .await
        .map_err(DbError::Database)
}

/// Get withdrawal requests pending admin approval
pub async fn get_withdrawals_pending_admin_approval(
    pool: &PgPool,
    limit: i32,
    offset: i32,
) -> Result<Vec<WithdrawalRequest>, DbError> {
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
        WHERE state = 'pending'
            AND cancelled_at IS NULL
        ORDER BY created_at ASC
        LIMIT $1 OFFSET $2
        "#,
        limit as i64,
        offset as i64
    )
    .fetch_all(pool)
    .await?;

    Ok(requests)
}

/// Approve a withdrawal request (admin action)
pub async fn approve_withdrawal_request(
    pool: &PgPool,
    request_id: i32,
    admin_notes: &Option<String>,
) -> Result<WithdrawalRequest, DbError> {
    let approved_at = chrono::Utc::now();

    let request = sqlx::query_as!(
        WithdrawalRequest,
        r#"
        UPDATE withdrawal_requests
        SET state = 'approved',
            kyc_validated = true,
            balance_validated = true,
            validated_at = $2,
            updated_at = $2,
            validation_errors = CASE
                WHEN $3::text IS NOT NULL THEN
                    jsonb_build_object('admin_notes', $3::text)
                ELSE validation_errors
            END
        WHERE id = $1
            AND state = 'pending'
            AND cancelled_at IS NULL
        RETURNING
            id, user_id, user_address, recipient_address, amount, amount_usdc,
            state, kyc_validated, balance_validated, kyc_status_at_request,
            available_balance_at_request, transaction_hash, block_number,
            confirmation_count, required_confirmations, error_message,
            validation_errors, created_at, updated_at, validated_at,
            submitted_at, confirmed_at, failed_at, cancelled_at
        "#,
        request_id,
        approved_at,
        admin_notes.as_deref()
    )
    .fetch_one(pool)
    .await?;

    Ok(request)
}

/// Reject a withdrawal request (admin action)
pub async fn reject_withdrawal_request(
    pool: &PgPool,
    request_id: i32,
    rejection_reason: &str,
    admin_notes: Option<&str>,
) -> Result<WithdrawalRequest, DbError> {
    let rejected_at = chrono::Utc::now();

    let validation_errors = serde_json::json!({
        "rejection_reason": rejection_reason,
        "admin_notes": admin_notes,
        "rejected_at": rejected_at
    });

    let request = sqlx::query_as!(
        WithdrawalRequest,
        r#"
        UPDATE withdrawal_requests
        SET state = 'cancelled',
            kyc_validated = false,
            balance_validated = false,
            cancelled_at = $2,
            updated_at = $2,
            validation_errors = $3,
            error_message = $4
        WHERE id = $1
            AND state = 'pending'
            AND cancelled_at IS NULL
        RETURNING
            id, user_id, user_address, recipient_address, amount, amount_usdc,
            state, kyc_validated, balance_validated, kyc_status_at_request,
            available_balance_at_request, transaction_hash, block_number,
            confirmation_count, required_confirmations, error_message,
            validation_errors, created_at, updated_at, validated_at,
            submitted_at, confirmed_at, failed_at, cancelled_at
        "#,
        request_id,
        rejected_at,
        validation_errors,
        rejection_reason
    )
    .fetch_one(pool)
    .await?;

    Ok(request)
}

/// Admin withdrawal dashboard data (duplicated here to avoid circular import)
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct AdminWithdrawalDashboard {
    pub pending_approval_count: i64,
    pub pending_execution_count: i64,
    pub total_pending_amount: String,
    pub recent_requests: Vec<WithdrawalRequest>,
    pub execution_stats: Option<serde_json::Value>,
}

/// Get admin dashboard stats for withdrawals
pub async fn get_admin_withdrawal_dashboard_stats(
    pool: &PgPool,
) -> Result<AdminWithdrawalDashboard, DbError> {
    // Get counts by status
    let stats = sqlx::query!(
        r#"
        SELECT
            COUNT(CASE WHEN state = 'pending' THEN 1 END) as pending_approval_count,
            COUNT(CASE WHEN state = 'approved' THEN 1 END) as pending_execution_count,
            COALESCE(SUM(CASE WHEN state IN ('pending', 'approved') THEN amount ELSE 0 END), 0) as total_pending_amount
        FROM withdrawal_requests
        WHERE cancelled_at IS NULL
            AND confirmed_at IS NULL
        "#
    )
    .fetch_one(pool)
    .await?;

    // Get recent requests
    let recent_requests = sqlx::query_as!(
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
        WHERE cancelled_at IS NULL
        ORDER BY created_at DESC
        LIMIT 10
        "#
    )
    .fetch_all(pool)
    .await?;

    Ok(AdminWithdrawalDashboard {
        pending_approval_count: stats.pending_approval_count.unwrap_or(0),
        pending_execution_count: stats.pending_execution_count.unwrap_or(0),
        total_pending_amount: stats
            .total_pending_amount
            .unwrap_or(rust_decimal::Decimal::ZERO)
            .to_string(),
        recent_requests,
        execution_stats: None, // Will be filled by the handler if executor is available
    })
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
        WHERE state IN ('pending', 'validated', 'failed')
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

/// Database model for pool borough limits
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::FromRow)]
pub struct PoolBoroughLimit {
    pub id: uuid::Uuid,
    pub pool_id: uuid::Uuid,
    pub borough: String,
    pub max_percentage: rust_decimal::Decimal,
    pub current_allocated: rust_decimal::Decimal,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Request structure for setting pool borough limits
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetPoolBoroughLimitRequest {
    pub pool_id: uuid::Uuid,
    pub borough: String,
    pub max_percentage: rust_decimal::Decimal, // 0.1000 = 10%
}

/// Response structure for borough validation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoroughValidationResult {
    pub is_valid: bool,
    pub current_allocated: rust_decimal::Decimal,
    pub max_allowed: rust_decimal::Decimal,
    pub total_pool_deposits: rust_decimal::Decimal,
    pub percentage_after_loan: rust_decimal::Decimal,
    pub max_percentage: rust_decimal::Decimal,
}

/// Borough summary for a pool
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::FromRow)]
pub struct PoolBoroughSummary {
    pub borough: String,
    pub current_allocated: rust_decimal::Decimal,
    pub max_percentage: rust_decimal::Decimal,
    pub max_allowed: rust_decimal::Decimal,
    pub remaining_capacity: rust_decimal::Decimal,
    pub utilization_percentage: rust_decimal::Decimal,
}

/// Validate pool parameters
pub fn validate_pool_parameters(
    max_ltv_ratio: rust_decimal::Decimal,
    max_interest_rate: rust_decimal::Decimal,
    repayment_period_days: i32,
) -> Result<(), String> {
    use std::str::FromStr;

    // LTV ratio validation: 10% to 95%
    let min_ltv = rust_decimal::Decimal::from_str("0.1000")
        .map_err(|e| format!("Invalid LTV min value: {}", e))?;
    let max_ltv = rust_decimal::Decimal::from_str("0.9500")
        .map_err(|e| format!("Invalid LTV max value: {}", e))?;
    if max_ltv_ratio < min_ltv || max_ltv_ratio > max_ltv {
        return Err(format!(
            "LTV ratio must be between 10% and 95%, got: {}%",
            max_ltv_ratio * rust_decimal::Decimal::from(100)
        ));
    }

    // Interest rate validation: 1% to 50%
    let min_rate = rust_decimal::Decimal::from_str("0.0100")
        .map_err(|e| format!("Invalid interest rate min value: {}", e))?;
    let max_rate = rust_decimal::Decimal::from_str("0.5000")
        .map_err(|e| format!("Invalid interest rate max value: {}", e))?;
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

// ============================================================================
// INVESTOR PROFILE FUNCTIONS
// ============================================================================

/// Create investor profile for a user
pub async fn create_investor_profile(
    pool: &PgPool,
    user_id: uuid::Uuid,
    request: &CreateInvestorProfileRequest,
) -> Result<InvestorProfile, sqlx::Error> {
    let id = uuid::Uuid::new_v4();
    let now = chrono::Utc::now();
    let accredited = request.accredited_investor.unwrap_or(false);
    let jurisdiction = request.regulatory_jurisdiction.as_deref().unwrap_or("US");

    let profile = sqlx::query_as!(
        InvestorProfile,
        r#"
        INSERT INTO investor_profiles (id, user_id, accredited_investor, investment_capacity, regulatory_jurisdiction, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $6)
        RETURNING
            id, user_id, accredited_investor, investment_capacity, regulatory_jurisdiction,
            created_at as "created_at!: DateTime<Utc>",
            updated_at as "updated_at!: DateTime<Utc>"
        "#,
        id,
        user_id,
        accredited,
        request.investment_capacity,
        jurisdiction,
        now
    )
    .fetch_one(pool)
    .await?;

    Ok(profile)
}

/// Get investor profile by user ID
pub async fn get_investor_profile_by_user_id(
    pool: &PgPool,
    user_id: uuid::Uuid,
) -> Result<Option<InvestorProfile>, sqlx::Error> {
    let profile = sqlx::query_as!(
        InvestorProfile,
        r#"
        SELECT
            id, user_id, accredited_investor, investment_capacity, regulatory_jurisdiction,
            created_at as "created_at!: DateTime<Utc>",
            updated_at as "updated_at!: DateTime<Utc>"
        FROM investor_profiles
        WHERE user_id = $1
        "#,
        user_id
    )
    .fetch_optional(pool)
    .await?;

    Ok(profile)
}

/// Update investor profile
pub async fn update_investor_profile(
    pool: &PgPool,
    user_id: uuid::Uuid,
    request: &UpdateInvestorProfileRequest,
) -> Result<InvestorProfile, sqlx::Error> {
    let now = chrono::Utc::now();

    // Build dynamic update query
    let mut set_clauses = vec!["updated_at = $2".to_string()];
    let mut param_count = 2;

    if request.accredited_investor.is_some() {
        param_count += 1;
        set_clauses.push(format!("accredited_investor = ${}", param_count));
    }

    if request.investment_capacity.is_some() {
        param_count += 1;
        set_clauses.push(format!("investment_capacity = ${}", param_count));
    }

    if request.regulatory_jurisdiction.is_some() {
        param_count += 1;
        set_clauses.push(format!("regulatory_jurisdiction = ${}", param_count));
    }

    let query_str = format!(
        r#"
        UPDATE investor_profiles
        SET {}
        WHERE user_id = $1
        RETURNING
            id, user_id, accredited_investor, investment_capacity, regulatory_jurisdiction,
            created_at as "created_at!: DateTime<Utc>",
            updated_at as "updated_at!: DateTime<Utc>"
        "#,
        set_clauses.join(", ")
    );

    let mut query = sqlx::query_as::<_, InvestorProfile>(&query_str)
        .bind(user_id)
        .bind(now);

    if let Some(accredited) = request.accredited_investor {
        query = query.bind(accredited);
    }

    if let Some(capacity) = request.investment_capacity {
        query = query.bind(capacity);
    }

    if let Some(jurisdiction) = &request.regulatory_jurisdiction {
        query = query.bind(jurisdiction);
    }

    let profile = query.fetch_one(pool).await?;
    Ok(profile)
}

/// Get investor profile with identity information
pub async fn get_investor_profile_with_identity(
    pool: &PgPool,
    user_id: uuid::Uuid,
) -> Result<Option<(InvestorProfile, Identity)>, sqlx::Error> {
    let result = sqlx::query!(
        r#"
        SELECT
            ip.id as profile_id, ip.user_id, ip.accredited_investor,
            ip.investment_capacity, ip.regulatory_jurisdiction,
            ip.created_at as "profile_created_at!", ip.updated_at as "profile_updated_at!",
            i.id as identity_id, i.user_type, i.email, i.kyc_status,
            i.kyc_submitted_at, i.kyc_verified_at, i.kyc_investor_type as "kyc_investor_type?: KycInvestorType",
            i.created_at as "identity_created_at!", i.updated_at as "identity_updated_at!",
            i.last_login_at, i.email_verified, i.email_verification_token,
            i.first_name, i.last_name, i.jurisdiction, i.wallet_address
        FROM investor_profiles ip
        JOIN identities i ON ip.user_id = i.id
        WHERE ip.user_id = $1 AND i.deleted_at IS NULL
        "#,
        user_id
    )
    .fetch_optional(pool)
    .await?;

    if let Some(row) = result {
        let profile = InvestorProfile {
            id: row.profile_id,
            user_id: row.user_id,
            accredited_investor: row.accredited_investor,
            investment_capacity: row.investment_capacity,
            regulatory_jurisdiction: row.regulatory_jurisdiction,
            created_at: row.profile_created_at,
            updated_at: row.profile_updated_at,
        };

        let identity = Identity {
            id: row.identity_id,
            user_type: row.user_type,
            email: row.email,
            kyc_status: row.kyc_status,
            kyc_submitted_at: row.kyc_submitted_at,
            kyc_verified_at: row.kyc_verified_at,
            kyc_investor_type: row.kyc_investor_type,
            created_at: row.identity_created_at,
            updated_at: row.identity_updated_at,
            last_login_at: row.last_login_at,
            email_verified: row.email_verified,
            email_verification_token: row.email_verification_token,
            first_name: row.first_name,
            last_name: row.last_name,
            jurisdiction: row.jurisdiction,
            wallet_address: row.wallet_address,
        };

        Ok(Some((profile, identity)))
    } else {
        Ok(None)
    }
}

// ============================================================================
// LIQUIDITY POOL QUERY FUNCTIONS
// ============================================================================

/// Get all liquidity pools with filtering and pagination
pub async fn get_all_liquidity_pools(
    pool: &PgPool,
    status_filter: Option<&str>,
    investor_id_filter: Option<uuid::Uuid>,
    limit: Option<i64>,
    offset: Option<i64>,
    order_by: Option<&str>,
) -> Result<Vec<LiquidityPool>, sqlx::Error> {
    let limit = limit.unwrap_or(50);
    let offset = offset.unwrap_or(0);
    let order_clause = match order_by {
        Some("name") => "ORDER BY name ASC",
        Some("created_at") => "ORDER BY created_at DESC",
        Some("updated_at") => "ORDER BY updated_at DESC",
        Some("total_deposits") => "ORDER BY total_deposits DESC",
        Some("utilization") => {
            "ORDER BY (utilized_liquidity::float / NULLIF(total_deposits::float, 0)) DESC"
        }
        _ => "ORDER BY created_at DESC",
    };

    let mut query = format!(
        r#"
        SELECT
            id, investor_id, name, total_deposits, available_liquidity,
            utilized_liquidity, max_ltv_ratio, max_interest_rate,
            repayment_period_days, status,
            created_at, updated_at
        FROM liquidity_pools
        WHERE 1=1
        "#
    );

    let mut bind_count = 0;

    if status_filter.is_some() {
        bind_count += 1;
        query.push_str(&format!(" AND status = ${}", bind_count));
    }

    if investor_id_filter.is_some() {
        bind_count += 1;
        query.push_str(&format!(" AND investor_id = ${}", bind_count));
    }

    query.push_str(&format!(
        " {} LIMIT ${} OFFSET ${}",
        order_clause,
        bind_count + 1,
        bind_count + 2
    ));

    let mut db_query = sqlx::query_as::<_, LiquidityPool>(&query);

    if let Some(status) = status_filter {
        db_query = db_query.bind(status);
    }
    if let Some(investor_id) = investor_id_filter {
        db_query = db_query.bind(investor_id);
    }

    let pools = db_query.bind(limit).bind(offset).fetch_all(pool).await?;

    Ok(pools)
}

/// Pool summary with calculated utilization rate
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct PoolSummary {
    // Pool basic information
    pub id: uuid::Uuid,
    pub investor_id: uuid::Uuid,
    pub name: String,
    pub total_deposits: rust_decimal::Decimal,
    pub available_liquidity: rust_decimal::Decimal,
    pub utilized_liquidity: rust_decimal::Decimal,
    pub utilization_rate: Option<rust_decimal::Decimal>,
    pub max_ltv_ratio: rust_decimal::Decimal,
    pub max_interest_rate: rust_decimal::Decimal,
    pub repayment_period_days: i32,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    // Loan metrics
    pub total_loans: i64,
    pub active_loans: i64,
    pub total_loan_volume: rust_decimal::Decimal,
    pub average_loan_amount: Option<rust_decimal::Decimal>,
    pub default_rate: rust_decimal::Decimal,
    pub total_interest_earned: rust_decimal::Decimal,
    pub total_principal_repaid: rust_decimal::Decimal,
}

/// Get pool summary with utilization rate calculation
pub async fn get_pool_summary(
    pool: &PgPool,
    pool_id: uuid::Uuid,
) -> Result<Option<PoolSummary>, sqlx::Error> {
    let summary = sqlx::query_as!(
        PoolSummary,
        r#"
        SELECT
            lp.id, lp.investor_id, lp.name, lp.total_deposits, lp.available_liquidity,
            lp.utilized_liquidity,
            CASE
                WHEN lp.total_deposits > 0 THEN (lp.utilized_liquidity / lp.total_deposits)
                ELSE NULL
            END as "utilization_rate",
            lp.max_ltv_ratio, lp.max_interest_rate,
            lp.repayment_period_days, lp.status,
            lp.created_at as "created_at!: DateTime<Utc>",
            lp.updated_at as "updated_at!: DateTime<Utc>",
            -- Loan metrics (default to 0 for now, TODO: calculate from actual loan data)
            0::bigint as "total_loans!",
            0::bigint as "active_loans!",
            0::decimal as "total_loan_volume!",
            NULL::decimal as "average_loan_amount",
            0::decimal as "default_rate!",
            0::decimal as "total_interest_earned!",
            0::decimal as "total_principal_repaid!"
        FROM liquidity_pools lp
        WHERE lp.id = $1
        "#,
        pool_id
    )
    .fetch_optional(pool)
    .await?;

    Ok(summary)
}

/// Get all pool summaries for an investor with aggregated metrics
pub async fn get_investor_pool_summaries(
    pool: &PgPool,
    investor_id: uuid::Uuid,
) -> Result<Vec<PoolSummary>, sqlx::Error> {
    let summaries = sqlx::query_as!(
        PoolSummary,
        r#"
        SELECT
            lp.id, lp.investor_id, lp.name, lp.total_deposits, lp.available_liquidity,
            lp.utilized_liquidity,
            CASE
                WHEN lp.total_deposits > 0 THEN (lp.utilized_liquidity / lp.total_deposits)
                ELSE NULL
            END as "utilization_rate",
            lp.max_ltv_ratio, lp.max_interest_rate,
            lp.repayment_period_days, lp.status,
            lp.created_at as "created_at!: DateTime<Utc>",
            lp.updated_at as "updated_at!: DateTime<Utc>",
            -- Loan metrics (default to 0 for now, TODO: calculate from actual loan data)
            0::bigint as "total_loans!",
            0::bigint as "active_loans!",
            0::decimal as "total_loan_volume!",
            NULL::decimal as "average_loan_amount",
            0::decimal as "default_rate!",
            0::decimal as "total_interest_earned!",
            0::decimal as "total_principal_repaid!"
        FROM liquidity_pools lp
        WHERE lp.investor_id = $1
        ORDER BY lp.created_at DESC
        "#,
        investor_id
    )
    .fetch_all(pool)
    .await?;

    Ok(summaries)
}

/// Investor dashboard metrics
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct InvestorDashboardMetrics {
    pub total_pools: i64,
    pub active_pools: i64,
    pub total_liquidity_provided: rust_decimal::Decimal,
    pub total_utilized_liquidity: rust_decimal::Decimal,
    pub average_utilization_rate: rust_decimal::Decimal,
    pub total_interest_earned: rust_decimal::Decimal,
    pub total_loans_originated: i64,
    pub total_loan_volume: rust_decimal::Decimal,
}

/// Get investor dashboard metrics
pub async fn get_investor_dashboard_metrics(
    pool: &PgPool,
    investor_id: uuid::Uuid,
) -> Result<InvestorDashboardMetrics, sqlx::Error> {
    let metrics = sqlx::query_as!(
        InvestorDashboardMetrics,
        r#"
        SELECT
            COUNT(lp.*)::int8 as "total_pools!",
            COUNT(CASE WHEN lp.status = 'ACTIVE' THEN 1 END)::int8 as "active_pools!",
            COALESCE(SUM(lp.total_deposits), 0) as "total_liquidity_provided!",
            COALESCE(SUM(lp.utilized_liquidity), 0) as "total_utilized_liquidity!",
            CASE
                WHEN SUM(lp.total_deposits) > 0 THEN SUM(lp.utilized_liquidity) / SUM(lp.total_deposits)
                ELSE 0
            END as "average_utilization_rate!",
            -- Calculate total interest earned from pool financial transactions
            COALESCE((
                SELECT SUM(amount)
                FROM pool_financial_transactions pft
                WHERE pft.transaction_type = 'INTEREST'
                AND pft.pool_id IN (
                    SELECT id FROM liquidity_pools WHERE investor_id = $1
                )
            ), 0) as "total_interest_earned!",
            -- Calculate total loans originated from credit line draws linked to investor's pools
            COALESCE((
                SELECT COUNT(DISTINCT clft.id)
                FROM credit_line_financial_transactions clft
                JOIN credit_lines cl ON clft.credit_line_id = cl.id
                WHERE clft.transaction_type = 'DRAW'
                AND cl.pool_id IN (
                    SELECT id FROM liquidity_pools WHERE investor_id = $1
                )
            ), 0)::int8 as "total_loans_originated!",
            -- Calculate total loan volume from credit line draws
            COALESCE((
                SELECT SUM(clft.amount)
                FROM credit_line_financial_transactions clft
                JOIN credit_lines cl ON clft.credit_line_id = cl.id
                WHERE clft.transaction_type = 'DRAW'
                AND cl.pool_id IN (
                    SELECT id FROM liquidity_pools WHERE investor_id = $1
                )
            ), 0) as "total_loan_volume!"
        FROM liquidity_pools lp
        WHERE lp.investor_id = $1
        "#,
        investor_id
    )
    .fetch_one(pool)
    .await?;

    Ok(metrics)
}

/// Enhanced investor metrics including APR and yield calculations
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct EnhancedInvestorMetrics {
    pub investor_id: uuid::Uuid,
    pub total_pools: i64,
    pub active_pools: i64,
    pub total_liquidity_provided: rust_decimal::Decimal,
    pub total_utilized_liquidity: rust_decimal::Decimal,
    pub average_utilization_rate: rust_decimal::Decimal,
    pub total_interest_earned: rust_decimal::Decimal,
    pub total_loans_originated: i64,
    pub total_loan_volume: rust_decimal::Decimal,
    pub lifetime_apr: Option<rust_decimal::Decimal>, // Annual Percentage Rate
    pub lifetime_yield: Option<rust_decimal::Decimal>, // Total return percentage
    pub average_loan_apr: Option<rust_decimal::Decimal>, // Average APR across all loans
    pub average_loan_duration_days: Option<i32>,     // Average loan duration in days
    pub total_principal_repaid: rust_decimal::Decimal,
    pub total_outstanding_principal: rust_decimal::Decimal,
    pub default_rate: rust_decimal::Decimal,
}

/// Investor pool metrics dashboard structure
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct InvestorPoolMetrics {
    pub investor_id: uuid::Uuid,
    pub total_pools: i64,
    pub active_pools: i64,
    pub paused_pools: i64,
    pub active_total_capital: rust_decimal::Decimal,
    pub total_capital_deployed: rust_decimal::Decimal,
    pub lifetime_percentage_yield: Option<rust_decimal::Decimal>,
    pub current_annual_yield_apr: Option<rust_decimal::Decimal>,
    pub total_interest_earned: rust_decimal::Decimal,
    pub average_utilization_rate: rust_decimal::Decimal,
    pub total_loans_originated: i64,
    pub default_rate: rust_decimal::Decimal,
    pub created_at: DateTime<Utc>,
}

/// Get investor pool metrics for dashboard display
pub async fn get_investor_pool_metrics(
    pool: &PgPool,
    investor_id: uuid::Uuid,
) -> Result<InvestorPoolMetrics, sqlx::Error> {
    let row = sqlx::query!(
        r#"
        SELECT
            COUNT(lp.*)::int8 as "total_pools!",
            COUNT(CASE WHEN lp.status = 'ACTIVE' THEN 1 END)::int8 as "active_pools!",
            COUNT(CASE WHEN lp.status = 'PAUSED' THEN 1 END)::int8 as "paused_pools!",
            COALESCE(SUM(CASE WHEN lp.status = 'ACTIVE' THEN lp.total_deposits ELSE 0 END), 0) as "active_total_capital!",
            COALESCE(SUM(lp.utilized_liquidity), 0) as "total_capital_deployed!",
            -- Lifetime percentage yield: total interest earned / total capital provided
            CASE
                WHEN SUM(lp.total_deposits) > 0 THEN
                    (COALESCE((
                        SELECT SUM(amount)
                        FROM pool_financial_transactions pft
                        WHERE pft.transaction_type = 'INTEREST'
                        AND pft.pool_id IN (
                            SELECT id FROM liquidity_pools WHERE investor_id = $1
                        )
                    ), 0) / SUM(lp.total_deposits)) * 100
                ELSE NULL
            END as "lifetime_percentage_yield",
            -- Current annual yield (APR): annualized based on average pool age
            CASE
                WHEN SUM(lp.total_deposits) > 0 AND AVG(EXTRACT(EPOCH FROM (NOW() - lp.created_at)) / 86400) > 0 THEN
                    (COALESCE((
                        SELECT SUM(amount)
                        FROM pool_financial_transactions pft
                        WHERE pft.transaction_type = 'INTEREST'
                        AND pft.pool_id IN (
                            SELECT id FROM liquidity_pools WHERE investor_id = $1
                        )
                    ), 0) / SUM(lp.total_deposits)) * (365 / AVG(EXTRACT(EPOCH FROM (NOW() - lp.created_at)) / 86400)) * 100
                ELSE NULL
            END as "current_annual_yield_apr",
            COALESCE((
                SELECT SUM(amount)
                FROM pool_financial_transactions pft
                WHERE pft.transaction_type = 'INTEREST'
                AND pft.pool_id IN (
                    SELECT id FROM liquidity_pools WHERE investor_id = $1
                )
            ), 0) as "total_interest_earned!",
            CASE
                WHEN SUM(lp.total_deposits) > 0 THEN (SUM(lp.utilized_liquidity) / SUM(lp.total_deposits)) * 100
                ELSE 0
            END as "average_utilization_rate!",
            COALESCE((
                SELECT COUNT(DISTINCT clft.id)
                FROM credit_line_financial_transactions clft
                JOIN credit_lines cl ON clft.credit_line_id = cl.id
                WHERE clft.transaction_type = 'DRAW'
                AND cl.pool_id IN (
                    SELECT id FROM liquidity_pools WHERE investor_id = $1
                )
            ), 0)::int8 as "total_loans_originated!",
            COALESCE((
                SELECT
                    CASE
                        WHEN COUNT(*) > 0 THEN (COUNT(CASE WHEN cl.status = 'DEFAULTED' THEN 1 END)::decimal / COUNT(*)::decimal) * 100
                        ELSE 0
                    END
                FROM credit_line_financial_transactions clft
                JOIN credit_lines cl ON clft.credit_line_id = cl.id
                WHERE clft.transaction_type = 'DRAW'
                AND cl.pool_id IN (
                    SELECT id FROM liquidity_pools WHERE investor_id = $1
                )
            ), 0) as "default_rate!",
            COALESCE(MIN(lp.created_at), NOW()) as "created_at!: DateTime<Utc>"
        FROM liquidity_pools lp
        WHERE lp.investor_id = $1
        "#,
        investor_id
    )
    .fetch_one(pool)
    .await?;

    Ok(InvestorPoolMetrics {
        investor_id,
        total_pools: row.total_pools,
        active_pools: row.active_pools,
        paused_pools: row.paused_pools,
        active_total_capital: row.active_total_capital,
        total_capital_deployed: row.total_capital_deployed,
        lifetime_percentage_yield: row.lifetime_percentage_yield,
        current_annual_yield_apr: row.current_annual_yield_apr,
        total_interest_earned: row.total_interest_earned,
        average_utilization_rate: row.average_utilization_rate,
        total_loans_originated: row.total_loans_originated,
        default_rate: row.default_rate,
        created_at: row.created_at,
    })
}

/// Get enhanced investor metrics with APR and yield calculations
/* pub async fn get_enhanced_investor_metrics(
    pool: &PgPool,
    investor_id: uuid::Uuid,
) -> Result<EnhancedInvestorMetrics, sqlx::Error> {
    let metrics = sqlx::query_as!(
        EnhancedInvestorMetrics,
        r#"
        WITH pool_stats AS (
            SELECT
                lp.id,
                lp.total_deposits,
                lp.utilized_liquidity,
                lp.created_at,
                EXTRACT(EPOCH FROM (NOW() - lp.created_at)) / 86400 as pool_age_days
            FROM liquidity_pools lp
            WHERE lp.investor_id = $1
        ),
        loan_stats AS (
            SELECT
                cl.pool_id,
                clft.amount as draw_amount,
                clft.processed_at as draw_date,
                -- Calculate interest payments for this loan
                COALESCE((
                    SELECT SUM(amount)
                    FROM credit_line_financial_transactions clft2
                    WHERE clft2.credit_line_id = cl.id
                    AND clft2.transaction_type = 'INTEREST'
                ), 0) as interest_earned,
                -- Calculate principal repayments
                COALESCE((
                    SELECT SUM(amount)
                    FROM credit_line_financial_transactions clft2
                    WHERE clft2.credit_line_id = cl.id
                    AND clft2.transaction_type = 'PAYMENT'
                ), 0) as principal_repaid,
                -- Current outstanding balance
                GREATEST(cl.utilized_amount, 0) as outstanding_balance,
                -- Loan duration in days
                EXTRACT(EPOCH FROM (NOW() - cl.opened_at)) / 86400 as loan_age_days,
                -- Default flag (simplified - loan with no payments for 90+ days)
                CASE
                    WHEN cl.status = 'DEFAULTED' THEN 1
                    WHEN cl.status = 'ACTIVE' AND EXTRACT(EPOCH FROM (NOW() - cl.opened_at)) / 86400 > 90
                         AND NOT EXISTS (
                             SELECT 1 FROM credit_line_financial_transactions clft3
                             WHERE clft3.credit_line_id = cl.id
                             AND clft3.transaction_type IN ('PAYMENT', 'INTEREST')
                             AND clft3.processed_at > NOW() - INTERVAL '90 days'
                         ) THEN 1
                    ELSE 0
                END as is_defaulted
            FROM credit_lines cl
            JOIN credit_line_financial_transactions clft ON cl.id = clft.credit_line_id
            WHERE cl.pool_id IN (SELECT id FROM pool_stats)
            AND clft.transaction_type = 'DRAW'
        ),
        aggregate_metrics AS (
            SELECT
                COUNT(ps.*)::int8 as total_pools,
                COUNT(CASE WHEN ps.total_deposits > 0 THEN 1 END)::int8 as active_pools,
                COALESCE(SUM(ps.total_deposits), 0) as total_liquidity_provided,
                COALESCE(SUM(ps.utilized_liquidity), 0) as total_utilized_liquidity,
                CASE
                    WHEN SUM(ps.total_deposits) > 0 THEN SUM(ps.utilized_liquidity) / SUM(ps.total_deposits)
                    ELSE 0
                END as average_utilization_rate,
                COALESCE(SUM(ls.interest_earned), 0) as total_interest_earned,
                COUNT(DISTINCT ls.pool_id)::int8 as total_loans_originated,
                COALESCE(SUM(ls.draw_amount), 0) as total_loan_volume,
                COALESCE(SUM(ls.principal_repaid), 0) as total_principal_repaid,
                COALESCE(SUM(ls.outstanding_balance), 0) as total_outstanding_principal,
                CASE
                    WHEN COUNT(ls.*) > 0 THEN (SUM(ls.is_defaulted)::decimal / COUNT(ls.*)::decimal) * 100
                    ELSE 0
                END as default_rate_percentage,
                AVG(ls.loan_age_days) as average_loan_duration_days
            FROM pool_stats ps
            LEFT JOIN loan_stats ls ON ps.id = ls.pool_id
        )
        SELECT
            $1::uuid as investor_id,
            am.total_pools,
            am.active_pools,
            am.total_liquidity_provided,
            am.total_utilized_liquidity,
            am.average_utilization_rate,
            am.total_interest_earned,
            am.total_loans_originated,
            am.total_loan_volume,
            am.total_principal_repaid,
            am.total_outstanding_principal,
            am.default_rate_percentage as default_rate,
            am.average_loan_duration_days::int4 as average_loan_duration_days,
            -- Calculate lifetime APR: (total interest earned / total liquidity provided) * (365 / average pool age)
            CASE
                WHEN am.total_liquidity_provided > 0 AND (SELECT AVG(pool_age_days) FROM pool_stats) > 0 THEN
                    (am.total_interest_earned / am.total_liquidity_provided) * (365 / (SELECT AVG(pool_age_days) FROM pool_stats))
                ELSE NULL
            END as lifetime_apr,
            -- Calculate lifetime yield: total interest earned / total liquidity provided
            CASE
                WHEN am.total_liquidity_provided > 0 THEN am.total_interest_earned / am.total_liquidity_provided
                ELSE NULL
            END as lifetime_yield,
            -- Calculate average loan APR: (total interest earned / total loan volume) * (365 / average loan duration)
            CASE
                WHEN am.total_loan_volume > 0 AND am.average_loan_duration_days > 0 THEN
                    (am.total_interest_earned / am.total_loan_volume) * (365 / am.average_loan_duration_days)
                ELSE NULL
            END as average_loan_apr
        FROM aggregate_metrics am
        "#,
        investor_id
    )
    .fetch_one(pool)
    .await?;

    Ok(metrics)
} */

/// Pool utilization data point for historical analysis
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct PoolUtilizationData {
    pub timestamp: DateTime<Utc>,
    pub total_deposits: rust_decimal::Decimal,
    pub available_liquidity: rust_decimal::Decimal,
    pub utilized_liquidity: rust_decimal::Decimal,
    pub utilization_rate: Option<rust_decimal::Decimal>,
}

/// Get pool utilization history (for now, returns current state - can be extended with audit table)
pub async fn get_pool_utilization_history(
    pool: &PgPool,
    pool_id: uuid::Uuid,
    _period_days: i32,
) -> Result<Vec<PoolUtilizationData>, sqlx::Error> {
    // For now, return current state. This can be extended with historical data from audit logs
    let current_data = sqlx::query_as!(
        PoolUtilizationData,
        r#"
        SELECT
            updated_at as "timestamp!: DateTime<Utc>",
            total_deposits,
            available_liquidity,
            utilized_liquidity,
            CASE
                WHEN total_deposits > 0 THEN (utilized_liquidity / total_deposits)
                ELSE NULL
            END as "utilization_rate"
        FROM liquidity_pools
        WHERE id = $1
        "#,
        pool_id
    )
    .fetch_all(pool)
    .await?;

    Ok(current_data)
}

/// Pool transaction record (simplified - can be extended with actual transaction tracking)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct PoolTransaction {
    pub transaction_id: uuid::Uuid,
    pub pool_id: uuid::Uuid,
    pub operation_type: String,
    pub amount: rust_decimal::Decimal,
    pub user_id: uuid::Uuid,
    pub timestamp: DateTime<Utc>,
    pub metadata: Option<String>,
}

/// Get pool transaction history (placeholder - would need actual transaction tracking table)
pub async fn get_pool_transactions(
    pool: &PgPool,
    pool_id: uuid::Uuid,
    transaction_type_filter: Option<&str>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<PoolTransaction>, sqlx::Error> {
    // For now, return pool audit events as transactions
    // This could be extended with a proper transactions table
    let limit = limit.unwrap_or(50);
    let offset = offset.unwrap_or(0);

    let mut query = r#"
        SELECT
            id as transaction_id,
            pool_id,
            operation_type,
            COALESCE(
                (new_values->>'total_deposits')::decimal - COALESCE((old_values->>'total_deposits')::decimal, 0),
                0
            ) as amount,
            COALESCE((metadata->>'user_id')::uuid, '00000000-0000-0000-0000-000000000000'::uuid) as user_id,
            created_at as timestamp,
            metadata::text as metadata
        FROM liquidity_pool_audit
        WHERE pool_id = $1
    "#.to_string();

    let mut bind_count = 1;

    if transaction_type_filter.is_some() {
        bind_count += 1;
        query.push_str(&format!(" AND operation_type = ${}", bind_count));
    }

    query.push_str(&format!(
        " ORDER BY created_at DESC LIMIT ${} OFFSET ${}",
        bind_count + 1,
        bind_count + 2
    ));

    let mut db_query = sqlx::query_as::<_, PoolTransaction>(&query).bind(pool_id);

    if let Some(tx_type) = transaction_type_filter {
        db_query = db_query.bind(tx_type);
    }

    let transactions = db_query.bind(limit).bind(offset).fetch_all(pool).await?;

    Ok(transactions)
}

/// Market overview metrics
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct MarketOverviewMetrics {
    pub total_pools: i64,
    pub active_pools: i64,
    pub total_liquidity: rust_decimal::Decimal,
    pub total_utilized_liquidity: rust_decimal::Decimal,
    pub platform_utilization_rate: rust_decimal::Decimal,
    pub total_loans: i64,
    pub active_loans: i64,
    pub total_loan_volume: rust_decimal::Decimal,
    pub average_interest_rate: rust_decimal::Decimal,
    pub platform_default_rate: rust_decimal::Decimal,
}

/// Get market overview statistics
pub async fn get_market_overview_metrics(
    pool: &PgPool,
) -> Result<MarketOverviewMetrics, sqlx::Error> {
    let metrics = sqlx::query_as!(
        MarketOverviewMetrics,
        r#"
        SELECT
            COUNT(*)::int8 as "total_pools!",
            COUNT(CASE WHEN status = 'active' THEN 1 END)::int8 as "active_pools!",
            COALESCE(SUM(total_deposits), 0) as "total_liquidity!",
            COALESCE(SUM(utilized_liquidity), 0) as "total_utilized_liquidity!",
            CASE
                WHEN SUM(total_deposits) > 0 THEN SUM(utilized_liquidity) / SUM(total_deposits)
                ELSE 0
            END as "platform_utilization_rate!",
            -- TODO: Calculate from actual loan data when available
            0::int8 as "total_loans!",
            0::int8 as "active_loans!",
            0::decimal as "total_loan_volume!",
            COALESCE(AVG(max_interest_rate), 0) as "average_interest_rate!",
            0::decimal as "platform_default_rate!"
        FROM liquidity_pools
        "#,
    )
    .fetch_one(pool)
    .await?;

    Ok(metrics)
}

// ============================================================================
// MERCHANT FUNCTIONS
// ============================================================================

/// Create a new merchant account
pub async fn create_merchant(
    pool: &PgPool,
    user_id: uuid::Uuid,
    request: &CreateMerchantRequest,
) -> Result<Merchant, sqlx::Error> {
    let id = uuid::Uuid::new_v4();

    let result = sqlx::query!(
        r#"
        INSERT INTO merchants (id, user_id, legal_name, dba, tax_id, mcc, country, kyc_status, business_type, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NOW(), NOW())
        RETURNING
            id, user_id, legal_name, dba, tax_id, mcc,
            country as "country!: String",
            kyc_status as "kyc_status!: String",
            business_type,
            created_at, updated_at
        "#,
        id,
        user_id,
        request.legal_name,
        request.dba,
        request.tax_id,
        request.mcc,
        request.country,
        "NOT_SUBMITTED", // Default KYC status
        request.business_type,
    )
    .fetch_one(pool)
    .await?;

    Ok(Merchant {
        id: result.id,
        user_id: result.user_id,
        legal_name: result.legal_name,
        dba: result.dba,
        tax_id: result.tax_id,
        mcc: result.mcc,
        country: result.country,
        kyc_status: result.kyc_status,
        business_type: result.business_type,
        borough: None, // New merchants don't have borough set initially
        created_at: result
            .created_at
            .expect("created_at should not be null")
            .and_utc(),
        updated_at: result
            .updated_at
            .expect("updated_at should not be null")
            .and_utc(),
    })
}

/// Get merchant by user ID
pub async fn get_merchant_by_user_id(
    pool: &PgPool,
    user_id: uuid::Uuid,
) -> Result<Option<Merchant>, sqlx::Error> {
    let result = sqlx::query!(
        r#"
        SELECT
            id, user_id, legal_name, dba, tax_id, mcc,
            country as "country!: String",
            kyc_status as "kyc_status!: String",
            business_type,
            borough,
            created_at, updated_at
        FROM merchants
        WHERE user_id = $1
        "#,
        user_id
    )
    .fetch_optional(pool)
    .await?;

    if let Some(row) = result {
        Ok(Some(Merchant {
            id: row.id,
            user_id: row.user_id,
            legal_name: row.legal_name,
            dba: row.dba,
            tax_id: row.tax_id,
            mcc: row.mcc,
            country: row.country,
            kyc_status: row.kyc_status,
            business_type: row.business_type,
            borough: row.borough,
            created_at: row
                .created_at
                .expect("created_at should not be null")
                .and_utc(),
            updated_at: row
                .updated_at
                .expect("updated_at should not be null")
                .and_utc(),
        }))
    } else {
        Ok(None)
    }
}

/// Get merchant by ID
pub async fn get_merchant_by_id(
    pool: &PgPool,
    merchant_id: uuid::Uuid,
) -> Result<Option<Merchant>, sqlx::Error> {
    // For now, use a simpler approach - just implement the basic functionality
    let result = sqlx::query!(
        r#"
        SELECT
            id, user_id, legal_name, dba, tax_id, mcc,
            country as "country!: String",
            kyc_status as "kyc_status!: String",
            business_type,
            borough,
            created_at, updated_at
        FROM merchants
        WHERE id = $1
        "#,
        merchant_id
    )
    .fetch_optional(pool)
    .await?;

    if let Some(row) = result {
        Ok(Some(Merchant {
            id: row.id,
            user_id: row.user_id,
            legal_name: row.legal_name,
            dba: row.dba,
            tax_id: row.tax_id,
            mcc: row.mcc,
            country: row.country,
            kyc_status: row.kyc_status,
            business_type: row.business_type,
            borough: row.borough,
            created_at: row
                .created_at
                .expect("created_at should not be null")
                .and_utc(),
            updated_at: row
                .updated_at
                .expect("updated_at should not be null")
                .and_utc(),
        }))
    } else {
        Ok(None)
    }
}

/// Get merchant by email - finds identity by email, then gets merchant by user_id
pub async fn get_merchant_by_email(
    pool: &PgPool,
    email: &str,
) -> Result<Option<Merchant>, sqlx::Error> {
    // First get the identity by email
    let identity_opt = get_identity_by_email(pool, email).await?;

    match identity_opt {
        Some(identity) => {
            // Then get the merchant by user_id
            get_merchant_by_user_id(pool, identity.id).await
        }
        None => Ok(None),
    }
}

/// Create merchant profile - simplified implementation
pub async fn create_merchant_profile(
    pool: &PgPool,
    merchant_id: uuid::Uuid,
    request: &CreateMerchantProfileRequest,
) -> Result<MerchantProfile, sqlx::Error> {
    let id = uuid::Uuid::new_v4();

    let result = sqlx::query!(
        r#"
        INSERT INTO merchant_profiles (id, merchant_id, industry, years_in_business, employee_count, monthly_revenue_estimate, business_description, profile_completed, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, false, NOW(), NOW())
        RETURNING
            id, merchant_id, industry, years_in_business, employee_count, monthly_revenue_estimate, business_description, profile_completed, created_at, updated_at
        "#,
        id,
        merchant_id,
        request.industry,
        request.years_in_business,
        request.employee_count,
        request.monthly_revenue_estimate,
        request.business_description,
    )
    .fetch_one(pool)
    .await?;

    Ok(MerchantProfile {
        id: result.id,
        merchant_id: result.merchant_id,
        industry: result.industry,
        years_in_business: result.years_in_business,
        employee_count: result.employee_count,
        monthly_revenue_estimate: result.monthly_revenue_estimate,
        business_description: result.business_description,
        profile_completed: result.profile_completed.unwrap_or_default(),
        created_at: result
            .created_at
            .expect("created_at should not be null")
            .and_utc(),
        updated_at: result
            .updated_at
            .expect("updated_at should not be null")
            .and_utc(),
    })
}

/// Get merchant profile by merchant ID - simplified
pub async fn get_merchant_profile_by_merchant_id(
    pool: &PgPool,
    merchant_id: uuid::Uuid,
) -> Result<Option<MerchantProfile>, sqlx::Error> {
    let result = sqlx::query!(
        r#"
        SELECT
            id, merchant_id, industry, years_in_business, employee_count,
            monthly_revenue_estimate, business_description, profile_completed,
            created_at, updated_at
        FROM merchant_profiles
        WHERE merchant_id = $1
        "#,
        merchant_id
    )
    .fetch_optional(pool)
    .await?;

    if let Some(row) = result {
        Ok(Some(MerchantProfile {
            id: row.id,
            merchant_id: row.merchant_id,
            industry: row.industry,
            years_in_business: row.years_in_business,
            employee_count: row.employee_count,
            monthly_revenue_estimate: row.monthly_revenue_estimate,
            business_description: row.business_description,
            profile_completed: row.profile_completed.unwrap_or_default(),
            created_at: row
                .created_at
                .expect("created_at should not be null")
                .and_utc(),
            updated_at: row
                .updated_at
                .expect("updated_at should not be null")
                .and_utc(),
        }))
    } else {
        Ok(None)
    }
}

/// Update merchant profile
pub async fn update_merchant_profile(
    pool: &PgPool,
    merchant_id: uuid::Uuid,
    request: &UpdateMerchantProfileRequest,
) -> Result<Option<MerchantProfile>, sqlx::Error> {
    // First update the merchant basic info if provided
    if request.legal_name.is_some()
        || request.dba.is_some()
        || request.mcc.is_some()
        || request.country.is_some()
        || request.business_type.is_some()
    {
        sqlx::query!(
            r#"
            UPDATE merchants
            SET
                legal_name = COALESCE($2, legal_name),
                dba = COALESCE($3, dba),
                mcc = COALESCE($4, mcc),
                country = COALESCE($5, country),
                business_type = COALESCE($6, business_type),
                updated_at = NOW()
            WHERE id = $1
            "#,
            merchant_id,
            request.legal_name,
            request.dba,
            request.mcc,
            request.country,
            request.business_type
        )
        .execute(pool)
        .await?;
    }

    // Then update the merchant profile if it exists
    let profile = sqlx::query_as!(
        MerchantProfile,
        r#"
        UPDATE merchant_profiles
        SET
            industry = COALESCE($2, industry),
            years_in_business = COALESCE($3, years_in_business),
            employee_count = COALESCE($4, employee_count),
            monthly_revenue_estimate = COALESCE($5, monthly_revenue_estimate),
            business_description = COALESCE($6, business_description),
            profile_completed = CASE
                WHEN $2 IS NOT NULL OR $3 IS NOT NULL OR $4 IS NOT NULL OR $5 IS NOT NULL OR $6 IS NOT NULL
                THEN true
                ELSE profile_completed
            END,
            updated_at = NOW()
        WHERE merchant_id = $1
        RETURNING
            id, merchant_id,
            industry,
            years_in_business,
            employee_count,
            monthly_revenue_estimate,
            business_description,
            profile_completed as "profile_completed!: bool",
            created_at as "created_at!: DateTime<Utc>",
            updated_at as "updated_at!: DateTime<Utc>"
        "#,
        merchant_id,
        request.industry,
        request.years_in_business,
        request.employee_count,
        request.monthly_revenue_estimate,
        request.business_description
    )
    .fetch_optional(pool)
    .await?;

    Ok(profile)
}

/// Get merchant with profile by user ID
pub async fn get_merchant_with_profile_by_user_id(
    pool: &PgPool,
    user_id: uuid::Uuid,
) -> Result<Option<(Merchant, Option<MerchantProfile>)>, sqlx::Error> {
    // First get the merchant
    let merchant_opt = get_merchant_by_user_id(pool, user_id).await?;

    if let Some(merchant) = merchant_opt {
        // Then get the profile if it exists
        let profile_opt = get_merchant_profile_by_merchant_id(pool, merchant.id).await?;
        Ok(Some((merchant, profile_opt)))
    } else {
        Ok(None)
    }
}

// ============================================================================
// MERCHANT TURNOVER TRACKING
// ============================================================================

/// Daily turnover record for merchant business activity
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize, serde::Deserialize)]
pub struct MerchantTurnoverDaily {
    pub id: uuid::Uuid,
    pub merchant_id: uuid::Uuid,
    pub date: chrono::NaiveDate,
    pub gross_receipts: rust_decimal::Decimal,
    pub refunds: rust_decimal::Decimal,
    pub net_receipts: rust_decimal::Decimal,
    pub source: String,
    pub verified: bool,
    pub verification_method: Option<String>,
    pub currency: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Request to create/update daily turnover data
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct CreateTurnoverRequest {
    pub date: chrono::NaiveDate,
    pub gross_receipts: rust_decimal::Decimal,
    pub refunds: Option<rust_decimal::Decimal>,
    pub source: Option<String>,
    pub verification_method: Option<String>,
}

/// Request to update existing turnover data
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct UpdateTurnoverRequest {
    pub gross_receipts: Option<rust_decimal::Decimal>,
    pub refunds: Option<rust_decimal::Decimal>,
    pub verification_method: Option<String>,
}

/// Turnover analytics summary
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize, serde::Deserialize)]
pub struct MerchantTurnoverAnalytics {
    pub merchant_id: uuid::Uuid,
    pub legal_name: String,
    pub total_days_recorded: i64,
    pub verified_days: i64,
    pub total_net_receipts: Option<rust_decimal::Decimal>,
    pub avg_daily_receipts: Option<rust_decimal::Decimal>,
    pub last_turnover_date: Option<chrono::NaiveDate>,
    pub first_turnover_date: Option<chrono::NaiveDate>,
    pub monthly_estimate: Option<rust_decimal::Decimal>,
    pub data_quality_score: i32,
}

/// Profile completion status for merchant eligibility assessment
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize, serde::Deserialize)]
pub struct MerchantProfileCompletion {
    pub merchant_id: uuid::Uuid,
    pub legal_name: String,
    pub kyc_status: String,
    pub has_legal_name: bool,
    pub has_tax_id: bool,
    pub has_mcc: bool,
    pub has_industry: bool,
    pub has_years_in_business: bool,
    pub has_revenue_estimate: bool,
    pub kyc_approved: bool,
    pub has_sufficient_turnover_history: bool,
    pub has_verified_turnover_data: bool,
    pub has_bank_statement: bool,
    pub completion_score: i32,
}

/// Business document record
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize, serde::Deserialize)]
pub struct MerchantDocument {
    pub id: uuid::Uuid,
    pub merchant_id: uuid::Uuid,
    pub document_type: String,
    pub document_url: Option<String>,
    pub document_hash: Option<String>,
    pub file_size_bytes: Option<i32>,
    pub original_filename: Option<String>,
    pub mime_type: Option<String>,
    pub uploaded_at: DateTime<Utc>,
    pub verified_at: Option<DateTime<Utc>>,
    pub status: String,
    pub verification_notes: Option<String>,
    pub created_by: Option<uuid::Uuid>,
    pub updated_at: DateTime<Utc>,
}

/// Request to create a document record
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct CreateDocumentRequest {
    pub document_type: String,
    pub document_url: Option<String>,
    pub document_hash: Option<String>,
    pub file_size_bytes: Option<i32>,
    pub original_filename: Option<String>,
    pub mime_type: Option<String>,
}

/// Create or update daily turnover record
pub async fn create_or_update_turnover(
    pool: &PgPool,
    merchant_id: uuid::Uuid,
    request: &CreateTurnoverRequest,
) -> Result<MerchantTurnoverDaily, sqlx::Error> {
    let refunds = request
        .refunds
        .unwrap_or_else(|| rust_decimal::Decimal::ZERO);
    let net_receipts = request.gross_receipts - refunds;
    let source = request
        .source
        .clone()
        .unwrap_or_else(|| "MANUAL".to_string());

    let turnover = sqlx::query_as!(
        MerchantTurnoverDaily,
        r#"
        INSERT INTO merchant_turnover_daily (
            merchant_id, date, gross_receipts, refunds, net_receipts, source, verification_method
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        ON CONFLICT (merchant_id, date)
        DO UPDATE SET
            gross_receipts = EXCLUDED.gross_receipts,
            refunds = EXCLUDED.refunds,
            net_receipts = EXCLUDED.net_receipts,
            source = EXCLUDED.source,
            verification_method = EXCLUDED.verification_method,
            updated_at = NOW()
        RETURNING
            id, merchant_id, date,
            gross_receipts as "gross_receipts!: rust_decimal::Decimal",
            refunds as "refunds!: rust_decimal::Decimal",
            net_receipts as "net_receipts!: rust_decimal::Decimal",
            source as "source!: String",
            verified as "verified!: bool",
            verification_method,
            currency as "currency!: String",
            created_at as "created_at!: DateTime<Utc>",
            updated_at as "updated_at!: DateTime<Utc>"
        "#,
        merchant_id,
        request.date,
        request.gross_receipts,
        refunds,
        net_receipts,
        source,
        request.verification_method
    )
    .fetch_one(pool)
    .await?;

    Ok(turnover)
}

/// Update existing turnover record
pub async fn update_turnover(
    pool: &PgPool,
    merchant_id: uuid::Uuid,
    date: chrono::NaiveDate,
    request: &UpdateTurnoverRequest,
) -> Result<Option<MerchantTurnoverDaily>, sqlx::Error> {
    let turnover = sqlx::query_as!(
        MerchantTurnoverDaily,
        r#"
        UPDATE merchant_turnover_daily
        SET
            gross_receipts = COALESCE($3, gross_receipts),
            refunds = COALESCE($4, refunds),
            net_receipts = COALESCE($3, gross_receipts) - COALESCE($4, refunds),
            verification_method = COALESCE($5, verification_method),
            updated_at = NOW()
        WHERE merchant_id = $1 AND date = $2
        RETURNING
            id, merchant_id, date,
            gross_receipts as "gross_receipts!: rust_decimal::Decimal",
            refunds as "refunds!: rust_decimal::Decimal",
            net_receipts as "net_receipts!: rust_decimal::Decimal",
            source as "source!: String",
            verified as "verified!: bool",
            verification_method,
            currency as "currency!: String",
            created_at as "created_at!: DateTime<Utc>",
            updated_at as "updated_at!: DateTime<Utc>"
        "#,
        merchant_id,
        date,
        request.gross_receipts,
        request.refunds,
        request.verification_method
    )
    .fetch_optional(pool)
    .await?;

    Ok(turnover)
}

/// Get turnover records for a merchant with optional filtering
pub async fn get_merchant_turnover(
    pool: &PgPool,
    merchant_id: uuid::Uuid,
    date_from: Option<chrono::NaiveDate>,
    date_to: Option<chrono::NaiveDate>,
    verified_only: Option<bool>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<MerchantTurnoverDaily>, sqlx::Error> {
    let mut query = sqlx::QueryBuilder::new(
        r#"
        SELECT id, merchant_id, date, gross_receipts, refunds, net_receipts, source,
               verified, verification_method, currency, created_at, updated_at
        FROM merchant_turnover_daily
        WHERE merchant_id =
        "#,
    );

    query.push_bind(merchant_id);

    if let Some(from_date) = date_from {
        query.push(" AND date >= ");
        query.push_bind(from_date);
    }

    if let Some(to_date) = date_to {
        query.push(" AND date <= ");
        query.push_bind(to_date);
    }

    if let Some(verified) = verified_only {
        query.push(" AND verified = ");
        query.push_bind(verified);
    }

    query.push(" ORDER BY date DESC");

    if let Some(limit_val) = limit {
        query.push(" LIMIT ");
        query.push_bind(limit_val);
    }

    if let Some(offset_val) = offset {
        query.push(" OFFSET ");
        query.push_bind(offset_val);
    }

    let turnover = query
        .build_query_as::<MerchantTurnoverDaily>()
        .fetch_all(pool)
        .await?;

    Ok(turnover)
}

/// Get turnover analytics for a merchant
pub async fn get_merchant_turnover_analytics(
    pool: &PgPool,
    merchant_id: uuid::Uuid,
) -> Result<Option<MerchantTurnoverAnalytics>, sqlx::Error> {
    let analytics = sqlx::query_as!(
        MerchantTurnoverAnalytics,
        r#"
        SELECT
            merchant_id as "merchant_id!: uuid::Uuid",
            legal_name as "legal_name!: String",
            total_days_recorded as "total_days_recorded!: i64",
            verified_days as "verified_days!: i64",
            total_net_receipts,
            avg_daily_receipts,
            last_turnover_date,
            first_turnover_date,
            monthly_estimate,
            data_quality_score as "data_quality_score!: i32"
        FROM merchant_turnover_analytics
        WHERE merchant_id = $1
        "#,
        merchant_id
    )
    .fetch_optional(pool)
    .await?;

    Ok(analytics)
}

/// Get profile completion status for a merchant
pub async fn get_merchant_profile_completion(
    pool: &PgPool,
    merchant_id: uuid::Uuid,
) -> Result<Option<MerchantProfileCompletion>, sqlx::Error> {
    let completion = sqlx::query_as!(
        MerchantProfileCompletion,
        r#"
        SELECT
            merchant_id as "merchant_id!: uuid::Uuid",
            legal_name as "legal_name!: String",
            kyc_status as "kyc_status!: String",
            has_legal_name as "has_legal_name!: bool",
            has_tax_id as "has_tax_id!: bool",
            has_mcc as "has_mcc!: bool",
            has_industry as "has_industry!: bool",
            has_years_in_business as "has_years_in_business!: bool",
            has_revenue_estimate as "has_revenue_estimate!: bool",
            kyc_approved as "kyc_approved!: bool",
            has_sufficient_turnover_history as "has_sufficient_turnover_history!: bool",
            has_verified_turnover_data as "has_verified_turnover_data!: bool",
            has_bank_statement as "has_bank_statement!: bool",
            completion_score as "completion_score!: i32"
        FROM merchant_profile_completion
        WHERE merchant_id = $1
        "#,
        merchant_id
    )
    .fetch_optional(pool)
    .await?;

    Ok(completion)
}

/// Create a new document record
pub async fn create_merchant_document(
    pool: &PgPool,
    merchant_id: uuid::Uuid,
    request: &CreateDocumentRequest,
    created_by: Option<uuid::Uuid>,
) -> Result<MerchantDocument, sqlx::Error> {
    let document = sqlx::query_as!(
        MerchantDocument,
        r#"
        INSERT INTO merchant_documents (
            merchant_id, document_type, document_url, document_hash,
            file_size_bytes, original_filename, mime_type, created_by
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        RETURNING
            id, merchant_id,
            document_type as "document_type!: String",
            document_url, document_hash,
            file_size_bytes, original_filename, mime_type,
            uploaded_at as "uploaded_at!: DateTime<Utc>",
            verified_at as "verified_at?: DateTime<Utc>",
            status as "status!: String",
            verification_notes, created_by,
            updated_at as "updated_at!: DateTime<Utc>"
        "#,
        merchant_id,
        request.document_type,
        request.document_url,
        request.document_hash,
        request.file_size_bytes,
        request.original_filename,
        request.mime_type,
        created_by
    )
    .fetch_one(pool)
    .await?;

    Ok(document)
}

/// Get documents for a merchant
pub async fn get_merchant_documents(
    pool: &PgPool,
    merchant_id: uuid::Uuid,
    document_type: Option<String>,
    status: Option<String>,
) -> Result<Vec<MerchantDocument>, sqlx::Error> {
    let mut query = sqlx::QueryBuilder::new(
        r#"
        SELECT id, merchant_id, document_type, document_url, document_hash,
               file_size_bytes, original_filename, mime_type, uploaded_at,
               verified_at, status, verification_notes, created_by, updated_at
        FROM merchant_documents
        WHERE merchant_id =
        "#,
    );

    query.push_bind(merchant_id);

    if let Some(doc_type) = document_type {
        query.push(" AND document_type = ");
        query.push_bind(doc_type);
    }

    if let Some(doc_status) = status {
        query.push(" AND status = ");
        query.push_bind(doc_status);
    }

    query.push(" ORDER BY uploaded_at DESC");

    let documents = query
        .build_query_as::<MerchantDocument>()
        .fetch_all(pool)
        .await?;

    Ok(documents)
}

/// Update document verification status
pub async fn update_document_status(
    pool: &PgPool,
    document_id: uuid::Uuid,
    status: String,
    verification_notes: Option<String>,
) -> Result<Option<MerchantDocument>, sqlx::Error> {
    let document = if status == "VERIFIED" {
        sqlx::query_as!(
            MerchantDocument,
            r#"
            UPDATE merchant_documents
            SET
                status = $2,
                verification_notes = $3,
                verified_at = NOW(),
                updated_at = NOW()
            WHERE id = $1
            RETURNING
                id, merchant_id,
                document_type as "document_type!: String",
                document_url, document_hash,
                file_size_bytes, original_filename, mime_type,
                uploaded_at as "uploaded_at!: DateTime<Utc>",
                verified_at as "verified_at?: DateTime<Utc>",
                status as "status!: String",
                verification_notes, created_by,
                updated_at as "updated_at!: DateTime<Utc>"
            "#,
            document_id,
            status,
            verification_notes
        )
        .fetch_optional(pool)
        .await?
    } else {
        sqlx::query_as!(
            MerchantDocument,
            r#"
            UPDATE merchant_documents
            SET
                status = $2,
                verification_notes = $3,
                updated_at = NOW()
            WHERE id = $1
            RETURNING
                id, merchant_id,
                document_type as "document_type!: String",
                document_url, document_hash,
                file_size_bytes, original_filename, mime_type,
                uploaded_at as "uploaded_at!: DateTime<Utc>",
                verified_at as "verified_at?: DateTime<Utc>",
                status as "status!: String",
                verification_notes, created_by,
                updated_at as "updated_at!: DateTime<Utc>"
            "#,
            document_id,
            status,
            verification_notes
        )
        .fetch_optional(pool)
        .await?
    };

    Ok(document)
}

/// Get investor profile with identity information by user ID or email
pub async fn get_investor_profile_by_user_id_or_email(
    pool: &PgPool,
    user_id: Option<uuid::Uuid>,
    email: Option<&str>,
) -> Result<Option<(InvestorProfile, Identity)>, sqlx::Error> {
    // Ensure exactly one parameter is provided
    match (user_id, email) {
        (Some(_), Some(_)) => return Err(sqlx::Error::RowNotFound),
        (None, None) => return Err(sqlx::Error::RowNotFound),
        _ => {}
    }

    let result = sqlx::query!(
        r#"
        SELECT
            profile_id as "profile_id!", user_id as "user_id!", user_email as "user_email!",
            user_type as "user_type!", kyc_status as "kyc_status!",
            kyc_submitted_at, kyc_verified_at,
            kyc_investor_type as "kyc_investor_type?: KycInvestorType",
            accredited_investor, investment_capacity, regulatory_jurisdiction,
            profile_created_at as "profile_created_at!", profile_updated_at as "profile_updated_at!",
            first_name, last_name, jurisdiction, wallet_address
        FROM get_investor_profile_by_user_id_or_email($1, $2)
        "#,
        user_id,
        email
    )
    .fetch_optional(pool)
    .await?;

    match result {
        Some(row) => {
            let profile = InvestorProfile {
                id: row.profile_id,
                user_id: row.user_id,
                accredited_investor: row.accredited_investor.unwrap_or(false),
                investment_capacity: row.investment_capacity,
                regulatory_jurisdiction: row.regulatory_jurisdiction.unwrap_or_default(),
                created_at: row.profile_created_at,
                updated_at: row.profile_updated_at,
            };

            let identity = Identity {
                id: row.user_id,
                user_type: row.user_type,
                email: row.user_email,
                kyc_status: row.kyc_status,
                kyc_submitted_at: row.kyc_submitted_at,
                kyc_verified_at: row.kyc_verified_at,
                kyc_investor_type: row.kyc_investor_type,
                created_at: row.profile_created_at, // Using profile timestamps since this is joined data
                updated_at: row.profile_updated_at,
                last_login_at: None,
                email_verified: None,
                email_verification_token: None,
                first_name: row.first_name,
                last_name: row.last_name,
                jurisdiction: row.jurisdiction,
                wallet_address: row.wallet_address,
            };

            Ok(Some((profile, identity)))
        }
        None => Ok(None),
    }
}

// ============================================================================
// FINANCIAL TRANSACTION TYPES AND FUNCTIONS
// ============================================================================

/// Account financial transaction (deposits/withdrawals from smart contract)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct AccountFinancialTransaction {
    pub id: uuid::Uuid,
    pub user_id: uuid::Uuid,
    pub transaction_type: String, // DEPOSIT, WITHDRAWAL
    pub amount: rust_decimal::Decimal,
    pub currency: String,
    pub transaction_hash: String,    // NOT NULL in database
    pub blockchain_status: String,   // PENDING, CONFIRMED, FAILED
    pub block_number: Option<i64>,   // NULL in database
    pub processed_at: DateTime<Utc>, // NOT NULL (has default)
}

/// Pool financial transaction (funding/withdrawals from pools)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct PoolFinancialTransaction {
    pub id: uuid::Uuid,
    pub pool_id: uuid::Uuid,
    pub user_id: uuid::Uuid,
    pub transaction_type: String, // FUND, WITHDRAW, INTEREST, RETURN
    pub amount: rust_decimal::Decimal,
    pub currency: String,
    pub state_transaction_id: Option<uuid::Uuid>, // Link to state transaction if applicable
    pub transaction_hash: Option<String>,         // NULL in database
    pub description: Option<String>,              // NULL in database
    pub processed_at: DateTime<Utc>,              // NOT NULL (has default)
}

/// Credit line financial transaction (draws/payments from credit lines)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct CreditLineFinancialTransaction {
    pub id: uuid::Uuid,
    pub credit_line_id: uuid::Uuid,
    pub merchant_id: uuid::Uuid,
    pub transaction_type: String, // DRAW, PAYMENT, INTEREST, FEE, PENALTY
    pub amount: rust_decimal::Decimal,
    pub currency: String,
    pub state_transaction_id: Option<uuid::Uuid>, // Link to state transaction if applicable
    pub transaction_hash: Option<String>,         // NULL in database
    pub description: Option<String>,              // NULL in database
    pub processed_at: DateTime<Utc>,              // NOT NULL (has default)
}

/// Unified financial transaction view
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct UserFinancialTransaction {
    pub transaction_category: String, // ACCOUNT, POOL, CREDIT_LINE
    pub id: uuid::Uuid,
    pub user_id: uuid::Uuid,
    pub transaction_type: String,
    pub amount: rust_decimal::Decimal,
    pub currency: String,
    pub transaction_hash: Option<String>, // Can be NULL from view
    pub processed_at: DateTime<Utc>,      // NOT NULL from view
    pub pool_id: Option<uuid::Uuid>,      // Can be NULL from view
    pub credit_line_id: Option<uuid::Uuid>, // Can be NULL from view
    pub description: String,              // NOT NULL from view (has COALESCE)
}

/// Financial summary for a user
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserFinancialSummary {
    pub total_deposits: rust_decimal::Decimal,
    pub total_withdrawals: rust_decimal::Decimal,
    pub net_account_flow: rust_decimal::Decimal,
    pub total_pool_funding: rust_decimal::Decimal,
    pub total_pool_withdrawals: rust_decimal::Decimal,
    pub total_credit_line_payments: rust_decimal::Decimal,
    pub total_credit_line_draws: rust_decimal::Decimal,
}

/// Request to create account financial transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAccountFinancialTransactionRequest {
    pub user_id: uuid::Uuid,
    pub transaction_type: String,
    pub amount: rust_decimal::Decimal,
    pub currency: String,
    pub transaction_hash: String,
    pub blockchain_status: String,
    pub block_number: Option<i64>,
}

/// Request to create pool financial transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePoolFinancialTransactionRequest {
    pub pool_id: uuid::Uuid,
    pub user_id: uuid::Uuid,
    pub transaction_type: String,
    pub amount: rust_decimal::Decimal,
    pub currency: String,
    pub state_transaction_id: Option<uuid::Uuid>,
    pub transaction_hash: Option<String>,
    pub description: Option<String>,
}

/// Request to create credit line financial transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCreditLineFinancialTransactionRequest {
    pub credit_line_id: uuid::Uuid,
    pub merchant_id: uuid::Uuid,
    pub transaction_type: String,
    pub amount: rust_decimal::Decimal,
    pub currency: String,
    pub state_transaction_id: Option<uuid::Uuid>,
    pub transaction_hash: Option<String>,
    pub description: Option<String>,
}

// ============================================================================
// FINANCIAL TRANSACTION FUNCTIONS
// ============================================================================

/// Create account financial transaction
pub async fn create_account_financial_transaction(
    pool: &PgPool,
    request: &CreateAccountFinancialTransactionRequest,
) -> Result<AccountFinancialTransaction, sqlx::Error> {
    // Insert the transaction
    let row = sqlx::query!(
        r#"
        INSERT INTO account_financial_transactions (
            user_id, transaction_type, amount, currency,
            transaction_hash, blockchain_status, block_number, processed_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, NOW())
        RETURNING id
        "#,
        request.user_id,
        request.transaction_type,
        request.amount,
        request.currency,
        request.transaction_hash,
        request.blockchain_status,
        request.block_number
    )
    .fetch_one(pool)
    .await?;

    // Fetch the created transaction manually
    let row = sqlx::query!(
        r#"
        SELECT id, user_id, transaction_type, amount, currency,
               transaction_hash, blockchain_status, block_number, processed_at
        FROM account_financial_transactions
        WHERE id = $1
        "#,
        row.id
    )
    .fetch_one(pool)
    .await?;

    let transaction = AccountFinancialTransaction {
        id: row.id,
        user_id: row.user_id,
        transaction_type: row.transaction_type,
        amount: row.amount,
        currency: row.currency.unwrap_or_default(),
        transaction_hash: row.transaction_hash,
        blockchain_status: row.blockchain_status.unwrap_or_default(),
        block_number: row.block_number,
        processed_at: row.processed_at.unwrap_or_else(|| chrono::Utc::now()),
    };

    Ok(transaction)
}

/// Create pool financial transaction
pub async fn create_pool_financial_transaction(
    pool: &PgPool,
    request: &CreatePoolFinancialTransactionRequest,
) -> Result<PoolFinancialTransaction, sqlx::Error> {
    // Insert the transaction
    let row = sqlx::query!(
        r#"
        INSERT INTO pool_financial_transactions (
            pool_id, user_id, transaction_type, amount, currency,
            state_transaction_id, transaction_hash, description, processed_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NOW())
        RETURNING id
        "#,
        request.pool_id,
        request.user_id,
        request.transaction_type,
        request.amount,
        request.currency,
        request.state_transaction_id,
        request.transaction_hash,
        request.description
    )
    .fetch_one(pool)
    .await?;

    // Fetch the created transaction manually
    let row = sqlx::query!(
        r#"
        SELECT id, pool_id, user_id, transaction_type, amount, currency,
               state_transaction_id, transaction_hash, description, processed_at
        FROM pool_financial_transactions
        WHERE id = $1
        "#,
        row.id
    )
    .fetch_one(pool)
    .await?;

    let transaction = PoolFinancialTransaction {
        id: row.id,
        pool_id: row.pool_id,
        user_id: row.user_id,
        transaction_type: row.transaction_type,
        amount: row.amount,
        currency: row.currency.unwrap_or_default(),
        state_transaction_id: row.state_transaction_id,
        transaction_hash: row.transaction_hash,
        description: row.description,
        processed_at: row.processed_at.unwrap_or_else(|| chrono::Utc::now()),
    };

    Ok(transaction)
}

/// Update pool balances after financial transaction
pub async fn update_pool_balance_after_transaction(
    pool: &PgPool,
    pool_id: uuid::Uuid,
    transaction_type: &str,
    amount: rust_decimal::Decimal,
) -> Result<(), sqlx::Error> {
    match transaction_type {
        "FUND" => {
            // Increase total_deposits and available_liquidity
            sqlx::query!(
                r#"
                UPDATE liquidity_pools
                SET total_deposits = total_deposits + $1,
                    available_liquidity = available_liquidity + $1,
                    updated_at = NOW()
                WHERE id = $2
                "#,
                amount,
                pool_id
            )
            .execute(pool)
            .await?;
        }
        "WITHDRAW" => {
            // Decrease total_deposits and available_liquidity
            sqlx::query!(
                r#"
                UPDATE liquidity_pools
                SET total_deposits = GREATEST(total_deposits - $1, 0),
                    available_liquidity = GREATEST(available_liquidity - $1, 0),
                    updated_at = NOW()
                WHERE id = $2
                "#,
                amount,
                pool_id
            )
            .execute(pool)
            .await?;
        }
        _ => {
            // For other transaction types (INTEREST, RETURN), don't modify balances
            return Ok(());
        }
    }

    Ok(())
}

/// Create credit line financial transaction
pub async fn create_credit_line_financial_transaction(
    pool: &PgPool,
    request: &CreateCreditLineFinancialTransactionRequest,
) -> Result<CreditLineFinancialTransaction, sqlx::Error> {
    // Insert the transaction
    let row = sqlx::query!(
        r#"
        INSERT INTO credit_line_financial_transactions (
            credit_line_id, merchant_id, transaction_type, amount, currency,
            state_transaction_id, transaction_hash, description, processed_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NOW())
        RETURNING id
        "#,
        request.credit_line_id,
        request.merchant_id,
        request.transaction_type,
        request.amount,
        request.currency,
        request.state_transaction_id,
        request.transaction_hash,
        request.description
    )
    .fetch_one(pool)
    .await?;

    // Fetch the created transaction manually
    let row = sqlx::query!(
        r#"
        SELECT id, credit_line_id, merchant_id, transaction_type, amount, currency,
               state_transaction_id, transaction_hash, description, processed_at
        FROM credit_line_financial_transactions
        WHERE id = $1
        "#,
        row.id
    )
    .fetch_one(pool)
    .await?;

    let transaction = CreditLineFinancialTransaction {
        id: row.id,
        credit_line_id: row.credit_line_id,
        merchant_id: row.merchant_id,
        transaction_type: row.transaction_type,
        amount: row.amount,
        currency: row.currency.unwrap_or_default(),
        state_transaction_id: row.state_transaction_id,
        transaction_hash: row.transaction_hash,
        description: row.description,
        processed_at: row.processed_at.unwrap_or_else(|| chrono::Utc::now()),
    };

    Ok(transaction)
}

/// Get account financial transactions for user
pub async fn get_user_account_financial_transactions(
    pool: &PgPool,
    user_id: uuid::Uuid,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<AccountFinancialTransaction>, sqlx::Error> {
    let limit_val = limit.unwrap_or(50);
    let offset_val = offset.unwrap_or(0);

    let rows = sqlx::query!(
        r#"
        SELECT id, user_id, transaction_type, amount, currency,
               transaction_hash, blockchain_status, block_number, processed_at
        FROM account_financial_transactions
        WHERE user_id = $1
        ORDER BY processed_at DESC
        LIMIT $2 OFFSET $3
        "#,
        user_id,
        limit_val,
        offset_val
    )
    .fetch_all(pool)
    .await?;

    let transactions = rows
        .into_iter()
        .map(|row| AccountFinancialTransaction {
            id: row.id,
            user_id: row.user_id,
            transaction_type: row.transaction_type,
            amount: row.amount,
            currency: row.currency.unwrap_or_default(),
            transaction_hash: row.transaction_hash,
            blockchain_status: row.blockchain_status.unwrap_or_default(),
            block_number: row.block_number,
            processed_at: row.processed_at.unwrap_or_else(|| chrono::Utc::now()),
        })
        .collect();

    Ok(transactions)
}

/// Get pool financial transactions for user
pub async fn get_user_pool_financial_transactions(
    pool: &PgPool,
    user_id: uuid::Uuid,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<PoolFinancialTransaction>, sqlx::Error> {
    let limit_val = limit.unwrap_or(50);
    let offset_val = offset.unwrap_or(0);

    let rows = sqlx::query!(
        r#"
        SELECT id, pool_id, user_id, transaction_type, amount, currency,
               state_transaction_id, transaction_hash, description, processed_at
        FROM pool_financial_transactions
        WHERE user_id = $1
        ORDER BY processed_at DESC
        LIMIT $2 OFFSET $3
        "#,
        user_id,
        limit_val,
        offset_val
    )
    .fetch_all(pool)
    .await?;

    let transactions = rows
        .into_iter()
        .map(|row| PoolFinancialTransaction {
            id: row.id,
            pool_id: row.pool_id,
            user_id: row.user_id,
            transaction_type: row.transaction_type,
            amount: row.amount,
            currency: row.currency.unwrap_or_default(),
            state_transaction_id: row.state_transaction_id,
            transaction_hash: row.transaction_hash,
            description: row.description,
            processed_at: row.processed_at.unwrap_or_else(|| chrono::Utc::now()),
        })
        .collect();

    Ok(transactions)
}

/// Get credit line financial transactions for merchant
pub async fn get_merchant_credit_line_financial_transactions(
    pool: &PgPool,
    merchant_id: uuid::Uuid,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<CreditLineFinancialTransaction>, sqlx::Error> {
    let limit_val = limit.unwrap_or(50);
    let offset_val = offset.unwrap_or(0);

    let rows = sqlx::query!(
        r#"
        SELECT id, credit_line_id, merchant_id, transaction_type, amount, currency,
               state_transaction_id, transaction_hash, description, processed_at
        FROM credit_line_financial_transactions
        WHERE merchant_id = $1
        ORDER BY processed_at DESC
        LIMIT $2 OFFSET $3
        "#,
        merchant_id,
        limit_val,
        offset_val
    )
    .fetch_all(pool)
    .await?;

    let transactions = rows
        .into_iter()
        .map(|row| CreditLineFinancialTransaction {
            id: row.id,
            credit_line_id: row.credit_line_id,
            merchant_id: row.merchant_id,
            transaction_type: row.transaction_type,
            amount: row.amount,
            currency: row.currency.unwrap_or_default(),
            state_transaction_id: row.state_transaction_id,
            transaction_hash: row.transaction_hash,
            description: row.description,
            processed_at: row.processed_at.unwrap_or_else(|| chrono::Utc::now()),
        })
        .collect();

    Ok(transactions)
}

/// Get unified financial transactions for user
pub async fn get_user_financial_transactions(
    pool: &PgPool,
    user_id: uuid::Uuid,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<UserFinancialTransaction>, sqlx::Error> {
    let limit_val = limit.unwrap_or(50);
    let offset_val = offset.unwrap_or(0);

    let rows = sqlx::query!(
        r#"
        SELECT * FROM user_financial_transactions
        WHERE user_id = $1
        ORDER BY processed_at DESC
        LIMIT $2 OFFSET $3
        "#,
        user_id,
        limit_val,
        offset_val
    )
    .fetch_all(pool)
    .await?;

    let transactions = rows
        .into_iter()
        .map(|row| UserFinancialTransaction {
            transaction_category: row
                .transaction_category
                .expect("transaction_category should not be null"),
            id: row.id.expect("id should not be null"),
            user_id: row.user_id.expect("user_id should not be null"),
            transaction_type: row
                .transaction_type
                .expect("transaction_type should not be null"),
            amount: row.amount.expect("amount should not be null"),
            currency: row.currency.expect("currency should not be null"),
            transaction_hash: row.transaction_hash,
            processed_at: row.processed_at.unwrap_or_else(|| chrono::Utc::now()),
            pool_id: row.pool_id,
            credit_line_id: row.credit_line_id,
            description: row.description.unwrap_or_default(),
        })
        .collect();

    Ok(transactions)
}

/// Get pool financial transactions
pub async fn get_pool_financial_transactions(
    pool: &PgPool,
    pool_id: uuid::Uuid,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<PoolFinancialTransaction>, sqlx::Error> {
    let limit_val = limit.unwrap_or(50);
    let offset_val = offset.unwrap_or(0);

    let rows = sqlx::query!(
        r#"
        SELECT id, pool_id, user_id, transaction_type, amount, currency,
               state_transaction_id, transaction_hash, description, processed_at
        FROM pool_financial_transactions
        WHERE pool_id = $1
        ORDER BY processed_at DESC
        LIMIT $2 OFFSET $3
        "#,
        pool_id,
        limit_val,
        offset_val
    )
    .fetch_all(pool)
    .await?;

    let transactions = rows
        .into_iter()
        .map(|row| PoolFinancialTransaction {
            id: row.id,
            pool_id: row.pool_id,
            user_id: row.user_id,
            transaction_type: row.transaction_type,
            amount: row.amount,
            currency: row.currency.unwrap_or_default(),
            state_transaction_id: row.state_transaction_id,
            transaction_hash: row.transaction_hash,
            description: row.description,
            processed_at: row.processed_at.unwrap_or_else(|| chrono::Utc::now()),
        })
        .collect();

    Ok(transactions)
}

/// Get credit line financial transactions
pub async fn get_credit_line_financial_transactions(
    pool: &PgPool,
    credit_line_id: uuid::Uuid,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<CreditLineFinancialTransaction>, sqlx::Error> {
    let limit_val = limit.unwrap_or(50);
    let offset_val = offset.unwrap_or(0);

    let rows = sqlx::query!(
        r#"
        SELECT id, credit_line_id, merchant_id, transaction_type, amount, currency,
               state_transaction_id, transaction_hash, description, processed_at
        FROM credit_line_financial_transactions
        WHERE credit_line_id = $1
        ORDER BY processed_at DESC
        LIMIT $2 OFFSET $3
        "#,
        credit_line_id,
        limit_val,
        offset_val
    )
    .fetch_all(pool)
    .await?;

    let transactions = rows
        .into_iter()
        .map(|row| CreditLineFinancialTransaction {
            id: row.id,
            credit_line_id: row.credit_line_id,
            merchant_id: row.merchant_id,
            transaction_type: row.transaction_type,
            amount: row.amount,
            currency: row.currency.unwrap_or_default(),
            state_transaction_id: row.state_transaction_id,
            transaction_hash: row.transaction_hash,
            description: row.description,
            processed_at: row.processed_at.unwrap_or_else(|| chrono::Utc::now()),
        })
        .collect();

    Ok(transactions)
}

/// Get user financial summary
pub async fn get_user_financial_summary(
    pool: &PgPool,
    user_id: uuid::Uuid,
) -> Result<UserFinancialSummary, sqlx::Error> {
    let row = sqlx::query!(
        r#"
        SELECT * FROM get_user_financial_summary($1)
        "#,
        user_id
    )
    .fetch_one(pool)
    .await?;

    Ok(UserFinancialSummary {
        total_deposits: row.total_deposits.unwrap_or(rust_decimal::Decimal::ZERO),
        total_withdrawals: row.total_withdrawals.unwrap_or(rust_decimal::Decimal::ZERO),
        net_account_flow: row.net_account_flow.unwrap_or(rust_decimal::Decimal::ZERO),
        total_pool_funding: row
            .total_pool_funding
            .unwrap_or(rust_decimal::Decimal::ZERO),
        total_pool_withdrawals: row
            .total_pool_withdrawals
            .unwrap_or(rust_decimal::Decimal::ZERO),
        total_credit_line_payments: row
            .total_credit_line_payments
            .unwrap_or(rust_decimal::Decimal::ZERO),
        total_credit_line_draws: row
            .total_credit_line_draws
            .unwrap_or(rust_decimal::Decimal::ZERO),
    })
}

// ============================================================================
// POOL BOROUGH LIMIT FUNCTIONS
// ============================================================================

/// Set or update a borough percentage limit for a pool
pub async fn set_pool_borough_limit(
    pool: &PgPool,
    request: SetPoolBoroughLimitRequest,
) -> Result<PoolBoroughLimit, sqlx::Error> {
    let result = sqlx::query!(
        r#"
        INSERT INTO pool_borough_limits (pool_id, borough, max_percentage, current_allocated)
        VALUES ($1, $2, $3, 0)
        ON CONFLICT (pool_id, borough)
        DO UPDATE SET
            max_percentage = $3,
            updated_at = NOW()
        RETURNING id, pool_id, borough, max_percentage, current_allocated, created_at, updated_at
        "#,
        request.pool_id,
        request.borough,
        request.max_percentage
    )
    .fetch_one(pool)
    .await?;

    let limit = PoolBoroughLimit {
        id: result.id,
        pool_id: result.pool_id,
        borough: result.borough,
        max_percentage: result.max_percentage,
        current_allocated: result.current_allocated,
        created_at: result.created_at.unwrap_or_else(|| Utc::now()),
        updated_at: result.updated_at.unwrap_or_else(|| Utc::now()),
    };

    Ok(limit)
}

/// Get all borough limits for a specific pool
pub async fn get_pool_borough_limits(
    pool: &PgPool,
    pool_id: uuid::Uuid,
) -> Result<Vec<PoolBoroughLimit>, sqlx::Error> {
    let results = sqlx::query!(
        r#"
        SELECT id, pool_id, borough, max_percentage, current_allocated, created_at, updated_at
        FROM pool_borough_limits
        WHERE pool_id = $1
        ORDER BY borough ASC
        "#,
        pool_id
    )
    .fetch_all(pool)
    .await?;

    let limits = results
        .into_iter()
        .map(|row| PoolBoroughLimit {
            id: row.id,
            pool_id: row.pool_id,
            borough: row.borough,
            max_percentage: row.max_percentage,
            current_allocated: row.current_allocated,
            created_at: row.created_at.unwrap_or_else(|| Utc::now()),
            updated_at: row.updated_at.unwrap_or_else(|| Utc::now()),
        })
        .collect();

    Ok(limits)
}

/// Validate if a loan request would exceed borough percentage limits
pub async fn validate_borough_borrowing_limit(
    pool: &PgPool,
    pool_id: uuid::Uuid,
    borough: &str,
    requested_amount: rust_decimal::Decimal,
) -> Result<BoroughValidationResult, sqlx::Error> {
    let result = sqlx::query!(
        r#"
        SELECT * FROM validate_borough_borrowing_limit($1, $2, $3)
        "#,
        pool_id,
        borough,
        requested_amount
    )
    .fetch_one(pool)
    .await?;

    Ok(BoroughValidationResult {
        is_valid: result.is_valid.unwrap_or(false),
        current_allocated: result
            .current_allocated
            .unwrap_or(rust_decimal::Decimal::ZERO),
        max_allowed: result.max_allowed.unwrap_or(rust_decimal::Decimal::ZERO),
        total_pool_deposits: result
            .total_pool_deposits
            .unwrap_or(rust_decimal::Decimal::ZERO),
        percentage_after_loan: result
            .percentage_after_loan
            .unwrap_or(rust_decimal::Decimal::ZERO),
        max_percentage: result.max_percentage.unwrap_or(rust_decimal::Decimal::ONE),
    })
}

/// Update borough allocation when loans are originated or repaid
pub async fn update_borough_allocation(
    pool: &PgPool,
    pool_id: uuid::Uuid,
    borough: &str,
    amount_change: rust_decimal::Decimal,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        SELECT update_borough_allocation($1, $2, $3)
        "#,
        pool_id,
        borough,
        amount_change
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Get borough allocation summary for a pool
pub async fn get_pool_borough_summary(
    pool: &PgPool,
    pool_id: uuid::Uuid,
) -> Result<Vec<PoolBoroughSummary>, sqlx::Error> {
    let results = sqlx::query!(
        r#"
        SELECT * FROM get_pool_borough_summary($1)
        "#,
        pool_id
    )
    .fetch_all(pool)
    .await?;

    let summaries = results
        .into_iter()
        .map(|row| PoolBoroughSummary {
            borough: row.borough.unwrap_or_default(),
            current_allocated: row.current_allocated.unwrap_or(rust_decimal::Decimal::ZERO),
            max_percentage: row.max_percentage.unwrap_or(rust_decimal::Decimal::ZERO),
            max_allowed: row.max_allowed.unwrap_or(rust_decimal::Decimal::ZERO),
            remaining_capacity: row
                .remaining_capacity
                .unwrap_or(rust_decimal::Decimal::ZERO),
            utilization_percentage: row
                .utilization_percentage
                .unwrap_or(rust_decimal::Decimal::ZERO),
        })
        .collect();

    Ok(summaries)
}

/// Delete a borough limit for a pool
pub async fn delete_pool_borough_limit(
    pool: &PgPool,
    pool_id: uuid::Uuid,
    borough: &str,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query!(
        r#"
        DELETE FROM pool_borough_limits
        WHERE pool_id = $1 AND borough = $2
        "#,
        pool_id,
        borough
    )
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}
