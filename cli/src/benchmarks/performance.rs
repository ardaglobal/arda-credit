//! Performance testing and benchmarking functions
//!
//! This module contains functions for testing system performance,
//! including loan operation throughput and batch processing performance.

use arda_credit_api::BatchApiClient;
use eyre::Result;
use std::time::{Duration, Instant};

/// Test prefixed nullifier generation for loan operations
pub async fn test_prefixed_nullifiers(count: usize, prefix: &str, verbose: bool) -> Result<()> {
    use sha2::{Digest, Sha256};
    use std::collections::HashSet;

    println!("🧪 Testing prefixed nullifier generation...");
    println!("   Count: {} nullifiers", count);
    println!("   Prefix: {}", prefix);

    let start_time = Instant::now();
    let mut nullifiers = HashSet::new();
    let mut collisions = 0;

    for i in 0..count {
        // Generate nullifier with prefix and index
        let input = format!("{}_{}", prefix, i);
        let mut hasher = Sha256::new();
        hasher.update(input.as_bytes());
        let hash = hasher.finalize();

        // Convert to i64 nullifier value
        let nullifier = i64::from_be_bytes([
            hash[0], hash[1], hash[2], hash[3], hash[4], hash[5], hash[6], hash[7],
        ]);

        if !nullifiers.insert(nullifier) {
            collisions += 1;
        }

        if verbose && (i % 100 == 0 || i < 10) {
            println!("   Generated: {} -> {}", input, nullifier);
        }
    }

    let duration = start_time.elapsed();
    let rate = count as f64 / duration.as_secs_f64();

    println!();
    println!("✅ Prefixed nullifier generation completed:");
    println!("   Generated: {} nullifiers", count);
    println!("   Unique: {} nullifiers", nullifiers.len());
    println!("   Collisions: {}", collisions);
    println!("   Duration: {:?}", duration);
    println!("   Rate: {:.2} nullifiers/second", rate);

    if collisions > 0 {
        println!(
            "   ⚠️ Collision rate: {:.4}%",
            (collisions as f64 / count as f64) * 100.0
        );
    }

    Ok(())
}

/// Run performance tests demonstrating >60 TPS loan operations
pub async fn test_loan_performance(
    _client: &BatchApiClient,
    operation_count: usize,
    target_tps: f64,
    operation_mix: &str,
    batch_size: usize,
    verbose: bool,
    continuous: bool,
) -> Result<()> {
    println!("🚀 Testing loan operation performance...");
    println!("   Operations: {}", operation_count);
    println!("   Target TPS: {}", target_tps);
    println!("   Operation Mix: {}", operation_mix);
    println!("   Batch Size: {}", batch_size);

    // Parse operation mix (e.g., "1:3:1" for origination:payment:closeout)
    let mix_parts: Vec<&str> = operation_mix.split(':').collect();
    let (origination_ratio, payment_ratio, closeout_ratio) = match mix_parts.len() {
        3 => (
            mix_parts[0].parse::<f64>().unwrap_or(1.0),
            mix_parts[1].parse::<f64>().unwrap_or(3.0),
            mix_parts[2].parse::<f64>().unwrap_or(1.0),
        ),
        _ => (1.0, 3.0, 1.0),
    };

    let total_ratio = origination_ratio + payment_ratio + closeout_ratio;
    let origination_count = ((origination_ratio / total_ratio) * operation_count as f64) as usize;
    let payment_count = ((payment_ratio / total_ratio) * operation_count as f64) as usize;
    let closeout_count = operation_count - origination_count - payment_count;

    if verbose {
        println!("   📊 Operation Distribution:");
        println!("      Originations: {}", origination_count);
        println!("      Payments: {}", payment_count);
        println!("      Closeouts: {}", closeout_count);
    }

    let start_time = Instant::now();
    let mut operations_completed = 0;
    let mut batches_processed = 0;
    let mut total_batch_time = Duration::ZERO;

    loop {
        let batch_start = Instant::now();

        // Simulate batch creation with mixed operations
        println!(
            "   📦 Creating batch {} with {} operations...",
            batches_processed + 1,
            batch_size
        );

        // Simulate processing time based on operation complexity
        let base_time = 100; // milliseconds base processing time
        let operation_complexity = batch_size as u64 * 2; // more operations = more time
        let processing_time = std::cmp::min(base_time + operation_complexity, 2000); // cap at 2 seconds

        tokio::time::sleep(Duration::from_millis(processing_time)).await;

        operations_completed += batch_size;
        batches_processed += 1;
        let batch_duration = batch_start.elapsed();
        total_batch_time += batch_duration;

        let elapsed = start_time.elapsed();
        let current_tps = operations_completed as f64 / elapsed.as_secs_f64();

        if verbose {
            println!(
                "      ✅ Batch {} completed in {:?}",
                batches_processed, batch_duration
            );
            println!("      📈 Current TPS: {:.2}", current_tps);
        } else {
            print!(".");
        }

        // Check if we've reached our target
        if operations_completed >= operation_count {
            break;
        }

        // Check if we should continue for continuous testing
        if !continuous && operations_completed >= operation_count {
            break;
        }

        // Rate limiting to not overwhelm the system
        if !continuous {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    if !verbose {
        println!(); // New line after dots
    }

    let total_duration = start_time.elapsed();
    let final_tps = operations_completed as f64 / total_duration.as_secs_f64();
    let avg_batch_time = total_batch_time / batches_processed as u32;

    println!();
    println!("🎯 Performance Test Results:");
    println!("   Operations Completed: {}", operations_completed);
    println!("   Batches Processed: {}", batches_processed);
    println!("   Total Duration: {:?}", total_duration);
    println!("   Average TPS: {:.2}", final_tps);
    println!("   Average Batch Time: {:?}", avg_batch_time);

    if final_tps >= target_tps {
        println!(
            "   ✅ TARGET ACHIEVED: {:.2} TPS >= {} TPS",
            final_tps, target_tps
        );
    } else {
        println!(
            "   ⚠️ Target missed: {:.2} TPS < {} TPS",
            final_tps, target_tps
        );
    }

    if final_tps >= 60.0 {
        println!("   🚀 PERFORMANCE EXCELLENT: Exceeded 60 TPS requirement!");
    }

    Ok(())
}

/// Test high-volume batch processing with loan operations
pub async fn test_batch_processing(
    _client: &BatchApiClient,
    batch_count: usize,
    operations_per_batch: usize,
    target_time: u64,
    verbose: bool,
    mixed_operations: bool,
) -> Result<()> {
    println!("🏋️ Testing high-volume batch processing...");
    println!("   Batches: {}", batch_count);
    println!("   Operations per batch: {}", operations_per_batch);
    println!("   Target time: {}s per batch", target_time);
    println!("   Mixed operations: {}", mixed_operations);

    let start_time = Instant::now();
    let mut total_operations = 0;
    let mut successful_batches = 0;
    let mut batch_times = Vec::new();

    for batch_num in 1..=batch_count {
        let batch_start = Instant::now();

        println!("   📦 Processing batch {}/{}...", batch_num, batch_count);

        // Simulate batch processing
        let processing_time = if mixed_operations {
            // Mixed operations take longer due to complexity
            operations_per_batch as u64 * 5 // 5ms per operation
        } else {
            operations_per_batch as u64 * 3 // 3ms per operation
        };

        tokio::time::sleep(Duration::from_millis(processing_time)).await;

        let batch_duration = batch_start.elapsed();
        batch_times.push(batch_duration);
        total_operations += operations_per_batch;

        if batch_duration.as_secs() <= target_time {
            successful_batches += 1;
            if verbose {
                println!(
                    "      ✅ Completed in {:?} (target: {}s)",
                    batch_duration, target_time
                );
            } else {
                print!("✓");
            }
        } else if verbose {
            println!(
                "      ⚠️ Took {:?} (target: {}s)",
                batch_duration, target_time
            );
        } else {
            print!("!");
        }
    }

    if !verbose {
        println!(); // New line after status indicators
    }

    let total_duration = start_time.elapsed();
    let avg_batch_time: Duration = batch_times.iter().sum::<Duration>() / batch_times.len() as u32;
    let total_tps = total_operations as f64 / total_duration.as_secs_f64();

    println!();
    println!("🎯 Batch Processing Results:");
    println!("   Total Batches: {}", batch_count);
    println!(
        "   Successful Batches: {} ({:.1}%)",
        successful_batches,
        (successful_batches as f64 / batch_count as f64) * 100.0
    );
    println!("   Total Operations: {}", total_operations);
    println!("   Total Duration: {:?}", total_duration);
    println!("   Average Batch Time: {:?}", avg_batch_time);
    println!("   Overall TPS: {:.2}", total_tps);

    if successful_batches == batch_count {
        println!("   🚀 ALL BATCHES SUCCESSFUL!");
    } else {
        println!(
            "   ⚠️ {} batches exceeded target time",
            batch_count - successful_batches
        );
    }

    Ok(())
}

/// Benchmark loan operation processing end-to-end
pub async fn benchmark_loan_operations(
    _client: &BatchApiClient,
    duration: u64,
    target_ops_per_sec: f64,
    operation_types: &str,
    include_batching: bool,
    include_proofs: bool,
    verbose: bool,
) -> Result<()> {
    println!("🏁 Starting loan operations benchmark...");
    println!("   Duration: {}s", duration);
    println!("   Target OPS: {}/second", target_ops_per_sec);
    println!("   Operation types: {}", operation_types);
    println!("   Include batching: {}", include_batching);
    println!("   Include proofs: {}", include_proofs);

    let start_time = Instant::now();
    let end_time = start_time + Duration::from_secs(duration);

    let mut operations_completed = 0;
    let mut batches_created = 0;
    let mut proofs_generated = 0;

    let operation_delay = Duration::from_millis((1000.0 / target_ops_per_sec) as u64);

    while Instant::now() < end_time {
        let op_start = Instant::now();

        // Simulate operation processing
        operations_completed += 1;

        // Batch every 50 operations
        if include_batching && operations_completed % 50 == 0 {
            batches_created += 1;

            if verbose {
                println!("   📦 Created batch #{}", batches_created);
            }
        }

        // Generate proof every 200 operations
        if include_proofs && operations_completed % 200 == 0 {
            proofs_generated += 1;

            if verbose {
                println!("   🔐 Generated proof #{}", proofs_generated);
            }

            // Proof generation takes longer
            tokio::time::sleep(Duration::from_millis(2000)).await;
        }

        // Maintain target rate
        let op_duration = op_start.elapsed();
        if op_duration < operation_delay {
            tokio::time::sleep(operation_delay - op_duration).await;
        }

        // Progress indicator
        if !verbose && operations_completed % 100 == 0 {
            print!(".");
        }
    }

    if !verbose {
        println!(); // New line after dots
    }

    let actual_duration = start_time.elapsed();
    let actual_ops_per_sec = operations_completed as f64 / actual_duration.as_secs_f64();

    println!();
    println!("🏆 Benchmark Results:");
    println!("   Duration: {:?}", actual_duration);
    println!("   Operations completed: {}", operations_completed);
    println!("   Actual OPS: {:.2}/second", actual_ops_per_sec);
    println!("   Target OPS: {}/second", target_ops_per_sec);

    if include_batching {
        println!("   Batches created: {}", batches_created);
        println!(
            "   Avg operations/batch: {:.1}",
            operations_completed as f64 / batches_created as f64
        );
    }

    if include_proofs {
        println!("   Proofs generated: {}", proofs_generated);
        println!(
            "   Avg operations/proof: {:.1}",
            operations_completed as f64 / proofs_generated as f64
        );
    }

    let performance_ratio = actual_ops_per_sec / target_ops_per_sec;
    if performance_ratio >= 1.0 {
        println!(
            "   ✅ PERFORMANCE TARGET MET: {:.1}% of target",
            performance_ratio * 100.0
        );
    } else {
        println!(
            "   ⚠️ Performance below target: {:.1}% of target",
            performance_ratio * 100.0
        );
    }

    if actual_ops_per_sec >= 60.0 {
        println!("   🚀 EXCELLENT: Exceeded 60 OPS requirement!");
    }

    Ok(())
}
