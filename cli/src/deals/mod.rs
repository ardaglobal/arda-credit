//! Deal management commands with consolidated marketplace and transfer functionality
//!
//! This module handles all deal operations including:
//! - Deal origination between originators and investors
//! - Payment processing and deal servicing
//! - Deal closeout and settlement
//! - Deal status tracking and retrieval
//! - Marketplace operations (listing, browsing, removing deals for sale)
//! - Deal transfer operations (ownership transfers between investors)
//! - End-to-end deal testing and validation
//! - Credit line and liquidity pool operations
//! - Merchant onboarding and credit eligibility
//!
//! All deal operations maintain privacy through ZK proofs while enabling regulatory
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

/// Deal management commands with consolidated marketplace and transfer functionality
#[derive(Subcommand)]
pub enum DealCommands {
    // ========================================================================
    // CORE DEAL MANAGEMENT COMMANDS
    // ========================================================================
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

    /// Originate a new deal between parties
    Originate {
        /// Originator user ID (the party requesting the deal)
        #[arg(long)]
        originator_id: String, // was borrower_id
        /// Investor user ID (the party providing capital)
        #[arg(long)]
        investor_id: String, // was lender_id
        /// Deal amount in USDC
        #[arg(long)]
        amount: f64,
        /// Interest rate (optional, e.g., 0.08 for 8%)
        #[arg(long)]
        interest_rate: Option<f64>,
        /// Deal term in days (optional, defaults to 365)
        #[arg(long, default_value = "365")]
        term_days: i32,
        /// Show detailed output
        #[arg(short, long)]
        verbose: bool,
    },

    /// Make a payment toward a deal
    Payment {
        /// Deal ID to make payment to
        #[arg(long)]
        deal_id: String, // was loan_id
        /// Payment amount in USDC
        #[arg(long)]
        amount: f64,
        /// Show detailed output
        #[arg(short, long)]
        verbose: bool,
    },

    /// Close out a deal
    Closeout {
        /// Deal ID to close out
        #[arg(long)]
        deal_id: String, // was loan_id
        /// Show detailed output
        #[arg(short, long)]
        verbose: bool,
    },

    /// List deals for a user
    List {
        /// User ID to list deals for (optional - if not provided, lists all accessible deals)
        #[arg(long)]
        user_id: Option<String>,
        /// Filter by deal status (active, paid_off, closed)
        #[arg(long)]
        status: Option<String>,
        /// Limit results
        #[arg(long, default_value = "20")]
        limit: Option<i32>,
        /// Show detailed output
        #[arg(short, long)]
        verbose: bool,
    },

    /// Get deal details
    Get {
        /// Deal ID to retrieve
        #[arg(long)]
        deal_id: String, // was loan_id
        /// Show detailed output
        #[arg(short, long)]
        verbose: bool,
    },

    /// Update deal details
    Update {
        /// Deal ID to update
        #[arg(long)]
        deal_id: String, // was loan_id
        /// New outstanding balance
        #[arg(long)]
        outstanding_balance: Option<f64>,
        /// New deal status
        #[arg(long)]
        status: Option<String>,
        /// Show detailed output
        #[arg(short, long)]
        verbose: bool,
    },

    // ========================================================================
    // CONSOLIDATED MARKETPLACE COMMANDS (moved from separate module)
    // ========================================================================
    /// List a deal for sale in the marketplace
    ListForSale {
        /// Deal ID to list for sale
        #[arg(long)]
        deal_id: String, // was loan_id
        /// Asking price in USDC
        #[arg(long)]
        asking_price: f64,
        /// Optional description of the deal
        #[arg(long)]
        description: Option<String>,
        /// Seller user ID
        #[arg(long)]
        seller_id: String,
        /// Show detailed output
        #[arg(short, long)]
        verbose: bool,
    },

    /// Browse deals available for sale
    BrowseMarketplace {
        /// Maximum number of deals to display
        #[arg(long, default_value = "20")]
        limit: Option<i32>,
        /// Filter by listing status
        #[arg(long)]
        status: Option<String>,
        /// Show detailed output
        #[arg(short, long)]
        verbose: bool,
    },

    /// Remove a deal from sale
    RemoveFromSale {
        /// Deal ID to remove from sale
        #[arg(long)]
        deal_id: String, // was loan_id
        /// Show detailed output
        #[arg(short, long)]
        verbose: bool,
    },

    /// Get marketplace statistics
    MarketplaceStats {
        /// Show detailed output
        #[arg(short, long)]
        verbose: bool,
    },

    // ========================================================================
    // CONSOLIDATED TRANSFER COMMANDS (moved from separate module)
    // ========================================================================
    /// Transfer deal ownership to another investor
    Transfer {
        /// Deal ID to transfer
        #[arg(long)]
        deal_id: String, // was loan_id
        /// Current investor user ID (must own the deal)
        #[arg(long)]
        from_investor_id: String, // was from_lender_id
        /// New investor user ID
        #[arg(long)]
        to_investor_id: String, // was to_lender_id
        /// Transfer price in USDC
        #[arg(long)]
        transfer_price: f64,
        /// Show detailed output
        #[arg(short, long)]
        verbose: bool,
    },

    /// Get transfer history for a deal
    TransferHistory {
        /// Deal ID to get transfer history for
        #[arg(long)]
        deal_id: String, // was loan_id
        /// Maximum number of transfers to display
        #[arg(long, default_value = "10")]
        limit: Option<i32>,
        /// Show detailed output
        #[arg(short, long)]
        verbose: bool,
    },

    /// Validate if a deal transfer is possible
    ValidateTransfer {
        /// Deal ID to validate transfer for
        #[arg(long)]
        deal_id: String, // was loan_id
        /// Current investor user ID
        #[arg(long)]
        current_investor_id: String, // was current_lender_id
        /// Show detailed output
        #[arg(short, long)]
        verbose: bool,
    },

    /// Get user's deal transfer history
    UserTransferHistory {
        /// User ID to get transfer history for
        #[arg(long)]
        user_id: String,
        /// Maximum number of transfers to display
        #[arg(long, default_value = "10")]
        limit: Option<i32>,
        /// Show detailed output
        #[arg(short, long)]
        verbose: bool,
    },

    // ========================================================================
    // TESTING AND DEVELOPMENT COMMANDS
    // ========================================================================
    /// Test deal operations end-to-end
    Test {
        /// Number of test deals to create
        #[arg(long, default_value = "3")]
        count: Option<u32>,
        /// Test deal payment functionality
        #[arg(long)]
        test_payments: bool,
        /// Test marketplace functionality
        #[arg(long)]
        test_marketplace: bool,
        /// Test transfer functionality
        #[arg(long)]
        test_transfers: bool,
        /// Show detailed output
        #[arg(short, long)]
        verbose: bool,
    },
}

/// Handle deal command routing and execution with consolidated marketplace and transfer functionality
pub async fn handle_deal_command(
    client: &BatchApiClient,
    command: DealCommands,
    verbose: bool,
) -> Result<()> {
    match command {
        // ====================================================================
        // INVESTOR MANAGEMENT HANDLERS
        // ====================================================================
        DealCommands::RegisterInvestor {
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
        DealCommands::GetInvestorProfile {
            user_id,
            verbose: cmd_verbose,
        } => get_investor_profile(client, &user_id, cmd_verbose || verbose).await,
        DealCommands::UpdateInvestorProfile {
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

        // ====================================================================
        // MERCHANT MANAGEMENT HANDLERS
        // ====================================================================
        DealCommands::RegisterMerchant {
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
        DealCommands::SubmitTurnover {
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
        DealCommands::GetTurnover {
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
        DealCommands::UpdateTurnover {
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
        DealCommands::GetTurnoverAnalytics {
            period,
            verbose: cmd_verbose,
        } => get_turnover_analytics(client, &period, cmd_verbose || verbose).await,
        DealCommands::GetProfileCompletion {
            verbose: cmd_verbose,
        } => get_profile_completion(client, cmd_verbose || verbose).await,
        DealCommands::CheckCreditEligibility {
            merchant_id,
            verbose: cmd_verbose,
        } => check_credit_eligibility(client, &merchant_id, cmd_verbose || verbose).await,
        DealCommands::SubmitCreditLineRequest {
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
        DealCommands::BrowseAvailablePools {
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
        DealCommands::ListCreditLineRequests {
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

        // ====================================================================
        // CORE DEAL MANAGEMENT HANDLERS
        // ====================================================================
        DealCommands::Originate {
            originator_id, // was borrower_id
            investor_id,   // was lender_id
            amount,
            interest_rate,
            term_days,
            verbose: cmd_verbose,
        } => {
            originate_deal(
                // was originate_loan
                client,
                &originator_id, // was &borrower_id
                &investor_id,   // was &lender_id
                amount,
                interest_rate,
                term_days,
                cmd_verbose || verbose,
            )
            .await
        }

        DealCommands::Payment {
            // was LoanCommands::MakePayment
            deal_id, // was loan_id
            amount,
            verbose: cmd_verbose,
        } => {
            make_deal_payment(
                // was make_loan_payment
                client,
                &deal_id, // was &loan_id
                amount,
                cmd_verbose || verbose,
            )
            .await
        }

        DealCommands::Closeout {
            // was LoanCommands::CloseoutLoan
            deal_id, // was loan_id
            verbose: cmd_verbose,
        } => {
            closeout_deal(
                // was closeout_loan
                client,
                &deal_id, // was &loan_id
                cmd_verbose || verbose,
            )
            .await
        }

        DealCommands::List {
            // was LoanCommands::ListLoans
            user_id,
            status,
            limit,
            verbose: cmd_verbose,
        } => {
            list_deals(
                // was list_loans
                client,
                user_id.as_deref(),
                status.as_deref(),
                limit,
                cmd_verbose || verbose,
            )
            .await
        }

        DealCommands::Get {
            // was LoanCommands::GetLoan
            deal_id, // was loan_id
            verbose: cmd_verbose,
        } => {
            get_deal(
                // was get_loan
                client,
                &deal_id, // was &loan_id
                cmd_verbose || verbose,
            )
            .await
        }

        DealCommands::Update {
            deal_id, // was loan_id
            outstanding_balance,
            status,
            verbose: cmd_verbose,
        } => {
            update_deal(
                // was update_loan
                client,
                &deal_id, // was &loan_id
                outstanding_balance,
                status.as_deref(),
                cmd_verbose || verbose,
            )
            .await
        }

        // ====================================================================
        // CONSOLIDATED MARKETPLACE HANDLERS (moved from marketplace module)
        // ====================================================================
        DealCommands::ListForSale {
            deal_id, // was loan_id
            asking_price,
            description,
            seller_id,
            verbose: cmd_verbose,
        } => {
            list_deal_for_sale(
                // was list_loan_for_sale
                client,
                &deal_id, // was &loan_id
                asking_price,
                description.as_deref(),
                &seller_id,
                cmd_verbose || verbose,
            )
            .await
        }

        DealCommands::BrowseMarketplace {
            limit,
            status,
            verbose: cmd_verbose,
        } => {
            browse_deal_marketplace(
                // was browse_loan_marketplace
                client,
                limit,
                status.as_deref(),
                cmd_verbose || verbose,
            )
            .await
        }

        DealCommands::RemoveFromSale {
            deal_id, // was loan_id
            verbose: cmd_verbose,
        } => {
            remove_deal_from_sale(
                // was remove_loan_from_sale
                client,
                &deal_id, // was &loan_id
                cmd_verbose || verbose,
            )
            .await
        }

        DealCommands::MarketplaceStats {
            verbose: cmd_verbose,
        } => {
            get_deal_marketplace_stats(
                // was get_loan_marketplace_stats
                client,
                cmd_verbose || verbose,
            )
            .await
        }

        // ====================================================================
        // CONSOLIDATED TRANSFER HANDLERS (moved from transfer module)
        // ====================================================================
        DealCommands::Transfer {
            deal_id,          // was loan_id
            from_investor_id, // was from_lender_id
            to_investor_id,   // was to_lender_id
            transfer_price,
            verbose: cmd_verbose,
        } => {
            transfer_deal_ownership(
                // was transfer_loan_ownership
                client,
                &deal_id,          // was &loan_id
                &from_investor_id, // was &from_lender_id
                &to_investor_id,   // was &to_lender_id
                transfer_price,
                cmd_verbose || verbose,
            )
            .await
        }

        DealCommands::TransferHistory {
            deal_id, // was loan_id
            limit,
            verbose: cmd_verbose,
        } => {
            get_deal_transfer_history(
                // was get_loan_transfer_history
                client,
                &deal_id, // was &loan_id
                limit,
                cmd_verbose || verbose,
            )
            .await
        }

        DealCommands::ValidateTransfer {
            deal_id,             // was loan_id
            current_investor_id, // was current_lender_id
            verbose: cmd_verbose,
        } => {
            validate_deal_transfer(
                // was validate_loan_transfer
                client,
                &deal_id,             // was &loan_id
                &current_investor_id, // was &current_lender_id
                cmd_verbose || verbose,
            )
            .await
        }

        DealCommands::UserTransferHistory {
            user_id,
            limit,
            verbose: cmd_verbose,
        } => {
            get_user_deal_transfer_history(
                // was get_user_loan_transfer_history
                client,
                &user_id,
                limit,
                cmd_verbose || verbose,
            )
            .await
        }

        // ====================================================================
        // TESTING HANDLERS
        // ====================================================================
        DealCommands::Test {
            count,
            test_payments,
            test_marketplace,
            test_transfers,
            verbose: cmd_verbose,
        } => {
            test_deal_operations(
                // was test_loan_operations
                client,
                count,
                test_payments,
                test_marketplace,
                test_transfers,
                cmd_verbose || verbose,
            )
            .await
        }
    }
}
