use alloy_primitives::{Bytes, FixedBytes};
use ethereum_client::{Config, EthereumClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();
    tracing_subscriber::fmt::init();

    println!("🔍 Debug Signer Comparison");
    println!("==========================");

    // Test 1: Direct client creation (like our test)
    println!("1️⃣  Testing direct client creation...");
    let config1 = Config::from_env()?;
    let client1 = EthereumClient::new(config1.clone()).await?;

    if let Some(signer1) = &config1.signer {
        println!("   Signer 1: {}", signer1.address);
    }
    println!("   Contract 1: {}", config1.contract.credit_contract);

    // Test 2: Replicate exact prove command flow
    println!("\n2️⃣  Testing prove command flow...");
    let eth_config = Config::from_env()?;
    let eth_client = EthereumClient::new(eth_config.clone()).await?;

    if let Some(signer2) = &eth_config.signer {
        println!("   Signer 2: {}", signer2.address);
    }
    println!("   Contract 2: {}", eth_config.contract.credit_contract);

    // Compare configurations
    println!("\n3️⃣  Configuration comparison...");
    println!(
        "   Configs match: {}",
        config1.signer.as_ref().map(|s| &s.address)
            == eth_config.signer.as_ref().map(|s| &s.address)
    );

    // Test both with same dummy data
    println!("\n4️⃣  Testing both clients with same dummy data...");

    let test_state_id = FixedBytes::from([1u8; 32]);
    let test_state_root = FixedBytes::from([2u8; 32]);
    let test_proof = Bytes::from(vec![1, 2, 3, 4]);
    let test_public_values = Bytes::from(vec![5, 6, 7, 8]);

    // Test client 1
    println!("   Testing client 1...");
    match client1
        .update_state(
            test_state_id,
            test_state_root,
            test_proof.clone(),
            test_public_values.clone(),
        )
        .await
    {
        Ok(_) => println!("   ✅ Client 1 succeeded"),
        Err(e) => {
            let error_str = format!("{e}");
            if error_str.contains("0x7fcdd1f4") {
                println!("   ❌ Client 1: UnauthorizedAccess");
            } else if error_str.contains("0xf208777e") {
                println!("   ✅ Client 1: Authorization passed, proof failed");
            } else {
                println!("   ⚠️  Client 1: Other error: {e}");
            }
        }
    }

    // Test client 2 (prove command style)
    println!("   Testing client 2 (prove style)...");
    match eth_client
        .update_state(
            test_state_id,
            test_state_root,
            test_proof,
            test_public_values,
        )
        .await
    {
        Ok(_) => println!("   ✅ Client 2 succeeded"),
        Err(e) => {
            let error_str = format!("{e}");
            if error_str.contains("0x7fcdd1f4") {
                println!("   ❌ Client 2: UnauthorizedAccess");
            } else if error_str.contains("0xf208777e") {
                println!("   ✅ Client 2: Authorization passed, proof failed");
            } else {
                println!("   ⚠️  Client 2: Other error: {e}");
            }
        }
    }

    Ok(())
}
