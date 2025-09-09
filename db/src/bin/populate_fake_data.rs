use chrono::Utc;
use db::{
    create_credit_line_request, create_identity_with_password, create_investor_profile,
    create_liquidity_pool, create_merchant, create_merchant_profile, create_usdc_deposit,
    submit_kyc_data, update_identity_profile, CreateCreditLineRequestData,
    CreateInvestorProfileRequest, CreateLiquidityPoolRequest, CreateMerchantProfileRequest,
    CreateMerchantRequest,
};
use rand::Rng;
use rust_decimal::Decimal;
use sqlx::PgPool;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();

    // Connect to database
    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgresql://postgres:password@127.0.0.1:5432/arda_credit_db".to_string()
    });

    println!("🔌 Connecting to database...");
    let pool = PgPool::connect(&database_url).await?;

    // Run migrations to ensure schema is up to date
    println!("🔧 Running migrations...");
    sqlx::migrate!("./migrations").run(&pool).await?;

    println!("🎲 Starting fake data population based on PERFORMANCE.md...");

    // Based on PERFORMANCE.md:
    // - 100 new users per day (U=100)
    // - 10 new loans per day (L=10)
    // Mix of investors, lenders, and merchants

    let mut rng = rand::rng();

    // Create 200 users total (mix of investors, lenders, merchants)
    println!("👥 Creating users...");
    let mut user_ids = Vec::new();
    let mut investor_ids = Vec::new();
    let mut merchant_ids = Vec::new();

    for i in 0..200 {
        let user_type = match i % 3 {
            0 => "investor",
            1 => "lender",
            _ => "merchant",
        };

        let email = format!("user{}{}@example.com", i, Utc::now().timestamp());
        let user =
            create_identity_with_password(&pool, user_type, &email, "hashed_password_123").await?;

        // Submit KYC for most users (85% approval rate)
        if rng.random_bool(0.85) {
            submit_kyc_data(
                &pool,
                user.id,
                "passport",
                &format!("doc_hash_{}", i),
                Some("US"),
                Some("AI"),
            )
            .await?;
        }

        // Update user profile with realistic data
        let first_names = vec![
            "John", "Jane", "Michael", "Sarah", "David", "Lisa", "Robert", "Emily", "James",
            "Jessica",
        ];
        let last_names = vec![
            "Smith",
            "Johnson",
            "Williams",
            "Brown",
            "Jones",
            "Garcia",
            "Miller",
            "Davis",
            "Rodriguez",
            "Martinez",
        ];

        update_identity_profile(
            &pool,
            user.id,
            None,       // email stays same
            None,       // user_type stays same
            Some("US"), // jurisdiction
            Some(first_names[rng.random_range(0..first_names.len())]),
            Some(last_names[rng.random_range(0..last_names.len())]),
            Some(&format!("0x{:040x}", rng.random::<u128>())), // fake wallet
        )
        .await?;

        user_ids.push(user.id);

        match user_type {
            "investor" => {
                investor_ids.push(user.id);

                // Create investor profile
                let profile_request = CreateInvestorProfileRequest {
                    accredited_investor: Some(true),
                    investment_capacity: Some(Decimal::from(rng.random_range(10000..1000000))),
                    regulatory_jurisdiction: Some("US".to_string()),
                };
                create_investor_profile(&pool, user.id, &profile_request).await?;
            }
            "merchant" => {
                // Create merchant
                let merchant_request = CreateMerchantRequest {
                    legal_name: format!("Business LLC {}", i),
                    dba: Some(format!("Store {}", i)),
                    tax_id: Some(format!("12-{}{}", 1000000 + i, Utc::now().timestamp())),
                    mcc: Some("5411".to_string()), // Grocery stores
                    country: "US".to_string(),
                    business_type: Some("LLC".to_string()),
                };

                let merchant = create_merchant(&pool, user.id, &merchant_request).await?;
                merchant_ids.push(merchant.id);

                // Create merchant profile
                let profile_request = CreateMerchantProfileRequest {
                    industry: Some("Retail".to_string()),
                    years_in_business: Some(rng.random_range(1..20)),
                    employee_count: Some(rng.random_range(1..50)),
                    monthly_revenue_estimate: Some(Decimal::from(rng.random_range(5000..100000))),
                    business_description: Some(
                        "Retail business specializing in consumer goods".to_string(),
                    ),
                };
                create_merchant_profile(&pool, merchant.id, &profile_request).await?;
            }
            _ => {} // lender - no additional setup needed
        }
    }

    println!(
        "✅ Created {} users ({} investors, {} merchants)",
        user_ids.len(),
        investor_ids.len(),
        merchant_ids.len()
    );

    // Create liquidity pools (based on investors)
    println!("💰 Creating liquidity pools...");
    let mut pool_ids = Vec::new();

    for (i, &investor_id) in investor_ids.iter().enumerate() {
        if i < 20 {
            // Create 20 pools
            let pool_request = CreateLiquidityPoolRequest {
                investor_id,
                name: format!("Credit Pool {}", i + 1),
                max_ltv_ratio: Decimal::new(rng.random_range(5000..8000), 4), // 0.5000 to 0.8000
                max_interest_rate: Decimal::new(rng.random_range(800..1500), 4), // 0.0800 to 0.1500
                repayment_period_days: rng.random_range(30..365),
            };

            let pool = create_liquidity_pool(&pool, &pool_request).await?;
            pool_ids.push(pool.id);
        }
    }

    println!("✅ Created {} liquidity pools", pool_ids.len());

    // Create USDC deposits for users
    println!("💵 Creating USDC deposits...");
    for (i, &user_id) in user_ids.iter().enumerate() {
        if i < 100 {
            // First 100 users get deposits
            let deposit_amount = rng.random_range(1000..10000); // Smaller range to avoid overflow

            create_usdc_deposit(
                &pool,
                &format!("0x{:040x}", user_id.as_u128()),
                Decimal::from(deposit_amount) * Decimal::from(1_000_000), // Convert to wei (6 decimals for USDC)
                &format!("0x{:064x}", rng.random::<u64>()),
                &format!("0x{:064x}", rng.random::<u64>()),
                i64::from(rng.random_range(1000000u32..2000000u32)),
                i64::from(rng.random_range(1000000u32..2000000u32)),
                i64::from(rng.random_range(0..1000)),
            )
            .await?;
        }
    }

    println!("✅ Created deposits for 100 users");

    // Create credit line requests from merchants
    println!("📋 Creating credit line requests...");
    let mut credit_requests = Vec::new();

    for (i, &merchant_id) in merchant_ids.iter().enumerate() {
        if i < 30 {
            // Create 30 credit requests
            let request_data = CreateCreditLineRequestData {
                merchant_id,
                requested_amount: Decimal::from(rng.random_range(10000..200000)), // $10k-$200k
                requested_rate_apr: Some(Decimal::new(rng.random_range(800..1200), 4)), // 0.0800-0.1200
                requested_tenor_days: rng.random_range(30..365),
                request_description: Some(format!(
                    "Credit line request for business expansion {}",
                    i + 1
                )),
            };

            let credit_request = create_credit_line_request(&pool, request_data).await?;
            credit_requests.push(credit_request.id);
        }
    }

    println!("✅ Created {} credit line requests", credit_requests.len());

    // Summary statistics
    println!("\n📊 DATA POPULATION SUMMARY");
    println!("═══════════════════════════");
    println!("👥 Users: {}", user_ids.len());
    println!("   - Investors: {}", investor_ids.len());
    println!("   - Merchants: {}", merchant_ids.len());
    println!(
        "   - Lenders: {}",
        user_ids.len() - investor_ids.len() - merchant_ids.len()
    );
    println!("💰 Liquidity Pools: {}", pool_ids.len());
    println!("💵 USDC Deposits: 100");
    println!("📋 Credit Requests: {}", credit_requests.len());

    println!("\n✨ Fake data population complete!");
    println!("🚀 Ready for frontend testing with realistic data volumes");

    Ok(())
}
