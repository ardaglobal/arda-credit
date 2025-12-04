/// Loan Tape DTOs for deal records with comprehensive financial data
///
/// This module provides DTOs for loan tape operations, which combine
/// deal data with payment statistics, company information, and computed metrics.
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

/// Request to create a single loan tape record
#[derive(Debug, Clone, Serialize, Deserialize, Validate, utoipa::ToSchema)]
pub struct CreateLoanTapeRequest {
    /// Originating company name
    #[schema(example = "TechFlow Solutions Inc")]
    pub originator_name: String,

    /// Investing company name
    #[schema(example = "Apex Credit Partners LP")]
    pub investor_name: String,

    /// Principal amount of the deal
    #[schema(value_type = String, example = "2500000.00")]
    pub principal_amount: Decimal,

    /// Current outstanding balance
    #[schema(value_type = String, example = "2150000.00")]
    pub outstanding_balance: Decimal,

    /// Interest rate in basis points (e.g., 875 = 8.75%)
    #[schema(example = 875)]
    pub interest_rate_bps: i32,

    /// Deal status: originated, active, paid_off, closed, defaulted
    #[schema(example = "active")]
    pub deal_status: String,

    /// Origination date and time
    #[schema(example = "2024-01-15T09:30:00Z")]
    pub originated_at: DateTime<Utc>,

    /// Maturity date (optional)
    #[schema(example = "2027-01-15T00:00:00Z")]
    pub maturity_date: Option<DateTime<Utc>>,

    /// Custom deal identifier (optional)
    #[schema(example = "DL-20240115-001")]
    pub custom_deal_id: Option<String>,

    /// Deal title
    #[schema(example = "TechFlow Working Capital Facility")]
    pub title: Option<String>,

    /// Deal description
    #[schema(example = "Working capital facility for software company")]
    pub description: Option<String>,

    /// Payment frequency: daily, weekly, biweekly, monthly, quarterly, etc.
    #[schema(example = "monthly")]
    pub payment_frequency: Option<String>,

    /// Collateral asset UUID (for linking to collateral table)
    #[schema(example = "450e8400-e29b-41d4-a716-446655440003")]
    pub collateral_id: Option<String>,

    /// Collateral asset name/identifier
    #[schema(example = "Warehouse Property - 123 Main St")]
    pub collateral_name: Option<String>,

    /// Collateral type (free text)
    #[schema(example = "Equipment")]
    pub collateral_type: Option<String>,

    /// Collateral physical or legal address
    #[schema(example = "123 Main St, New York, NY 10001")]
    pub collateral_address: Option<String>,

    /// Custom collateral identifier (user-defined)
    #[schema(example = "COLL-202412-1234")]
    pub collateral_custom_id: Option<String>,

    /// Collateral value in USD
    #[schema(value_type = Option<String>, example = "3500000.00")]
    pub collateral_value: Option<Decimal>,

    /// Loan-to-Value ratio (0-1 range, e.g., 0.7143 = 71.43%)
    #[schema(value_type = Option<String>, example = "0.7143")]
    pub ltv_ratio: Option<Decimal>,

    /// Risk level: low, medium, high
    #[schema(example = "medium")]
    pub risk_level: Option<String>,

    /// Asset class
    #[schema(example = "direct_lending")]
    pub asset_class: Option<String>,

    /// Geographic region
    #[schema(example = "north_america")]
    pub region: Option<String>,

    /// Industry sector
    #[schema(example = "software_saas")]
    pub industry: Option<String>,

    /// Numeric credit score (300-850)
    #[schema(example = 720)]
    pub credit_score: Option<i32>,

    /// Debt Service Coverage Ratio
    #[schema(value_type = Option<String>, example = "1.45")]
    pub dscr: Option<Decimal>,

    /// Blockchain transaction hash for state transitions
    #[schema(example = "0x7f8e9a2b1c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f")]
    pub state_transition_hash: Option<String>,

    /// Whether this deal is included in a ZK proof batch
    #[schema(example = true)]
    pub proof_included: Option<bool>,
}

/// Request to create multiple loan tape records in bulk
#[derive(Debug, Clone, Serialize, Deserialize, Validate, utoipa::ToSchema)]
pub struct BulkCreateLoanTapeRequest {
    /// User ID of the uploader (must be associated with the consistent company)
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub user_id: Uuid,

    /// Optional document ID from the ingestion service that created these deals
    #[schema(example = "650e8400-e29b-41d4-a716-446655440000")]
    pub document_id: Uuid,

    /// Array of loan tape records to create
    #[validate(length(min = 1, max = 100))]
    pub records: Vec<CreateLoanTapeRequest>,
}

/// Complete loan tape record response with all computed and joined data
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct LoanTapeRecord {
    /// Deal UUID (system identifier)
    pub loan_id: Uuid,

    /// Custom deal identifier (user-defined)
    #[schema(example = "DL-20240115-001")]
    pub deal_id: Option<String>,

    /// Originating company ID
    pub originator_id: Uuid,

    /// Originating company name (from companies table)
    #[schema(example = "TechFlow Solutions Inc")]
    pub originator_name: String,

    /// Originator KYB status (from companies table)
    #[schema(example = "approved")]
    pub originator_kyb_status: String,

    /// Investing company ID
    pub investor_id: Option<Uuid>,

    /// Investing company name (from companies table)
    #[schema(example = "Apex Credit Partners LP")]
    pub investor_name: Option<String>,

    /// Investor KYB status (from companies table)
    #[schema(example = "approved")]
    pub investor_kyb_status: Option<String>,

    /// Principal amount
    #[schema(value_type = String, example = "2500000.00")]
    pub principal_amount: Decimal,

    /// Current outstanding balance
    #[schema(value_type = String, example = "2150000.00")]
    pub outstanding_balance: Decimal,

    /// Interest rate in basis points
    #[schema(example = 875)]
    pub interest_rate_bps: i32,

    /// Interest rate as APR percentage (computed)
    #[schema(value_type = String, example = "8.75")]
    pub interest_rate_apr: Decimal,

    /// Deal status
    #[schema(example = "active")]
    pub loan_status: String,

    /// Origination timestamp
    pub originated_at: DateTime<Utc>,

    /// Maturity date
    pub maturity_date: Option<DateTime<Utc>>,

    /// Days until maturity (computed)
    #[schema(example = 428)]
    pub days_to_maturity: Option<i32>,

    /// Payment frequency
    #[schema(example = "monthly")]
    pub payment_frequency: Option<String>,

    /// Last payment date (from deal_payments table)
    pub last_payment_date: Option<DateTime<Utc>>,

    /// Last payment amount (from deal_payments table)
    #[schema(value_type = Option<String>, example = "45833.33")]
    pub last_payment_amount: Option<Decimal>,

    /// Total number of payments made (from deal_payments table)
    #[schema(example = 10)]
    pub total_payments_made: i64,

    /// Total interest paid (from deal_payments table)
    #[schema(value_type = String, example = "287500.00")]
    pub total_interest_paid: Decimal,

    /// Collateral type
    #[schema(example = "Equipment")]
    pub collateral_type: Option<String>,

    /// Collateral value
    #[schema(value_type = Option<String>, example = "3500000.00")]
    pub collateral_value: Option<Decimal>,

    /// Loan-to-Value ratio
    #[schema(value_type = Option<String>, example = "0.7143")]
    pub ltv_ratio: Option<Decimal>,

    /// Risk level
    #[schema(example = "medium")]
    pub risk_level: Option<String>,

    /// Asset class
    #[schema(example = "direct_lending")]
    pub asset_class: Option<String>,

    /// Geographic region
    #[schema(example = "north_america")]
    pub region: Option<String>,

    /// Borrower industry (from originator company)
    #[schema(example = "software_saas")]
    pub borrower_industry: Option<String>,

    /// Credit score
    #[schema(example = 720)]
    pub credit_score: Option<i32>,

    /// Debt Service Coverage Ratio
    #[schema(value_type = Option<String>, example = "1.45")]
    pub dscr: Option<Decimal>,

    /// Deal title
    #[schema(example = "TechFlow Working Capital Facility")]
    pub deal_title: Option<String>,

    /// State transition hash (as hex string)
    #[schema(example = "0x7f8e9a2b1c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f")]
    pub state_transition_hash: Option<String>,

    /// Whether included in ZK proof batch
    #[schema(example = true)]
    pub proof_included: bool,

    /// Document IDs that were used to create this deal (JSONB array of UUIDs)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_ids: Option<serde_json::Value>,

    /// Record creation timestamp
    pub created_at: DateTime<Utc>,

    /// Record last update timestamp
    pub updated_at: DateTime<Utc>,
}
