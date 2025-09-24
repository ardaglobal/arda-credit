use chrono::Utc;
use db::{
    create_credit_line, create_credit_line_request, create_identity_with_password,
    create_investor_profile, create_liquidity_pool, create_usdc_deposit,
    find_matching_liquidity_pools, set_pool_borough_limit, submit_kyc_data,
    update_borough_allocation, update_credit_line_request_status, update_identity_profile,
    CreateCreditLineRequestData, CreateInvestorProfileRequest, CreateLiquidityPoolRequest,
    CreditLineStatus, PoolMatchingCriteria, SetPoolBoroughLimitRequest,
};
use rand::Rng;
use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
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

    println!("🎲 Starting fake data population");

    // Based on docs/architecture/performance-targets.md:
    // - 100 new users per day (U=100)
    // - 10 new loans per day (L=10)
    // Mix of investors and originators

    let mut rng = rand::rng();

    // Create 200 users total (mix of investors and originators)
    println!("👥 Creating users...");
    let mut user_ids = Vec::new();
    let mut investor_ids = Vec::new();
    let mut originator_ids: Vec<uuid::Uuid> = Vec::new();

    for i in 0..200 {
        let user_type = match i % 3 {
            0 => "investor",
            1 => "investor",
            _ => "originator",
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
            "originator" => {
                // Originators are loan creators - no special profile setup needed for test data
                // They can create loans directly
                originator_ids.push(user.id);
                println!("Created originator: user {}", user.id);
            }
            _ => {} // lender - no additional setup needed
        }
    }

    println!(
        "✅ Created {} users ({} investors, {} originators)",
        user_ids.len(),
        investor_ids.len(),
        originator_ids.len()
    );

    // Create liquidity pools (based on investors)
    println!("💰 Creating liquidity pools...");
    let mut pool_ids = Vec::new();

    for (i, &investor_id) in investor_ids.iter().enumerate() {
        if i < 20 {
            // Create 20 pools with varied characteristics
            let pool_names = vec![
                "Conservative Credit Fund",
                "Growth Lending Pool",
                "High-Yield Credit",
                "Regional Development Fund",
                "Small Business Credit",
                "Rapid Deploy Pool",
                "Diversified Credit",
                "Premium Lending",
                "Community Development",
                "Enterprise Credit Pool",
                "Emerging Markets",
                "Stable Returns Fund",
                "Manhattan Focus Pool",
                "Brooklyn Community Fund",
                "Queens Development",
                "Bronx Growth Pool",
                "Staten Island Credit",
                "Multi-Borough Fund",
                "Metro Area Pool",
                "NYC Infrastructure",
            ];

            let pool_name = pool_names
                .get(i)
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("Credit Pool {}", i + 1));
            let pool_request = CreateLiquidityPoolRequest {
                investor_id,
                name: pool_name.clone(),
                max_ltv_ratio: Decimal::new(rng.random_range(5000..8500), 4), // 0.5000 to 0.8500
                max_interest_rate: Decimal::new(rng.random_range(800..1500), 4), // 0.0800 to 0.1500
                repayment_period_days: rng.random_range(30..365),
            };

            let created_pool = create_liquidity_pool(&pool, &pool_request).await?;
            pool_ids.push(created_pool.id);

            // Fund the pool with realistic amounts ($1M to $10M)
            let total_funding =
                Decimal::from(rng.random_range(1000000..10000000)) * Decimal::from(100); // Convert to cents
            let utilized_amount = total_funding * Decimal::new(rng.random_range(0..60), 2); // 0% to 60% utilized
            let available_amount = total_funding - utilized_amount;

            // Update pool balances
            sqlx::query!(
                "UPDATE liquidity_pools SET total_deposits = $1, utilized_liquidity = $2, available_liquidity = $3 WHERE id = $4",
                total_funding,
                utilized_amount,
                available_amount,
                created_pool.id
            ).execute(&pool).await?;

            println!(
                "Created funded pool: {} (${:.0}M total, ${:.0}M available)",
                pool_name,
                total_funding.to_f64().unwrap_or(0.0) / 100_000_000.0, // Convert from cents to millions
                available_amount.to_f64().unwrap_or(0.0) / 100_000_000.0
            );
        }
    }

    println!("✅ Created {} liquidity pools", pool_ids.len());

    // Add borough percentage limits to some pools for testing
    println!("🗺️ Adding borough percentage limits to pools...");
    let boroughs_with_limits = vec!["Manhattan", "Brooklyn", "Queens", "Bronx"];

    for (i, &pool_id) in pool_ids.iter().enumerate() {
        if i < 10 {
            // Add limits to first 10 pools
            for borough in &boroughs_with_limits {
                let percentage_limit = match borough {
                    &"Manhattan" => 0.25, // 25% - higher demand area
                    &"Brooklyn" => 0.20,  // 20%
                    &"Queens" => 0.20,    // 20%
                    &"Bronx" => 0.15,     // 15% - more conservative
                    _ => 0.10,            // 10% default
                };

                let limit_request = SetPoolBoroughLimitRequest {
                    pool_id,
                    borough: borough.to_string(),
                    max_percentage: Decimal::from_f64(percentage_limit)
                        .unwrap_or(Decimal::new(10, 2)),
                };

                set_pool_borough_limit(&pool, limit_request).await?;
                println!(
                    "Set {}% borough limit for {} in pool {}",
                    (percentage_limit * 100.0) as u8,
                    borough,
                    i + 1
                );
            }
        }
    }

    println!(
        "✅ Added borough limits to {} pools",
        std::cmp::min(10, pool_ids.len())
    );

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

    // Get merchant data for more realistic credit requests
    let merchants_data = sqlx::query!(
        "SELECT m.id, m.legal_name, m.borough, mp.monthly_revenue_estimate, mp.industry
         FROM merchants m
         JOIN merchant_profiles mp ON m.id = mp.merchant_id
         LIMIT 50"
    )
    .fetch_all(&pool)
    .await?;

    let request_purposes = vec![
        "Working capital for seasonal inventory buildup",
        "Equipment purchase and facility expansion",
        "Bridge financing for new market expansion",
        "Cash flow support during growth phase",
        "Technology upgrade and digital transformation",
        "Supply chain optimization and inventory management",
        "Real estate acquisition for business expansion",
        "Marketing campaign and customer acquisition",
        "Debt consolidation and financial restructuring",
        "Emergency capital for unexpected opportunities",
    ];

    for (i, merchant) in merchants_data.iter().enumerate() {
        if i < 40 {
            // Create 40 credit requests from different merchants
            // Calculate reasonable credit amount based on revenue (2-6x monthly revenue)
            let monthly_revenue = merchant
                .monthly_revenue_estimate
                .unwrap_or(Decimal::from(20000));
            let credit_multiplier = Decimal::new(rng.random_range(200..600), 2); // 2.0 to 6.0x
            let base_amount = monthly_revenue * credit_multiplier;
            let requested_amount = (base_amount / Decimal::from(100)).round() * Decimal::from(100); // Round to nearest $100

            let request_data = CreateCreditLineRequestData {
                merchant_id: merchant.id,
                requested_amount,
                requested_rate_apr: Some(Decimal::new(rng.random_range(800..1400), 4)), // 0.0800-0.1400
                requested_tenor_days: rng.random_range(30..365),
                request_description: Some(format!(
                    "{} for {} business in {}. Monthly revenue: ${:.0}",
                    request_purposes[i % request_purposes.len()],
                    merchant
                        .industry
                        .as_ref()
                        .unwrap_or(&"General".to_string())
                        .to_lowercase(),
                    merchant.borough.as_ref().unwrap_or(&"Unknown".to_string()),
                    monthly_revenue.to_f64().unwrap_or(0.0)
                )),
            };

            let credit_request = create_credit_line_request(&pool, request_data.clone()).await?;
            credit_requests.push(credit_request.id);

            // Auto-deploy the credit line by finding a matching pool
            let criteria = PoolMatchingCriteria {
                required_liquidity: requested_amount,
                max_ltv: Decimal::new(8000, 4), // 80%
                max_rate: request_data
                    .requested_rate_apr
                    .unwrap_or(Decimal::new(1500, 4)), // 15%
                min_repayment_days: request_data.requested_tenor_days,
            };

            match find_matching_liquidity_pools(&pool, &criteria).await {
                Ok(matching_pools) if !matching_pools.is_empty() => {
                    let selected_pool = &matching_pools[0];

                    // Create active credit line
                    match create_credit_line(
                        &pool,
                        credit_request.id,
                        merchant.id,
                        selected_pool.id,
                        requested_amount,
                        request_data
                            .requested_rate_apr
                            .unwrap_or(Decimal::new(1200, 4)), // 12%
                        request_data.requested_tenor_days,
                    )
                    .await
                    {
                        Ok(credit_line) => {
                            // Update borough allocation if merchant has borough
                            if let Some(ref merchant_borough) = merchant.borough {
                                let _ = update_borough_allocation(
                                    &pool,
                                    selected_pool.id,
                                    merchant_borough,
                                    requested_amount,
                                )
                                .await;
                            }

                            // Update request status to APPROVED
                            let _ = update_credit_line_request_status(
                                &pool,
                                credit_request.id,
                                CreditLineStatus::Approved,
                                None,
                            )
                            .await;

                            println!(
                                "✅ Auto-deployed credit line: {} ${:.0}k for {} ({}) → Pool: {}",
                                credit_line.id,
                                requested_amount.to_f64().unwrap_or(0.0) / 1000.0,
                                merchant.legal_name,
                                merchant.borough.as_ref().unwrap_or(&"Unknown".to_string()),
                                selected_pool.name
                            );
                        }
                        Err(e) => {
                            println!(
                                "⚠️  Failed to auto-deploy credit for {}: {}",
                                merchant.legal_name, e
                            );
                        }
                    }
                }
                Ok(_) => {
                    println!(
                        "⚠️  No matching pools for ${:.0}k request from {} ({})",
                        requested_amount.to_f64().unwrap_or(0.0) / 1000.0,
                        merchant.legal_name,
                        merchant.borough.as_ref().unwrap_or(&"Unknown".to_string())
                    );
                }
                Err(e) => {
                    println!(
                        "⚠️  Pool matching failed for {}: {}",
                        merchant.legal_name, e
                    );
                }
            }
        }
    }

    println!(
        "✅ Created {} credit line requests with auto-deployment",
        credit_requests.len()
    );

    // Summary statistics
    println!("\n📊 DATA POPULATION SUMMARY");
    println!("═══════════════════════════");
    println!("👥 Users: {}", user_ids.len());
    println!("   - Investors: {}", investor_ids.len());
    println!("   - Originators: {}", originator_ids.len());
    println!(
        "   - Admins: {}",
        user_ids.len() - investor_ids.len() - originator_ids.len()
    );
    println!("💰 Liquidity Pools: {}", pool_ids.len());
    println!(
        "   - With borough limits: {}",
        std::cmp::min(10, pool_ids.len())
    );
    println!("   - Funding range: $1M - $10M each");
    println!("💵 USDC Deposits: 100 users");
    println!(
        "📋 Credit Requests: {} (with auto-deployment)",
        credit_requests.len()
    );
    println!("🗺️ Borough Coverage:");
    println!("   - Manhattan, Brooklyn, Queens, Bronx");
    println!("   - Staten Island, Long Island, Westchester");
    println!("   - New Jersey, Connecticut");

    // Borough-specific statistics
    let borough_stats = sqlx::query!(
        "SELECT m.borough, COUNT(*) as count, AVG(mp.monthly_revenue_estimate) as avg_revenue
         FROM merchants m
         JOIN merchant_profiles mp ON m.id = mp.merchant_id
         WHERE m.borough IS NOT NULL
         GROUP BY m.borough
         ORDER BY count DESC"
    )
    .fetch_all(&pool)
    .await?;

    println!("\n📍 MERCHANTS BY BOROUGH:");
    for stat in borough_stats {
        println!(
            "   {} - {} merchants (avg revenue: ${:.0}/month)",
            stat.borough.as_ref().unwrap_or(&"Unknown".to_string()),
            stat.count.unwrap_or(0),
            stat.avg_revenue
                .unwrap_or(Decimal::ZERO)
                .to_f64()
                .unwrap_or(0.0)
        );
    }

    // Pool utilization statistics
    let pool_stats = sqlx::query!(
        "SELECT
            COUNT(*) as total_pools,
            SUM(total_deposits) as total_funding,
            SUM(available_liquidity) as total_available,
            SUM(utilized_liquidity) as total_utilized,
            AVG(utilized_liquidity::DECIMAL / NULLIF(total_deposits::DECIMAL, 0) * 100) as avg_utilization
         FROM liquidity_pools"
    ).fetch_one(&pool).await?;

    println!("\n💼 POOL UTILIZATION:");
    println!(
        "   Total funding: ${:.1}M",
        pool_stats
            .total_funding
            .unwrap_or(Decimal::ZERO)
            .to_f64()
            .unwrap_or(0.0)
            / 100_000_000.0
    );
    println!(
        "   Available liquidity: ${:.1}M",
        pool_stats
            .total_available
            .unwrap_or(Decimal::ZERO)
            .to_f64()
            .unwrap_or(0.0)
            / 100_000_000.0
    );
    println!(
        "   Utilized liquidity: ${:.1}M",
        pool_stats
            .total_utilized
            .unwrap_or(Decimal::ZERO)
            .to_f64()
            .unwrap_or(0.0)
            / 100_000_000.0
    );
    println!(
        "   Average utilization: {:.1}%",
        pool_stats
            .avg_utilization
            .unwrap_or(Decimal::ZERO)
            .to_f64()
            .unwrap_or(0.0)
    );

    println!("\n✨ Enhanced fake data population complete!");
    println!("🚀 Ready for borough percentage limit testing!");
    println!("🔬 Test scenarios include:");
    println!("   - Credit requests from merchants in different boroughs");
    println!("   - Pools with varying borough percentage limits");
    println!("   - Realistic funding amounts and utilization rates");
    println!("   - Borough-based risk diversification testing");

    Ok(())
}
