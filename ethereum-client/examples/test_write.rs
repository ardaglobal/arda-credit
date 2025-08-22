use alloy_primitives::{Bytes, FixedBytes};
use ethereum_client::{Config, EthereumClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();
    tracing_subscriber::fmt::init();

    println!("✍️  Testing Ethereum Write Operations");
    println!("====================================");

    let config = Config::from_env()?;
    let client = EthereumClient::new(config.clone()).await?;

    println!("✅ Client created successfully");

    if let Some(signer) = &config.signer {
        println!("🔐 Signer: {}", signer.address);
        println!("📝 Contract: {}", config.contract.credit_contract);
    }

    // First, let's check if our signer is authorized
    println!("\n🔍 Testing authorization...");

    if let Some(signer) = &config.signer {
        // We need to call the contract's isAuthorized function
        // Since the client doesn't have this method, let's test with a simple state read first
        println!("📋 Testing with signer address: {}", signer.address);

        // Check if we can read basic contract state
        match client.get_verifier_key().await {
            Ok(vkey) => {
                println!("✅ Contract read successful - connection is good");
                println!("   Verifier key length: {} bytes", vkey.len());
            }
            Err(e) => {
                println!("❌ Contract read failed: {e}");
                return Ok(());
            }
        }
    }

    // Test write operation with minimal data (will likely fail on proof verification)
    println!("\n🔍 Testing contract write operation...");
    println!("ℹ️  Note: This will test authorization, then likely fail on SP1 proof verification");

    let test_state_id = FixedBytes::from([1u8; 32]);
    let test_state_root = FixedBytes::from([2u8; 32]);
    let test_proof = Bytes::from(vec![1, 2, 3, 4]);
    let test_public_values = Bytes::from(vec![5, 6, 7, 8]);

    println!("📤 Attempting to update state...");

    match client
        .update_state(
            test_state_id,
            test_state_root,
            test_proof,
            test_public_values,
        )
        .await
    {
        Ok(result) => {
            println!("✅ State update successful!");
            println!("   Transaction hash: {:?}", result.transaction_hash);
            println!("   Block number: {:?}", result.block_number);
        }
        Err(e) => {
            println!("❌ State update failed: {e}");

            let error_str = format!("{e}");
            println!("\n🔍 Error Analysis:");
            if error_str.contains("0x7fcdd1f4") {
                println!("   Error code 0x7fcdd1f4 = UnauthorizedAccess()");
                println!("   ❌ AUTHORIZATION FAILED");
                println!("   The signer is not authorized to write to this contract");
            } else if error_str.contains("0xf208777e") {
                println!("\n🔍 Error Analysis:");
                println!("   Error code 0xf208777e = Proof verification failed");
                println!("   ✅ AUTHORIZATION PASSED");
                println!("   ❌ SP1 proof verification failed (expected with mock proof data)");
                println!("   This confirms that authorization is working correctly!");
            } else {
                println!("\n🔍 Error Analysis:");
                println!("   Unknown error - might be a different issue");
                println!("   Error details: {error_str}");
            }
        }
    }

    Ok(())
}
