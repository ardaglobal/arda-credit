//! Performance benchmarks and analytics operations
//!
//! This module handles all performance benchmarking and analytics operations for the Arda Credit CLI including:
//! - ZK proof verification and inclusion testing
//! - Performance benchmarking and stress testing
//! - Loan operation analytics and statistics
//! - Test data generation and system optimization
//! - Batch processing performance validation
//! - Nullifier generation testing
//!
//! All testing operations help ensure system reliability, performance targets,
//! and privacy guarantees while enabling comprehensive system validation.

use arda_credit_api::BatchApiClient;
use clap::Subcommand;
use eyre::Result;

// Sub-modules organized by scope
pub mod analytics; // Analytics and statistics functions
pub mod performance; // Performance testing and benchmarking
pub mod stress;
pub mod verification; // Proof verification functions // Stress testing and load testing

// Re-export all functions
pub use analytics::*;
pub use performance::*;
pub use stress::*;
pub use verification::*;

/// Performance benchmarking CLI commands
#[derive(Subcommand)]
pub enum BenchmarkCommands {
    /// Test prefixed nullifier generation for loan operations
    TestPrefixedNullifiers {
        /// Number of nullifiers to generate for testing
        #[arg(long, default_value = "100")]
        count: usize,
        /// Prefix to use for nullifier generation
        #[arg(long, default_value = "loan")]
        prefix: String,
        /// Show detailed timing and performance metrics
        #[arg(short, long)]
        verbose: bool,
    },

    /// Verify loan payment inclusion in batch proof
    VerifyLoanPayment {
        /// Batch ID containing the loan payment
        #[arg(long)]
        batch_id: i32,
        /// Loan ID to verify payment for
        #[arg(long)]
        loan_id: String,
        /// Payment amount to verify
        #[arg(long)]
        amount: String,
        /// Expected user ID for the payment
        #[arg(long)]
        user_id: String,
        /// Path to proof file (optional, will download if not provided)
        #[arg(long)]
        proof_file: Option<String>,
        /// Show detailed verification output
        #[arg(short, long)]
        verbose: bool,
    },

    /// Verify loan origination inclusion in batch proof
    VerifyLoanOrigination {
        /// Batch ID containing the loan origination
        #[arg(long)]
        batch_id: i32,
        /// Loan ID to verify origination for
        #[arg(long)]
        loan_id: String,
        /// Principal amount to verify
        #[arg(long)]
        principal: String,
        /// Borrower user ID
        #[arg(long)]
        borrower_id: String,
        /// Lender user ID
        #[arg(long)]
        lender_id: String,
        /// Path to proof file (optional, will download if not provided)
        #[arg(long)]
        proof_file: Option<String>,
        /// Show detailed verification output
        #[arg(short, long)]
        verbose: bool,
    },

    /// Verify generic loan operation inclusion in batch proof
    VerifyLoanOperation {
        /// Batch ID containing the loan operation
        #[arg(long)]
        batch_id: i32,
        /// Transaction ID to verify inclusion
        #[arg(long)]
        transaction_id: String,
        /// Transaction type (loan_origination, loan_payment, loan_closeout)
        #[arg(long)]
        transaction_type: String,
        /// Path to proof file (optional, will download if not provided)
        #[arg(long)]
        proof_file: Option<String>,
        /// Show detailed verification output
        #[arg(short, long)]
        verbose: bool,
    },

    /// Run performance tests demonstrating >60 TPS loan operations
    TestLoanPerformance {
        /// Number of loan operations to test
        #[arg(long, default_value = "1000")]
        operation_count: usize,
        /// Target TPS to achieve
        #[arg(long, default_value = "60")]
        target_tps: f64,
        /// Mix of operation types (origination:payment:closeout ratios)
        #[arg(long, default_value = "1:3:1")]
        operation_mix: String,
        /// Batch size for processing
        #[arg(long, default_value = "50")]
        batch_size: usize,
        /// Show detailed performance metrics
        #[arg(short, long)]
        verbose: bool,
        /// Run continuous performance test
        #[arg(long)]
        continuous: bool,
    },

    /// Query loan operation status from enhanced IMT
    QueryLoanStatus {
        /// Loan ID to query
        #[arg(long)]
        loan_id: String,
        /// Show operation history
        #[arg(long)]
        history: bool,
        /// Show audit trail
        #[arg(long)]
        audit_trail: bool,
        /// Maximum number of operations to show
        #[arg(long, default_value = "20")]
        limit: i32,
        /// Show detailed information
        #[arg(short, long)]
        verbose: bool,
    },

    /// Query user loan operations from IMT
    QueryUserLoans {
        /// User ID to query
        #[arg(long)]
        user_id: String,
        /// Filter by operation type
        #[arg(long)]
        operation_type: Option<String>,
        /// Maximum number of operations to show
        #[arg(long, default_value = "50")]
        limit: i32,
        /// Show detailed information
        #[arg(short, long)]
        verbose: bool,
    },

    /// Query loan operations by amount range
    QueryLoansByAmount {
        /// Minimum amount
        #[arg(long)]
        min_amount: String,
        /// Maximum amount
        #[arg(long)]
        max_amount: String,
        /// Filter by operation type
        #[arg(long)]
        operation_type: Option<String>,
        /// Maximum number of operations to show
        #[arg(long, default_value = "50")]
        limit: i32,
        /// Show detailed information
        #[arg(short, long)]
        verbose: bool,
    },

    /// Test high-volume batch processing with loan operations
    TestBatchProcessing {
        /// Number of batches to create and process
        #[arg(long, default_value = "10")]
        batch_count: usize,
        /// Operations per batch
        #[arg(long, default_value = "100")]
        operations_per_batch: usize,
        /// Target batch processing time (seconds)
        #[arg(long, default_value = "30")]
        target_time: u64,
        /// Show detailed batch statistics
        #[arg(short, long)]
        verbose: bool,
        /// Test with mixed operation types
        #[arg(long)]
        mixed_operations: bool,
    },

    /// Validate ZK proof generation performance (<10 minutes target)
    ValidateZkPerformance {
        /// Number of operations to include in proof
        #[arg(long, default_value = "500")]
        operation_count: usize,
        /// Target proof generation time (seconds)
        #[arg(long, default_value = "600")]
        target_time: u64,
        /// Include loan operations in proof
        #[arg(long)]
        include_loans: bool,
        /// Complexity level (1-5, higher = more complex operations)
        #[arg(long, default_value = "3")]
        complexity: u8,
        /// Show detailed timing breakdown
        #[arg(short, long)]
        verbose: bool,
    },

    /// Get loan operation statistics and analytics
    GetLoanStats {
        /// Show statistics for specific loan ID
        #[arg(long)]
        loan_id: Option<String>,
        /// Show user statistics for specific user ID
        #[arg(long)]
        user_id: Option<String>,
        /// Show batch statistics for specific batch ID
        #[arg(long)]
        batch_id: Option<i32>,
        /// Show system-wide statistics
        #[arg(long)]
        system_wide: bool,
        /// Include audit trail statistics
        #[arg(long)]
        include_audit: bool,
        /// Show detailed breakdown
        #[arg(short, long)]
        verbose: bool,
    },

    /// Generate loan operation test data for development and testing
    GenerateTestData {
        /// Number of users to create
        #[arg(long, default_value = "50")]
        users: usize,
        /// Number of loans to create
        #[arg(long, default_value = "100")]
        loans: usize,
        /// Number of payments per loan (average)
        #[arg(long, default_value = "10")]
        payments_per_loan: usize,
        /// Seed for reproducible test data
        #[arg(long)]
        seed: Option<u64>,
        /// Clean existing test data before generating
        #[arg(long)]
        clean: bool,
        /// Show generation progress
        #[arg(short, long)]
        verbose: bool,
    },

    /// Benchmark loan operation processing end-to-end
    BenchmarkLoanOperations {
        /// Duration of benchmark in seconds
        #[arg(long, default_value = "60")]
        duration: u64,
        /// Target operations per second
        #[arg(long, default_value = "60")]
        target_ops_per_sec: f64,
        /// Operation types to include
        #[arg(long, default_value = "all")]
        operation_types: String,
        /// Include batch processing in benchmark
        #[arg(long)]
        include_batching: bool,
        /// Include proof generation in benchmark
        #[arg(long)]
        include_proofs: bool,
        /// Show real-time metrics
        #[arg(short, long)]
        verbose: bool,
    },

    /// Refresh loan statistics and optimize performance
    RefreshLoanStats {
        /// Force complete recalculation
        #[arg(long)]
        force_recalc: bool,
        /// Optimize database indexes
        #[arg(long)]
        optimize_indexes: bool,
        /// Show optimization details
        #[arg(short, long)]
        verbose: bool,
    },

    /// Run comprehensive system stress tests
    StressTest {
        /// Duration of stress test in minutes
        #[arg(long, default_value = "10")]
        duration_minutes: u64,
        /// Concurrent operation threads
        #[arg(long, default_value = "10")]
        concurrent_threads: usize,
        /// Operations per thread per second
        #[arg(long, default_value = "10")]
        ops_per_thread_per_sec: f64,
        /// Include all operation types
        #[arg(long)]
        full_spectrum: bool,
        /// Show real-time metrics
        #[arg(short, long)]
        verbose: bool,
    },
}

/// Handle benchmark command routing and execution
pub async fn handle_benchmark_command(
    client: &BatchApiClient,
    command: BenchmarkCommands,
    verbose: bool,
) -> Result<()> {
    match command {
        BenchmarkCommands::TestPrefixedNullifiers {
            count,
            prefix,
            verbose: cmd_verbose,
        } => test_prefixed_nullifiers(count, &prefix, cmd_verbose || verbose).await,
        BenchmarkCommands::VerifyLoanPayment {
            batch_id,
            loan_id,
            amount,
            user_id,
            proof_file,
            verbose: cmd_verbose,
        } => {
            verify_loan_payment(
                client,
                batch_id,
                &loan_id,
                &amount,
                &user_id,
                proof_file.as_deref(),
                cmd_verbose || verbose,
            )
            .await
        }
        BenchmarkCommands::VerifyLoanOrigination {
            batch_id,
            loan_id,
            principal,
            borrower_id,
            lender_id,
            proof_file,
            verbose: cmd_verbose,
        } => {
            verify_loan_origination(
                client,
                batch_id,
                &loan_id,
                &principal,
                &borrower_id,
                &lender_id,
                proof_file.as_deref(),
                cmd_verbose || verbose,
            )
            .await
        }
        BenchmarkCommands::VerifyLoanOperation {
            batch_id,
            transaction_id,
            transaction_type,
            proof_file,
            verbose: cmd_verbose,
        } => {
            verify_loan_operation(
                client,
                batch_id,
                &transaction_id,
                &transaction_type,
                proof_file.as_deref(),
                cmd_verbose || verbose,
            )
            .await
        }
        BenchmarkCommands::TestLoanPerformance {
            operation_count,
            target_tps,
            operation_mix,
            batch_size,
            verbose: cmd_verbose,
            continuous,
        } => {
            test_loan_performance(
                client,
                operation_count,
                target_tps,
                &operation_mix,
                batch_size,
                cmd_verbose || verbose,
                continuous,
            )
            .await
        }
        BenchmarkCommands::QueryLoanStatus {
            loan_id,
            history,
            audit_trail,
            limit,
            verbose: cmd_verbose,
        } => {
            query_deal_status(
                client,
                &loan_id,
                history,
                audit_trail,
                limit,
                cmd_verbose || verbose,
            )
            .await
        }
        BenchmarkCommands::QueryUserLoans {
            user_id,
            operation_type,
            limit,
            verbose: cmd_verbose,
        } => {
            query_user_loans(
                client,
                &user_id,
                operation_type.as_deref(),
                limit,
                cmd_verbose || verbose,
            )
            .await
        }
        BenchmarkCommands::QueryLoansByAmount {
            min_amount,
            max_amount,
            operation_type,
            limit,
            verbose: cmd_verbose,
        } => {
            query_loans_by_amount(
                client,
                &min_amount,
                &max_amount,
                operation_type.as_deref(),
                limit,
                cmd_verbose || verbose,
            )
            .await
        }
        BenchmarkCommands::TestBatchProcessing {
            batch_count,
            operations_per_batch,
            target_time,
            verbose: cmd_verbose,
            mixed_operations,
        } => {
            test_batch_processing(
                client,
                batch_count,
                operations_per_batch,
                target_time,
                cmd_verbose || verbose,
                mixed_operations,
            )
            .await
        }
        BenchmarkCommands::ValidateZkPerformance {
            operation_count,
            target_time,
            include_loans,
            complexity,
            verbose: cmd_verbose,
        } => {
            validate_zk_performance(
                client,
                operation_count,
                target_time,
                include_loans,
                complexity,
                cmd_verbose || verbose,
            )
            .await
        }
        BenchmarkCommands::GetLoanStats {
            loan_id,
            user_id,
            batch_id,
            system_wide,
            include_audit,
            verbose: cmd_verbose,
        } => {
            get_loan_stats(
                client,
                loan_id.as_deref(),
                user_id.as_deref(),
                batch_id,
                system_wide,
                include_audit,
                cmd_verbose || verbose,
            )
            .await
        }
        BenchmarkCommands::GenerateTestData {
            users,
            loans,
            payments_per_loan,
            seed,
            clean,
            verbose: cmd_verbose,
        } => {
            generate_test_data(
                client,
                users,
                loans,
                payments_per_loan,
                seed,
                clean,
                cmd_verbose || verbose,
            )
            .await
        }
        BenchmarkCommands::BenchmarkLoanOperations {
            duration,
            target_ops_per_sec,
            operation_types,
            include_batching,
            include_proofs,
            verbose: cmd_verbose,
        } => {
            benchmark_loan_operations(
                client,
                duration,
                target_ops_per_sec,
                &operation_types,
                include_batching,
                include_proofs,
                cmd_verbose || verbose,
            )
            .await
        }
        BenchmarkCommands::RefreshLoanStats {
            force_recalc,
            optimize_indexes,
            verbose: cmd_verbose,
        } => {
            refresh_loan_stats(
                client,
                force_recalc,
                optimize_indexes,
                cmd_verbose || verbose,
            )
            .await
        }
        BenchmarkCommands::StressTest {
            duration_minutes,
            concurrent_threads,
            ops_per_thread_per_sec,
            full_spectrum,
            verbose: cmd_verbose,
        } => {
            stress_test(
                client,
                duration_minutes,
                concurrent_threads,
                ops_per_thread_per_sec,
                full_spectrum,
                cmd_verbose || verbose,
            )
            .await
        }
    }
}
