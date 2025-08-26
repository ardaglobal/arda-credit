#!/usr/bin/env cargo run --bin populate_test_data --
//! Test data population script for the Arda Credit database
//!
//! This script creates fake identities, loans, and payments for testing the CCR domain.
//! Run with: cargo run --bin populate_test_data

use chrono::Utc;
use db::{
    create_identity, init_db_with_url, Identity
};
use rust_decimal::Decimal;
use std::env;
use std::str::FromStr;
use uuid::Uuid;

// Local structs to match database schema
#[derive(Debug, Clone)]
pub struct Loan {
    pub id: Uuid,
    pub borrower_id: Uuid,
    pub lender_id: Uuid,
    pub principal_amount: Decimal,
    pub outstanding_balance: Decimal,
    pub interest_rate_bps: i32,
    pub loan_status: String,
    pub originated_at: chrono::DateTime<Utc>,
    pub maturity_date: Option<chrono::DateTime<Utc>>,
    pub created_at: chrono::DateTime<Utc>,
    pub updated_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct Payment {
    pub id: Uuid,
    pub loan_id: Uuid,
    pub payer_id: Uuid,
    pub amount: Decimal,
    pub payment_type: String,
    pub transaction_hash: Option<String>,
    pub processed_at: chrono::DateTime<Utc>,
    pub created_at: chrono::DateTime<Utc>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    println!("🚀 Starting database population with test data...");
    
    // Get database URL from environment
    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:password@127.0.0.1:5432/arda_credit_db".to_string());
    
    // Initialize database connection
    let pool = init_db_with_url(&database_url).await?;
    println!("✅ Connected to database");
    
    // Clear existing test data (optional)
    println!("🧹 Clearing existing test data...");
    clear_test_data(&pool).await?;
    
    // Create test identities
    println!("👥 Creating test identities...");
    let identities = create_test_identities(&pool).await?;
    println!("✅ Created {} identities", identities.len());
    
    // Create test loans
    println!("💰 Creating test loans...");
    let loans = create_test_loans(&pool, &identities).await?;
    println!("✅ Created {} loans", loans.len());
    
    // Create test payments
    println!("💳 Creating test payments...");
    let payments = create_test_payments(&pool, &loans, &identities).await?;
    println!("✅ Created {} payments", payments.len());
    
    // Print summary
    print_summary(&identities, &loans, &payments).await;
    
    println!("🎉 Database population completed successfully!");
    
    Ok(())
}

/// Clear existing test data from the database
async fn clear_test_data(pool: &sqlx::PgPool) -> Result<(), sqlx::Error> {
    // Delete in dependency order
    sqlx::query!("DELETE FROM payments").execute(pool).await?;
    sqlx::query!("DELETE FROM loans").execute(pool).await?;
    sqlx::query!("DELETE FROM balances").execute(pool).await?;
    sqlx::query!("DELETE FROM identities").execute(pool).await?;
    Ok(())
}

/// Create test identities with various KYC statuses
async fn create_test_identities(pool: &sqlx::PgPool) -> Result<Vec<Identity>, sqlx::Error> {
    let mut identities = Vec::new();
    
    // Test users with different types and KYC statuses
    let test_users = vec![
        ("Alice Johnson", "investor", "alice.johnson@example.com", "approved"),
        ("Bob Smith", "lender", "bob.smith@example.com", "approved"), 
        ("Carol Davis", "investor", "carol.davis@example.com", "pending"),
        ("David Wilson", "lender", "david.wilson@example.com", "approved"),
        ("Eve Brown", "investor", "eve.brown@example.com", "rejected"),
        ("Frank Miller", "lender", "frank.miller@example.com", "not_submitted"),
        ("Grace Lee", "investor", "grace.lee@example.com", "approved"),
        ("Henry Taylor", "lender", "henry.taylor@example.com", "approved"),
        ("Ivy Chen", "investor", "ivy.chen@example.com", "pending"),
        ("Jack Rodriguez", "lender", "jack.rodriguez@example.com", "approved"),
    ];
    
    for (name, user_type, email, kyc_status) in test_users {
        // Create the identity
        let mut identity = create_identity(pool, user_type, email).await?;
        
        // Update KYC status if not default
        if kyc_status != "not_submitted" {
            let now = Utc::now();
            sqlx::query!(
                r#"
                UPDATE identities 
                SET kyc_status = $2,
                    updated_at = $3
                WHERE id = $1
                "#,
                identity.id,
                kyc_status,
                now
            )
            .execute(pool)
            .await?;
            
            // Update our local copy
            identity.kyc_status = kyc_status.to_string();
            if kyc_status == "approved" {
                identity.kyc_submitted_at = Some(now);
                identity.kyc_verified_at = Some(now);
            } else if kyc_status != "not_submitted" {
                identity.kyc_submitted_at = Some(now);
            }
        }
        
        // Note: Balances are automatically created by database trigger
        
        identities.push(identity);
        println!("  👤 Created {} ({}) with KYC: {}", name, user_type, kyc_status);
    }
    
    Ok(identities)
}

/// Create test loans between identities
async fn create_test_loans(
    pool: &sqlx::PgPool, 
    identities: &[Identity]
) -> Result<Vec<Loan>, sqlx::Error> {
    let mut loans = Vec::new();
    
    // Filter approved identities for loan creation
    let approved_identities: Vec<&Identity> = identities
        .iter()
        .filter(|i| i.kyc_status == "approved")
        .collect();
    
    let investors: Vec<&Identity> = approved_identities
        .iter()
        .filter(|i| i.user_type == "investor")
        .copied()
        .collect();
        
    let lenders: Vec<&Identity> = approved_identities
        .iter()
        .filter(|i| i.user_type == "lender")
        .copied()
        .collect();
    
    // Create various loan scenarios
    let loan_scenarios = vec![
        // (amount, interest_rate_bps, status, days_ago)
        ("50000.00", 500, "active", 30),    // $500 loan, 5% APR
        ("100000.00", 750, "active", 45),   // $1000 loan, 7.5% APR
        ("25000.00", 400, "paid_off", 60),  // $250 loan, 4% APR, paid off
        ("75000.00", 650, "active", 15),    // $750 loan, 6.5% APR
        ("200000.00", 800, "originated", 5), // $2000 loan, 8% APR, new
        ("150000.00", 600, "active", 90),   // $1500 loan, 6% APR
        ("30000.00", 450, "closed", 120),   // $300 loan, 4.5% APR, closed
        ("80000.00", 700, "active", 20),    // $800 loan, 7% APR
    ];
    
    for (i, (amount_str, interest_rate_bps, status, days_ago)) in loan_scenarios.iter().enumerate() {
        if i >= investors.len() || i >= lenders.len() {
            break;
        }
        
        let borrower = investors[i];
        let lender = lenders[i % lenders.len()];
        let amount = Decimal::from_str(amount_str).map_err(|e| sqlx::Error::Decode(Box::new(e)))?;
        let originated_at = Utc::now() - chrono::Duration::days(*days_ago as i64);
        
        // Create loan
        let loan_id = Uuid::new_v4();
        let outstanding_balance = match *status {
            "paid_off" | "closed" => Decimal::ZERO,
            _ => amount * Decimal::from_str("0.8").map_err(|e| sqlx::Error::Decode(Box::new(e)))?, // 80% remaining
        };
        
        sqlx::query!(
            r#"
            INSERT INTO loans (id, borrower_id, lender_id, principal_amount, outstanding_balance, 
                             interest_rate_bps, loan_status, originated_at, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $8, $8)
            "#,
            loan_id,
            borrower.id,
            lender.id,
            amount,
            outstanding_balance,
            *interest_rate_bps as i32,
            status,
            originated_at
        )
        .execute(pool)
        .await?;
        
        let loan = Loan {
            id: loan_id,
            borrower_id: borrower.id,
            lender_id: lender.id,
            principal_amount: amount,
            outstanding_balance,
            interest_rate_bps: *interest_rate_bps as i32,
            loan_status: status.to_string(),
            originated_at,
            maturity_date: Some(originated_at + chrono::Duration::days(365)), // 1 year term
            created_at: originated_at,
            updated_at: originated_at,
        };
        
        loans.push(loan);
        println!(
            "  💰 Created ${} loan: {} -> {} ({})", 
            amount, 
            borrower.email.split('@').next().unwrap_or("unknown"),
            lender.email.split('@').next().unwrap_or("unknown"),
            status
        );
    }
    
    Ok(loans)
}

/// Create test payments for loans
async fn create_test_payments(
    pool: &sqlx::PgPool,
    loans: &[Loan], 
    identities: &[Identity]
) -> Result<Vec<Payment>, sqlx::Error> {
    let mut payments = Vec::new();
    
    for loan in loans {
        // Skip loans that are originated or paid off
        if loan.loan_status == "originated" || loan.loan_status == "paid_off" || loan.loan_status == "closed" {
            continue;
        }
        
        // Find the borrower
        let borrower = identities.iter().find(|i| i.id == loan.borrower_id);
        if borrower.is_none() {
            continue;
        }
        let borrower = borrower.unwrap();
        
        // Create 2-4 payments for each active loan
        let num_payments = 2 + (loan.id.as_u128() % 3) as usize; // 2-4 payments
        let payment_amount = loan.principal_amount / Decimal::from(num_payments * 2); // Smaller payments
        
        for i in 0..num_payments {
            let days_ago = 10 + (i * 15); // Spread payments over time
            let processed_at = Utc::now() - chrono::Duration::days(days_ago as i64);
            
            let payment_id = Uuid::new_v4();
            let payment_type = match i % 4 {
                0 => "principal",
                1 => "interest", 
                2 => "regular",
                _ => "penalty",
            };
            
            sqlx::query!(
                r#"
                INSERT INTO payments (id, loan_id, payer_id, amount, payment_type, 
                                    processed_at, created_at)
                VALUES ($1, $2, $3, $4, $5, $6, $6)
                "#,
                payment_id,
                loan.id,
                borrower.id,
                payment_amount,
                payment_type,
                processed_at
            )
            .execute(pool)
            .await?;
            
            let payment = Payment {
                id: payment_id,
                loan_id: loan.id,
                payer_id: borrower.id,
                amount: payment_amount,
                payment_type: payment_type.to_string(),
                transaction_hash: None,
                processed_at,
                created_at: processed_at,
            };
            
            payments.push(payment);
        }
        
        println!(
            "  💳 Created {} payments for loan {} (${} each)",
            num_payments, 
            loan.id.to_string()[0..8].to_string(),
            payment_amount
        );
    }
    
    Ok(payments)
}


/// Print summary of created test data
async fn print_summary(identities: &[Identity], loans: &[Loan], payments: &[Payment]) {
    println!("\n📊 Test Data Summary:");
    println!("=====================");
    
    // Identity summary
    let mut kyc_counts = std::collections::HashMap::new();
    let mut type_counts = std::collections::HashMap::new();
    
    for identity in identities {
        *kyc_counts.entry(identity.kyc_status.clone()).or_insert(0) += 1;
        *type_counts.entry(identity.user_type.clone()).or_insert(0) += 1;
    }
    
    println!("👥 Identities: {}", identities.len());
    for (user_type, count) in type_counts {
        println!("   - {}: {}", user_type, count);
    }
    println!("🔐 KYC Status:");
    for (status, count) in kyc_counts {
        println!("   - {}: {}", status, count);
    }
    
    // Loan summary
    let mut loan_status_counts = std::collections::HashMap::new();
    let mut total_principal = Decimal::ZERO;
    let mut total_outstanding = Decimal::ZERO;
    
    for loan in loans {
        *loan_status_counts.entry(loan.loan_status.clone()).or_insert(0) += 1;
        total_principal += loan.principal_amount;
        total_outstanding += loan.outstanding_balance;
    }
    
    println!("💰 Loans: {}", loans.len());
    println!("   - Total Principal: ${}", total_principal);
    println!("   - Total Outstanding: ${}", total_outstanding);
    println!("📈 Loan Status:");
    for (status, count) in loan_status_counts {
        println!("   - {}: {}", status, count);
    }
    
    // Payment summary
    let mut payment_type_counts = std::collections::HashMap::new();
    let mut total_payments = Decimal::ZERO;
    
    for payment in payments {
        *payment_type_counts.entry(payment.payment_type.clone()).or_insert(0) += 1;
        total_payments += payment.amount;
    }
    
    println!("💳 Payments: {}", payments.len());
    println!("   - Total Payment Amount: ${}", total_payments);
    println!("📊 Payment Types:");
    for (payment_type, count) in payment_type_counts {
        println!("   - {}: {}", payment_type, count);
    }
    
    println!("\n🌐 API Testing URLs:");
    println!("GET http://localhost:8080/api/v2/ccr/identities");
    println!("GET http://localhost:8080/api/v2/ccr/loans");
    println!("GET http://localhost:8080/api/v2/ccr/loans/stats");
    println!("POST http://localhost:8080/api/v2/ccr/identities (to create new user)");
}