//! Credit/loan operations for investors, merchants, and credit lines
//!
//! This module handles all loan and credit-related operations for the Arda Credit CLI including:
//! - Investor registration and profile management
//! - Merchant onboarding and business information management
//! - Turnover data submission and analytics
//! - Credit line eligibility checking and request management
//! - Pool browsing and credit matching operations
//! - Loan origination, payment, and closeout operations
//!
//! All loan operations maintain privacy through ZK proofs while enabling regulatory
//! compliance through proper KYC verification and audit trails.

pub mod credit;
pub mod investors;
pub mod merchants;
pub mod operations;

pub use credit::*;
pub use investors::*;
pub use merchants::*;
pub use operations::*;

use arda_credit_api::BatchApiClient;
use clap::Subcommand;
use eyre::Result;

/// Loan and credit CLI commands
#[derive(Subcommand)]
pub enum LoanCommands {
    /// Register a new investor
    RegisterInvestor {
        /// Investor email address
        #[arg(long)]
        email: String,
        /// First name (optional)
        #[arg(long)]
        first_name: Option<String>,
        /// Last name (optional)
        #[arg(long)]
        last_name: Option<String>,
        /// Jurisdiction (optional, defaults to US)
        #[arg(long)]
        jurisdiction: Option<String>,
        /// Whether the investor is accredited (optional, defaults to false)
        #[arg(long)]
        accredited_investor: Option<bool>,
        /// Maximum investment capacity in USDC (optional)
        #[arg(long)]
        investment_capacity: Option<String>,
        /// Regulatory jurisdiction (optional, defaults to US)
        #[arg(long)]
        regulatory_jurisdiction: Option<String>,
        /// Invite code (optional)
        #[arg(long)]
        invite_code: Option<String>,
        /// Show detailed output
        #[arg(short, long)]
        verbose: bool,
    },

    /// Get investor profile details
    GetInvestorProfile {
        /// User ID of the investor
        #[arg(long)]
        user_id: String,
        /// Show detailed output
        #[arg(short, long)]
        verbose: bool,
    },

    /// Update investor profile settings
    UpdateInvestorProfile {
        /// User ID of the investor
        #[arg(long)]
        user_id: String,
        /// Whether the investor is accredited
        #[arg(long)]
        accredited_investor: Option<bool>,
        /// Maximum investment capacity in USDC
        #[arg(long)]
        investment_capacity: Option<String>,
        /// Regulatory jurisdiction
        #[arg(long)]
        regulatory_jurisdiction: Option<String>,
        /// Show detailed output
        #[arg(short, long)]
        verbose: bool,
    },

    /// Register a new merchant with business information
    RegisterMerchant {
        /// Merchant email address
        #[arg(long)]
        email: String,
        /// Merchant password
        #[arg(long)]
        password: String,
        /// Legal business name
        #[arg(long)]
        legal_name: String,
        /// Doing Business As name (optional)
        #[arg(long)]
        dba: Option<String>,
        /// Tax ID (optional)
        #[arg(long)]
        tax_id: Option<String>,
        /// Merchant Category Code (optional)
        #[arg(long)]
        mcc: Option<String>,
        /// Country (defaults to USA)
        #[arg(long, default_value = "USA")]
        country: String,
        /// Business type (LLC, CORP, SOLE_PROP, PARTNERSHIP, OTHER)
        #[arg(long)]
        business_type: Option<String>,
        /// Jurisdiction (optional, defaults to US)
        #[arg(long)]
        jurisdiction: Option<String>,
        /// Invite code (optional)
        #[arg(long)]
        invite_code: Option<String>,
        /// Show detailed output
        #[arg(short, long)]
        verbose: bool,
    },

    /// Submit daily turnover data for a merchant
    SubmitTurnover {
        /// Date for the turnover record (YYYY-MM-DD format)
        #[arg(long)]
        date: String,
        /// Gross receipts amount
        #[arg(long)]
        gross_receipts: String,
        /// Refunds amount (optional)
        #[arg(long)]
        refunds: Option<String>,
        /// Data source (MANUAL, API, BANK_INTEGRATION)
        #[arg(long, default_value = "MANUAL")]
        source: String,
        /// Verification method (optional)
        #[arg(long)]
        verification_method: Option<String>,
        /// Show detailed output
        #[arg(short, long)]
        verbose: bool,
    },

    /// Get merchant turnover data
    GetTurnover {
        /// Start date filter (YYYY-MM-DD format, optional)
        #[arg(long)]
        start_date: Option<String>,
        /// End date filter (YYYY-MM-DD format, optional)
        #[arg(long)]
        end_date: Option<String>,
        /// Limit number of records returned
        #[arg(long, default_value = "30")]
        limit: Option<i32>,
        /// Show detailed output
        #[arg(short, long)]
        verbose: bool,
    },

    /// Update existing turnover record
    UpdateTurnover {
        /// Date of the turnover record to update (YYYY-MM-DD format)
        #[arg(long)]
        date: String,
        /// Updated gross receipts amount (optional)
        #[arg(long)]
        gross_receipts: Option<String>,
        /// Updated refunds amount (optional)
        #[arg(long)]
        refunds: Option<String>,
        /// Updated verification method (optional)
        #[arg(long)]
        verification_method: Option<String>,
        /// Show detailed output
        #[arg(short, long)]
        verbose: bool,
    },

    /// Get merchant turnover analytics
    GetTurnoverAnalytics {
        /// Time period for analytics (30d, 90d, 1y)
        #[arg(long, default_value = "90d")]
        period: String,
        /// Show detailed output
        #[arg(short, long)]
        verbose: bool,
    },

    /// Get merchant profile completion status
    GetProfileCompletion {
        /// Show detailed output
        #[arg(short, long)]
        verbose: bool,
    },

    /// Check merchant eligibility for credit line
    CheckCreditEligibility {
        /// Merchant ID to check eligibility for
        #[arg(long)]
        merchant_id: String,
        /// Show detailed output
        #[arg(short, long)]
        verbose: bool,
    },

    /// Submit a credit line request
    SubmitCreditLineRequest {
        /// Merchant ID requesting credit
        #[arg(long)]
        merchant_id: String,
        /// Requested credit amount in USD cents
        #[arg(long)]
        requested_amount: u64,
        /// Requested rate in basis points (optional)
        #[arg(long)]
        requested_rate_bps: Option<u64>,
        /// Requested tenor in days
        #[arg(long)]
        requested_tenor_days: u32,
        /// Show detailed output
        #[arg(short, long)]
        verbose: bool,
    },

    /// Browse available liquidity pools for credit matching
    BrowseAvailablePools {
        /// Required liquidity amount in USD cents
        #[arg(long)]
        required_liquidity: u64,
        /// Maximum LTV ratio in basis points
        #[arg(long)]
        max_ltv_bps: u64,
        /// Maximum interest rate in basis points
        #[arg(long)]
        max_rate_bps: u64,
        /// Minimum repayment days
        #[arg(long)]
        min_repayment_days: u32,
        /// Show detailed output
        #[arg(short, long)]
        verbose: bool,
    },

    /// List credit line requests for a merchant
    ListCreditLineRequests {
        /// Merchant ID to list requests for
        #[arg(long)]
        merchant_id: String,
        /// Filter by status (submitted, evaluating, approved, rejected, expired)
        #[arg(long)]
        status: Option<String>,
        /// Limit results
        #[arg(long, default_value = "10")]
        limit: u32,
        /// Offset for pagination
        #[arg(long, default_value = "0")]
        offset: u32,
        /// Show detailed output
        #[arg(short, long)]
        verbose: bool,
    },

    /// Originate a new loan between parties
    OriginateLoan {
        /// Borrower user ID
        #[arg(long)]
        borrower_id: String,
        /// Lender user ID
        #[arg(long)]
        lender_id: String,
        /// Loan amount in USDC
        #[arg(long)]
        amount: f64,
        /// Interest rate (optional, e.g., 0.08 for 8%)
        #[arg(long)]
        interest_rate: Option<f64>,
        /// Loan term in days (optional, defaults to 365)
        #[arg(long, default_value = "365")]
        term_days: i32,
        /// Show detailed output
        #[arg(short, long)]
        verbose: bool,
    },

    /// Make a payment toward a loan
    MakePayment {
        /// Loan ID to make payment to
        #[arg(long)]
        loan_id: String,
        /// Payment amount in USDC
        #[arg(long)]
        amount: f64,
        /// Show detailed output
        #[arg(short, long)]
        verbose: bool,
    },

    /// Close out a loan
    CloseoutLoan {
        /// Loan ID to close out
        #[arg(long)]
        loan_id: String,
        /// Show detailed output
        #[arg(short, long)]
        verbose: bool,
    },

    /// List loans for a user
    ListLoans {
        /// User ID to list loans for
        #[arg(long)]
        user_id: String,
        /// Filter by loan status (active, paid_off, closed)
        #[arg(long)]
        status: Option<String>,
        /// Limit results
        #[arg(long, default_value = "20")]
        limit: Option<i32>,
        /// Show detailed output
        #[arg(short, long)]
        verbose: bool,
    },

    /// Get loan details
    GetLoan {
        /// Loan ID to retrieve
        #[arg(long)]
        loan_id: String,
        /// Show detailed output
        #[arg(short, long)]
        verbose: bool,
    },

    /// Test loan operations end-to-end
    Test {
        /// Number of test loans to create
        #[arg(long, default_value = "3")]
        count: Option<i32>,
        /// Test payment operations
        #[arg(long)]
        test_payments: bool,
        /// Test merchant operations
        #[arg(long)]
        test_merchants: bool,
        /// Show detailed output
        #[arg(short, long)]
        verbose: bool,
    },
}

/// Handle loan command routing and execution
pub async fn handle_loan_command(
    client: &BatchApiClient,
    command: LoanCommands,
    verbose: bool,
) -> Result<()> {
    match command {
        LoanCommands::RegisterInvestor {
            email,
            first_name,
            last_name,
            jurisdiction,
            accredited_investor,
            investment_capacity,
            regulatory_jurisdiction,
            invite_code,
            verbose: cmd_verbose,
        } => {
            register_investor(
                client,
                &email,
                first_name.as_deref(),
                last_name.as_deref(),
                jurisdiction.as_deref(),
                accredited_investor,
                investment_capacity.as_deref(),
                regulatory_jurisdiction.as_deref(),
                invite_code.as_deref(),
                cmd_verbose || verbose,
            )
            .await
        }
        LoanCommands::GetInvestorProfile {
            user_id,
            verbose: cmd_verbose,
        } => get_investor_profile(client, &user_id, cmd_verbose || verbose).await,
        LoanCommands::UpdateInvestorProfile {
            user_id,
            accredited_investor,
            investment_capacity,
            regulatory_jurisdiction,
            verbose: cmd_verbose,
        } => {
            update_investor_profile(
                client,
                &user_id,
                accredited_investor,
                investment_capacity.as_deref(),
                regulatory_jurisdiction.as_deref(),
                cmd_verbose || verbose,
            )
            .await
        }
        LoanCommands::RegisterMerchant {
            email,
            password,
            legal_name,
            dba,
            tax_id,
            mcc,
            country,
            business_type,
            jurisdiction,
            invite_code,
            verbose: cmd_verbose,
        } => {
            register_merchant(
                client,
                &email,
                &password,
                &legal_name,
                dba.as_deref(),
                tax_id.as_deref(),
                mcc.as_deref(),
                &country,
                business_type.as_deref(),
                jurisdiction.as_deref(),
                invite_code.as_deref(),
                cmd_verbose || verbose,
            )
            .await
        }
        LoanCommands::SubmitTurnover {
            date,
            gross_receipts,
            refunds,
            source,
            verification_method,
            verbose: cmd_verbose,
        } => {
            submit_turnover(
                client,
                &date,
                &gross_receipts,
                refunds.as_deref(),
                &source,
                verification_method.as_deref(),
                cmd_verbose || verbose,
            )
            .await
        }
        LoanCommands::GetTurnover {
            start_date,
            end_date,
            limit,
            verbose: cmd_verbose,
        } => {
            get_turnover(
                client,
                start_date.as_deref(),
                end_date.as_deref(),
                limit,
                cmd_verbose || verbose,
            )
            .await
        }
        LoanCommands::UpdateTurnover {
            date,
            gross_receipts,
            refunds,
            verification_method,
            verbose: cmd_verbose,
        } => {
            update_turnover(
                client,
                &date,
                gross_receipts.as_deref(),
                refunds.as_deref(),
                verification_method.as_deref(),
                cmd_verbose || verbose,
            )
            .await
        }
        LoanCommands::GetTurnoverAnalytics {
            period,
            verbose: cmd_verbose,
        } => get_turnover_analytics(client, &period, cmd_verbose || verbose).await,
        LoanCommands::GetProfileCompletion {
            verbose: cmd_verbose,
        } => get_profile_completion(client, cmd_verbose || verbose).await,
        LoanCommands::CheckCreditEligibility {
            merchant_id,
            verbose: cmd_verbose,
        } => check_credit_eligibility(client, &merchant_id, cmd_verbose || verbose).await,
        LoanCommands::SubmitCreditLineRequest {
            merchant_id,
            requested_amount,
            requested_rate_bps,
            requested_tenor_days,
            verbose: cmd_verbose,
        } => {
            submit_credit_line_request(
                client,
                &merchant_id,
                requested_amount,
                requested_rate_bps,
                requested_tenor_days,
                cmd_verbose || verbose,
            )
            .await
        }
        LoanCommands::BrowseAvailablePools {
            required_liquidity,
            max_ltv_bps,
            max_rate_bps,
            min_repayment_days,
            verbose: cmd_verbose,
        } => {
            browse_available_pools(
                client,
                required_liquidity,
                max_ltv_bps,
                max_rate_bps,
                min_repayment_days,
                cmd_verbose || verbose,
            )
            .await
        }
        LoanCommands::ListCreditLineRequests {
            merchant_id,
            status,
            limit,
            offset,
            verbose: cmd_verbose,
        } => {
            list_credit_line_requests(
                client,
                &merchant_id,
                status.as_deref(),
                limit,
                offset,
                cmd_verbose || verbose,
            )
            .await
        }
        LoanCommands::OriginateLoan {
            borrower_id,
            lender_id,
            amount,
            interest_rate,
            term_days,
            verbose: cmd_verbose,
        } => {
            originate_loan(
                client,
                &borrower_id,
                &lender_id,
                amount,
                interest_rate,
                term_days,
                cmd_verbose || verbose,
            )
            .await
        }
        LoanCommands::MakePayment {
            loan_id,
            amount,
            verbose: cmd_verbose,
        } => make_payment(client, &loan_id, amount, cmd_verbose || verbose).await,
        LoanCommands::CloseoutLoan {
            loan_id,
            verbose: cmd_verbose,
        } => closeout_loan(client, &loan_id, cmd_verbose || verbose).await,
        LoanCommands::ListLoans {
            user_id,
            status,
            limit,
            verbose: cmd_verbose,
        } => {
            list_loans(
                client,
                &user_id,
                status.as_deref(),
                limit,
                cmd_verbose || verbose,
            )
            .await
        }
        LoanCommands::GetLoan {
            loan_id,
            verbose: cmd_verbose,
        } => get_loan(client, &loan_id, cmd_verbose || verbose).await,
        LoanCommands::Test {
            count,
            test_payments,
            test_merchants,
            verbose: cmd_verbose,
        } => {
            test_loan_operations(
                client,
                count,
                test_payments,
                test_merchants,
                cmd_verbose || verbose,
            )
            .await
        }
    }
}
