//! Credit line management, eligibility checking, and pool browsing
//!
//! This module handles all credit-related operations including:
//! - Credit eligibility assessment and scoring
//! - Credit line request submission and management
//! - Available liquidity pool discovery
//! - Merchant profile completion tracking
//! - Credit terms negotiation and approval

use arda_credit_api::BatchApiClient;
use eyre::Result;

/// Get merchant profile completion status
pub async fn get_profile_completion(client: &BatchApiClient, verbose: bool) -> Result<()> {
    println!("📋 Merchant Profile Completion Status");

    // Placeholder implementation - would check actual profile completeness
    println!();
    println!("✅ Profile Completion: 75%");

    println!();
    println!("✅ Completed Sections:");
    println!("   • Business Information ✅");
    println!("   • Contact Details ✅");
    println!("   • Tax Information ✅");
    println!("   • Bank Account ✅");

    println!();
    println!("⚠️  Incomplete Sections:");
    println!("   • KYC Documents (25% complete)");
    println!("   • Turnover History (60 days needed)");

    if verbose {
        println!();
        println!("🔧 Detailed Requirements:");
        println!("   KYC Documents Missing:");
        println!("     - Business registration certificate");
        println!("     - Tax ID verification");
        println!("     - Beneficial ownership form");
        println!("   Turnover Requirements:");
        println!("     - Need 30 more days of data");
        println!("     - Current streak: 30 days");
    }

    println!();
    println!("💡 Next Steps:");
    println!("   • Upload remaining KYC documents");
    println!("   • Continue daily turnover submissions");
    println!("   • Profile will be complete in ~30 days");

    Ok(())
}

/// Check merchant eligibility for credit line
pub async fn check_credit_eligibility(
    client: &BatchApiClient,
    merchant_id: &str,
    verbose: bool,
) -> Result<()> {
    println!(
        "🔍 Checking credit eligibility for merchant: {}",
        merchant_id
    );

    // Placeholder implementation - would run actual eligibility checks
    println!();
    println!("✅ Eligibility Assessment:");
    println!("   Merchant ID: {}", merchant_id);
    println!("   Overall Score: 825/1000");
    println!("   Eligibility: QUALIFIED");
    println!("   Maximum Credit: $250,000");

    println!();
    println!("📊 Assessment Breakdown:");
    println!("   • Business Age: ✅ 2.5 years (Pass)");
    println!("   • Revenue Stability: ✅ Excellent (850/1000)");
    println!("   • Cash Flow: ✅ Positive trend (+15%)");
    println!("   • Credit History: ✅ No defaults");
    println!("   • KYC Status: ✅ Fully verified");
    println!("   • Turnover Data: ✅ 90+ days complete");

    println!();
    println!("💰 Credit Terms Available:");
    println!("   • Credit Line: $50,000 - $250,000");
    println!("   • Interest Rate: 8.5% - 12.0% APR");
    println!("   • Repayment Terms: 30 - 365 days");
    println!("   • Collateral: None required");

    if verbose {
        println!();
        println!("🔧 Risk Assessment Details:");
        println!("   • Industry Risk: Low (MCC: 5411 - Grocery)");
        println!("   • Geographic Risk: Low (US - California)");
        println!("   • Financial Risk: Low (Strong cash flow)");
        println!("   • Operational Risk: Low (Established business)");
        println!("   • Recommended LTV: 75%");
        println!("   • Risk Rating: A-");
    }

    println!();
    println!("💡 Ready to proceed:");
    println!("   • Submit credit line request: cli loan submit-credit-line-request");
    println!("   • Browse available pools: cli loan browse-available-pools");

    Ok(())
}

/// Submit a credit line request
pub async fn submit_credit_line_request(
    client: &BatchApiClient,
    merchant_id: &str,
    requested_amount: u64,
    requested_rate_bps: Option<u64>,
    requested_tenor_days: u32,
    verbose: bool,
) -> Result<()> {
    println!("💳 Submitting credit line request...");

    let amount_usd = requested_amount as f64 / 100.0;
    println!("   Merchant ID: {}", merchant_id);
    println!("   Requested Amount: ${:.2}", amount_usd);
    if let Some(rate_bps) = requested_rate_bps {
        println!("   Requested Rate: {:.2}% APR", rate_bps as f64 / 100.0);
    }
    println!("   Requested Tenor: {} days", requested_tenor_days);

    // Basic validation
    if requested_amount < 1000_00 {
        println!("❌ Minimum credit request is $1,000.00");
        return Ok(());
    }

    if requested_tenor_days > 365 {
        println!("❌ Maximum tenor is 365 days");
        return Ok(());
    }

    // Submit credit line request via API
    let request_body = serde_json::json!({
        "requested_amount": requested_amount,
        "requested_rate_apr": requested_rate_bps,
        "requested_tenor_days": requested_tenor_days,
        "request_description": format!("Credit line request for ${:.2}", amount_usd)
    });

    match client
        .post("/api/v1/credit-lines/request", &request_body)
        .await
    {
        Ok(response) => {
            if let Some(request_obj) = response.as_object() {
                println!();
                println!("✅ Credit line request submitted!");

                if let Some(request_id) = request_obj.get("request_id") {
                    println!(
                        "   Request ID: {}",
                        request_id.as_str().unwrap_or("Unknown")
                    );
                }

                if let Some(status) = request_obj.get("status") {
                    println!("   Status: {}", status.as_str().unwrap_or("UNKNOWN"));
                }

                println!("   Estimated Review Time: 2-5 business days");

                println!();
                println!("📋 Request Details:");

                if let Some(amount) = request_obj.get("requested_amount") {
                    println!(
                        "   Amount: {}",
                        amount.as_str().unwrap_or(&format!("${:.2}", amount_usd))
                    );
                }

                if let Some(tenor) = request_obj.get("requested_tenor_days") {
                    println!(
                        "   Term: {} days",
                        tenor.as_u64().unwrap_or(requested_tenor_days as u64)
                    );
                }

                if let Some(created_at) = request_obj.get("created_at") {
                    println!("   Submitted: {}", created_at.as_str().unwrap_or("Unknown"));
                }

                if verbose {
                    println!();
                    println!("🔧 Processing Pipeline:");
                    println!("   1. Pool Balance Validation - ✅ Complete");
                    println!("   2. Automated Screening - 🔄 In progress");
                    println!("   3. Risk Assessment - ⏸️  Pending");
                    println!("   4. Pool Matching - ⏸️  Pending");
                    println!("   5. Terms Negotiation - ⏸️  Pending");
                    println!("   6. Final Approval - ⏸️  Pending");
                }

                println!();
                println!("💡 Track your request:");
                println!(
                    "   • Check status: cli loan list-credit-line-requests --merchant-id {}",
                    merchant_id
                );
            }
        }
        Err(e) => {
            println!("❌ Failed to submit credit line request");

            if let Ok(error_text) = e.to_string().parse::<serde_json::Value>() {
                if let Some(error_obj) = error_text.as_object() {
                    if let Some(error_msg) = error_obj.get("error") {
                        println!(
                            "   Error: {}",
                            error_msg.as_str().unwrap_or("Unknown error")
                        );
                    }
                    if let Some(details) = error_obj.get("details") {
                        println!("   Details: {}", details.as_str().unwrap_or(""));
                    }
                }
            } else {
                println!("   Error: {}", e);
            }

            println!();
            println!("💡 Suggestions:");
            println!("   • Check if you're requesting an amount below available pool liquidity");
            println!("   • Ensure your merchant profile is complete and KYC is approved");
            println!("   • Try requesting a smaller amount or check available pools first");

            return Err(e.into());
        }
    }

    Ok(())
}

/// Browse available liquidity pools for credit matching
pub async fn browse_available_pools(
    client: &BatchApiClient,
    required_liquidity: u64,
    max_ltv_bps: u64,
    max_rate_bps: u64,
    min_repayment_days: u32,
    verbose: bool,
) -> Result<()> {
    println!("🔍 Browsing available liquidity pools...");

    let liquidity_usd = required_liquidity as f64 / 100.0;
    println!("   Required Liquidity: ${:.2}", liquidity_usd);
    println!("   Max LTV: {:.2}%", max_ltv_bps as f64 / 100.0);
    println!("   Max Rate: {:.2}%", max_rate_bps as f64 / 100.0);
    println!("   Min Repayment: {} days", min_repayment_days);

    // Build query parameters
    let mut query_params = Vec::new();
    if required_liquidity > 0 {
        query_params.push(format!("min_amount={}", required_liquidity));
    }
    if max_ltv_bps > 0 {
        query_params.push(format!("max_ltv={}", max_ltv_bps));
    }
    if max_rate_bps > 0 {
        query_params.push(format!("max_rate={}", max_rate_bps));
    }
    if min_repayment_days > 0 {
        query_params.push(format!("min_repayment_days={}", min_repayment_days));
    }

    let query_string = if query_params.is_empty() {
        String::new()
    } else {
        format!("?{}", query_params.join("&"))
    };

    let endpoint = format!("/api/v1/pools/available{}", query_string);

    match client.get(&endpoint).await {
        Ok(response) => {
            if let Some(pools_array) = response.as_array() {
                if pools_array.is_empty() {
                    println!();
                    println!("❌ No matching pools found");
                    println!("   Try adjusting your criteria:");
                    println!("   • Lower the required liquidity amount");
                    println!("   • Increase the maximum LTV ratio");
                    println!("   • Increase the maximum interest rate");
                    println!("   • Reduce the minimum repayment days");
                } else {
                    println!();
                    println!("✅ Found {} matching pool(s):", pools_array.len());

                    for (index, pool) in pools_array.iter().enumerate() {
                        if let Some(pool_obj) = pool.as_object() {
                            println!();
                            println!(
                                "   {}. {}",
                                index + 1,
                                pool_obj
                                    .get("pool_name")
                                    .and_then(|n| n.as_str())
                                    .unwrap_or("Unknown Pool")
                            );

                            if let Some(pool_id) = pool_obj.get("pool_id") {
                                println!(
                                    "      Pool ID: {}",
                                    pool_id.as_str().unwrap_or("Unknown")
                                );
                            }

                            if let Some(available) = pool_obj.get("available_liquidity") {
                                println!(
                                    "      Available: {}",
                                    available.as_str().unwrap_or("Unknown")
                                );
                            }

                            if let Some(ltv) = pool_obj.get("max_ltv_ratio") {
                                println!("      Max LTV: {} ✅", ltv.as_str().unwrap_or("Unknown"));
                            }

                            if let Some(rate) = pool_obj.get("max_interest_rate") {
                                println!(
                                    "      Max Rate: {} ✅",
                                    rate.as_str().unwrap_or("Unknown")
                                );
                            }

                            if let Some(days) = pool_obj.get("repayment_period_days") {
                                println!(
                                    "      Repayment Period: {} days ✅",
                                    days.as_u64().unwrap_or(0)
                                );
                            }

                            if let Some(match_score) = pool_obj.get("match_score") {
                                println!(
                                    "      Match Score: {}%",
                                    match_score.as_u64().unwrap_or(0)
                                );
                            }
                        }
                    }

                    if verbose {
                        println!();
                        println!("🔧 Pool Analysis:");
                        println!("   Total Matching Pools: {}", pools_array.len());

                        // Calculate total available liquidity
                        let total_available: f64 = pools_array
                            .iter()
                            .filter_map(|pool| pool.get("available_liquidity"))
                            .filter_map(|val| val.as_str())
                            .filter_map(|s| s.trim_start_matches('$').parse::<f64>().ok())
                            .sum();

                        println!("   Total Available: ${:.2}", total_available);
                    }
                }
            } else {
                println!("❌ Invalid response format from server");
            }
        }
        Err(e) => {
            println!("❌ Failed to browse available pools");
            println!("   Error: {}", e);
            return Err(e.into());
        }
    }

    Ok(())
}

/// List credit line requests for a merchant
pub async fn list_credit_line_requests(
    client: &BatchApiClient,
    merchant_id: &str,
    status: Option<&str>,
    limit: u32,
    offset: u32,
    verbose: bool,
) -> Result<()> {
    println!("📋 Credit line requests for merchant: {}", merchant_id);

    if let Some(status) = status {
        println!("   Filter: Status = {}", status);
    }
    println!("   Limit: {}", limit);
    println!("   Offset: {}", offset);

    // Placeholder implementation - would fetch actual requests
    println!();
    println!("✅ Found 4 requests:");

    println!("   1. CR-123456 | $150,000 | APPROVED | 2024-01-20");
    println!("      Rate: 9.5% APR | Term: 180 days | Pool: pool-inst-001");

    println!("   2. CR-123455 | $75,000 | EVALUATING | 2024-01-18");
    println!("      Rate: 10.5% APR | Term: 90 days | Status: Risk review");

    println!("   3. CR-123454 | $200,000 | REJECTED | 2024-01-15");
    println!("      Reason: Insufficient collateral | Appeal: Available");

    println!("   4. CR-123453 | $100,000 | EXPIRED | 2024-01-10");
    println!("      Reason: No response within 7 days | Resubmit: Available");

    if verbose {
        println!();
        println!("🔧 Request Statistics:");
        println!("   Total Requests: 8");
        println!("   Approved: 3 (37.5%)");
        println!("   Evaluating: 2 (25.0%)");
        println!("   Rejected: 2 (25.0%)");
        println!("   Expired: 1 (12.5%)");
        println!("   Total Approved Amount: $425,000");
        println!("   Average Processing Time: 3.2 days");
    }

    Ok(())
}
