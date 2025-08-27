/// Comprehensive demonstration of the enhanced loan-aware IMT functionality
/// 
/// This example shows how to:
/// 1. Track loan lifecycle operations in the IMT
/// 2. Query loan operation history efficiently
/// 3. Generate loan statistics and audit trails
/// 4. Integrate with batch processing
/// 
/// Run with: cargo run --example loan_aware_imt_demo

use chrono::{Duration, Utc};
use db::merkle_tree::*;
use db::error::DbError;
use sqlx::PgPool;
use std::env;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::init();
    
    // Connect to database
    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:password@127.0.0.1:5432/arda_credit_db".to_string());
    
    let pool = PgPool::connect(&database_url).await?;
    
    println!("🚀 Enhanced Loan-Aware IMT Demonstration");
    println!("==========================================\n");
    
    // Initialize IMT
    let imt = IndexedMerkleTree::new(pool);
    
    // Demo 1: Loan Lifecycle Tracking
    println!("📊 Demo 1: Loan Lifecycle Tracking");
    demo_loan_lifecycle_tracking(&imt).await?;
    
    // Demo 2: Efficient Query Operations
    println!("\n🔍 Demo 2: Efficient Query Operations");
    demo_query_operations(&imt).await?;
    
    // Demo 3: Statistics and Analytics
    println!("\n📈 Demo 3: Statistics and Analytics");
    demo_statistics_analytics(&imt).await?;
    
    // Demo 4: Audit Trail Integration
    println!("\n🔐 Demo 4: Audit Trail Integration");
    demo_audit_trail(&imt).await?;
    
    // Demo 5: Batch Processing Integration
    println!("\n⚡ Demo 5: Batch Processing Integration");
    demo_batch_processing(&imt).await?;
    
    // Demo 6: Advanced Query Scenarios
    println!("\n🎯 Demo 6: Advanced Query Scenarios");
    demo_advanced_queries(&imt).await?;
    
    println!("\n✅ All demonstrations completed successfully!");
    println!("The IMT is now loan-aware while maintaining all existing functionality.");
    
    Ok(())
}

/// Demonstrate loan lifecycle tracking from origination to closeout
async fn demo_loan_lifecycle_tracking(imt: &IndexedMerkleTree) -> Result<(), DbError> {
    println!("   → Tracking complete loan lifecycle...");
    
    let loan_id = Uuid::new_v4();
    let borrower_id = Uuid::new_v4();
    let lender_id = Uuid::new_v4();
    
    // Step 1: Loan Origination
    let origination_result = imt
        .insert_loan_operation(
            100001,
            "loan_origination".to_string(),
            Some(loan_id),
            Some(borrower_id),
            Some(rust_decimal::Decimal::new(5000000, 2)), // $50,000.00
            Some(serde_json::json!({
                "principal": 50000.00,
                "interest_rate": 5.5,
                "term_months": 60,
                "lender_id": lender_id,
                "origination_date": Utc::now()
            })),
            None,
        )
        .await?;
    
    println!("   ✓ Loan originated: Nullifier ID {}, Tree Index {}", 
             origination_result.nullifier.id, origination_result.nullifier.tree_index);
    
    // Step 2: Monthly Payments
    let payment_amounts = vec![87500, 87500, 87500]; // $875.00 each
    for (i, &amount) in payment_amounts.iter().enumerate() {
        let payment_result = imt
            .insert_loan_operation(
                100002 + i as i64,
                "loan_payment".to_string(),
                Some(loan_id),
                Some(borrower_id),
                Some(rust_decimal::Decimal::new(amount, 2)),
                Some(serde_json::json!({
                    "payment_number": i + 1,
                    "payment_date": Utc::now() + Duration::days(30 * (i as i64 + 1)),
                    "principal_portion": amount * 70 / 100,
                    "interest_portion": amount * 30 / 100
                })),
                None,
            )
            .await?;
        
        println!("   ✓ Payment #{}: Nullifier ID {}, Amount ${:.2}", 
                 i + 1, payment_result.nullifier.id, amount as f64 / 100.0);
    }
    
    // Step 3: Early Loan Closeout
    let closeout_result = imt
        .insert_loan_operation(
            100006,
            "loan_closeout".to_string(),
            Some(loan_id),
            Some(borrower_id),
            Some(rust_decimal::Decimal::new(4737500, 2)), // $47,375.00 remaining balance
            Some(serde_json::json!({
                "closeout_type": "early_payoff",
                "remaining_balance": 47375.00,
                "closeout_date": Utc::now() + Duration::days(120),
                "savings": 2500.00
            })),
            None,
        )
        .await?;
    
    println!("   ✓ Loan closed out: Nullifier ID {}, Final Balance ${:.2}",
             closeout_result.nullifier.id, 
             closeout_result.nullifier.operation_amount.unwrap_or_default());
    
    Ok(())
}

/// Demonstrate efficient query operations for loan data
async fn demo_query_operations(imt: &IndexedMerkleTree) -> Result<(), DbError> {
    println!("   → Demonstrating efficient query operations...");
    
    // Create test data for multiple users and loans
    let users = vec![Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()];
    let loans = vec![Uuid::new_v4(), Uuid::new_v4()];
    
    // Insert diverse loan operations
    let operations = vec![
        (loans[0], users[0], "loan_origination", 3000000), // $30,000
        (loans[0], users[0], "loan_payment", 150000),      // $1,500
        (loans[1], users[1], "loan_origination", 1500000), // $15,000
        (loans[1], users[1], "loan_payment", 75000),       // $750
        (loans[0], users[0], "loan_payment", 150000),      // $1,500
    ];
    
    for (i, (loan_id, user_id, op_type, amount)) in operations.iter().enumerate() {
        imt.insert_loan_operation(
            200001 + i as i64,
            op_type.to_string(),
            Some(*loan_id),
            Some(*user_id),
            Some(rust_decimal::Decimal::new(*amount, 2)),
            Some(serde_json::json!({
                "demo": "query_operations",
                "sequence": i + 1
            })),
            None,
        ).await?;
    }
    
    // Query 1: Loan Operation History
    let loan_history = imt.get_loan_operation_history(loans[0], Some(10)).await?;
    println!("   ✓ Loan {} has {} operations", 
             loans[0], loan_history.len());
    
    // Query 2: User Loan Operations
    let user_operations = imt.get_user_loan_operations(users[0], None, Some(10)).await?;
    println!("   ✓ User {} has {} total operations", 
             users[0], user_operations.len());
    
    // Query 3: Filtered User Operations (payments only)
    let user_payments = imt.get_user_loan_operations(
        users[0], 
        Some("loan_payment".to_string()), 
        Some(10)
    ).await?;
    println!("   ✓ User {} has {} payment operations", 
             users[0], user_payments.len());
    
    // Query 4: Operations by Date Range
    let start_date = Utc::now() - Duration::hours(1);
    let end_date = Utc::now() + Duration::hours(1);
    let recent_operations = imt.get_loan_operations_by_type_range(
        "loan_origination".to_string(),
        start_date,
        end_date,
        Some(10)
    ).await?;
    println!("   ✓ Found {} loan originations in the last hour", 
             recent_operations.len());
    
    Ok(())
}

/// Demonstrate statistics and analytics capabilities
async fn demo_statistics_analytics(imt: &IndexedMerkleTree) -> Result<(), DbError> {
    println!("   → Generating loan statistics and analytics...");
    
    // Create sample data for analytics
    let loan_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    
    let analytics_operations = vec![
        ("loan_origination", 10000000), // $100,000
        ("loan_payment", 500000),       // $5,000
        ("loan_payment", 500000),       // $5,000
        ("loan_payment", 500000),       // $5,000
    ];
    
    for (i, (op_type, amount)) in analytics_operations.iter().enumerate() {
        imt.insert_loan_operation(
            300001 + i as i64,
            op_type.to_string(),
            Some(loan_id),
            Some(user_id),
            Some(rust_decimal::Decimal::new(*amount, 2)),
            Some(serde_json::json!({
                "analytics_demo": true,
                "category": "high_value_loan"
            })),
            None,
        ).await?;
    }
    
    // Get IMT Loan Statistics
    if let Some(stats) = imt.get_imt_loan_stats().await? {
        println!("   📊 IMT Loan Statistics:");
        println!("      • Total Loan Nullifiers: {}", 
                 stats.total_loan_nullifiers.unwrap_or(0));
        println!("      • Total Originations: {}", 
                 stats.total_loan_originations.unwrap_or(0));
        println!("      • Total Payments: {}", 
                 stats.total_loan_payments.unwrap_or(0));
        println!("      • Unique Loans: {}", 
                 stats.unique_loans.unwrap_or(0));
        println!("      • Unique Users: {}", 
                 stats.unique_users.unwrap_or(0));
        
        if let Some(total_amount) = stats.total_loan_amount {
            println!("      • Total Loan Amount: ${:.2}", total_amount);
        }
    }
    
    // Refresh statistics to ensure accuracy
    imt.refresh_loan_statistics().await?;
    println!("   ✓ Statistics refreshed and optimized");
    
    Ok(())
}

/// Demonstrate audit trail integration and tracking
async fn demo_audit_trail(imt: &IndexedMerkleTree) -> Result<(), DbError> {
    println!("   → Demonstrating audit trail integration...");
    
    let loan_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    
    // Insert operations that will generate audit trail
    let audit_operations = vec![
        ("loan_origination", 2000000, "Initial loan setup"),
        ("loan_payment", 100000, "First monthly payment"),
        ("loan_payment", 100000, "Second monthly payment"),
    ];
    
    for (i, (op_type, amount, description)) in audit_operations.iter().enumerate() {
        imt.insert_loan_operation(
            400001 + i as i64,
            op_type.to_string(),
            Some(loan_id),
            Some(user_id),
            Some(rust_decimal::Decimal::new(*amount, 2)),
            Some(serde_json::json!({
                "description": description,
                "audit_demo": true,
                "compliance_checked": true
            })),
            None,
        ).await?;
        
        // Small delay to ensure distinct timestamps
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
    
    // Query audit trail
    let loan_audit = imt.get_loan_audit_trail(loan_id, Some(10)).await?;
    println!("   🔍 Audit Trail for Loan {}:", loan_id);
    for (i, event) in loan_audit.iter().enumerate() {
        println!("      {}. {} - ${:.2} at {}", 
                 i + 1,
                 event.loan_operation_type.as_ref().unwrap_or(&"unknown".to_string()),
                 event.operation_amount.unwrap_or_default(),
                 event.timestamp.format("%Y-%m-%d %H:%M:%S"));
    }
    
    // Query user audit trail
    let user_audit = imt.get_user_loan_audit_trail(user_id, None, Some(10)).await?;
    println!("   ✓ User {} has {} audit trail entries", 
             user_id, user_audit.len());
    
    // Get comprehensive audit information
    let comprehensive_audit = imt.get_loan_audit_comprehensive(Some(loan_id), Some(10)).await?;
    println!("   ✓ Retrieved {} comprehensive audit records", 
             comprehensive_audit.len());
    
    // Get audit statistics
    let audit_stats = imt.get_loan_audit_stats().await?;
    println!("   ✓ Retrieved audit statistics for {} operation types", 
             audit_stats.len());
    
    Ok(())
}

/// Demonstrate batch processing integration
async fn demo_batch_processing(imt: &IndexedMerkleTree) -> Result<(), DbError> {
    println!("   → Demonstrating batch processing integration...");
    
    // Create unbatched operations
    let loan_ids = vec![Uuid::new_v4(), Uuid::new_v4()];
    let user_id = Uuid::new_v4();
    
    let batch_operations = vec![
        (loan_ids[0], "loan_origination", 4000000), // $40,000
        (loan_ids[0], "loan_payment", 200000),      // $2,000
        (loan_ids[1], "loan_origination", 2500000), // $25,000
        (loan_ids[1], "loan_payment", 125000),      // $1,250
    ];
    
    for (i, (loan_id, op_type, amount)) in batch_operations.iter().enumerate() {
        imt.insert_loan_operation(
            500001 + i as i64,
            op_type.to_string(),
            Some(*loan_id),
            Some(user_id),
            Some(rust_decimal::Decimal::new(*amount, 2)),
            Some(serde_json::json!({
                "batch_demo": true,
                "ready_for_processing": true
            })),
            None, // No batch_id initially
        ).await?;
    }
    
    // Get unbatched operations
    let unbatched = imt.get_unbatched_loan_operations(Some(20)).await?;
    println!("   📦 Found {} unbatched loan operations", unbatched.len());
    
    // Simulate batch processing
    if !unbatched.is_empty() {
        let nullifier_ids: Vec<i64> = unbatched.iter().map(|n| n.id).collect();
        let batch_id = 1001; // Simulated batch ID
        
        let affected_rows = imt.mark_loan_operations_batched(&nullifier_ids, batch_id).await?;
        println!("   ✓ Marked {} operations as batched with batch ID {}", 
                 affected_rows, batch_id);
        
        // Get batch summary
        if let Some(batch_summary) = imt.get_batch_loan_summary(batch_id).await? {
            println!("   📋 Batch {} Summary:", batch_id);
            println!("      • Total Operations: {}", 
                     batch_summary.total_loan_operations.unwrap_or(0));
            println!("      • Originations: {}", 
                     batch_summary.loan_originations.unwrap_or(0));
            println!("      • Payments: {}", 
                     batch_summary.loan_payments.unwrap_or(0));
            println!("      • Unique Loans: {}", 
                     batch_summary.unique_loans_affected.unwrap_or(0));
            println!("      • Total Amount: ${:.2}", 
                     batch_summary.total_loan_amount.unwrap_or_default());
        }
        
        // Verify no remaining unbatched operations
        let remaining_unbatched = imt.get_unbatched_loan_operations(Some(20)).await?;
        println!("   ✓ Remaining unbatched operations: {}", remaining_unbatched.len());
        
        // Get batch audit trail
        let batch_audit = imt.get_batch_audit_trail(batch_id).await?;
        println!("   ✓ Batch audit trail contains {} events", batch_audit.len());
    }
    
    Ok(())
}

/// Demonstrate advanced query scenarios
async fn demo_advanced_queries(imt: &IndexedMerkleTree) -> Result<(), DbError> {
    println!("   → Running advanced query scenarios...");
    
    let loan_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    
    // Create diverse operations with varying amounts
    let advanced_operations = vec![
        ("loan_origination", 750000),  // $7,500
        ("loan_payment", 50000),       // $500
        ("loan_payment", 150000),      // $1,500
        ("loan_payment", 300000),      // $3,000
        ("loan_payment", 75000),       // $750
    ];
    
    for (i, (op_type, amount)) in advanced_operations.iter().enumerate() {
        imt.insert_loan_operation(
            600001 + i as i64,
            op_type.to_string(),
            Some(loan_id),
            Some(user_id),
            Some(rust_decimal::Decimal::new(*amount, 2)),
            Some(serde_json::json!({
                "advanced_demo": true,
                "risk_category": if *amount > 200000 { "high" } else { "low" }
            })),
            None,
        ).await?;
    }
    
    // Query 1: Amount Range Queries
    let min_amount = rust_decimal::Decimal::new(100000, 2); // $1,000
    let max_amount = rust_decimal::Decimal::new(400000, 2); // $4,000
    
    let range_operations = imt.get_loan_operations_by_amount_range(
        min_amount,
        max_amount,
        None
    ).await?;
    println!("   🎯 Found {} operations between ${:.2} and ${:.2}",
             range_operations.len(),
             min_amount,
             max_amount);
    
    // Query 2: Payment-specific amount range
    let payment_range = imt.get_loan_operations_by_amount_range(
        min_amount,
        max_amount,
        Some("loan_payment".to_string())
    ).await?;
    println!("   💰 Found {} payment operations in amount range", 
             payment_range.len());
    
    // Query 3: High-value operations (above $2,000)
    let high_value_min = rust_decimal::Decimal::new(200000, 2);
    let high_value_max = rust_decimal::Decimal::new(10000000, 2);
    
    let high_value_ops = imt.get_loan_operations_by_amount_range(
        high_value_min,
        high_value_max,
        None
    ).await?;
    println!("   🔥 Found {} high-value operations (>${:.2})", 
             high_value_ops.len(), high_value_min);
    
    // Display operation details
    for op in high_value_ops.iter().take(3) {
        println!("      • {} - ${:.2} at index {}", 
                 op.operation_type, 
                 op.operation_amount.unwrap_or_default(),
                 op.tree_index);
    }
    
    Ok(())
}