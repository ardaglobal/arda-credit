use alloy_primitives::Address;
use ethereum_client::{Config, EthereumClient};

#[tokio::main]
#[allow(clippy::unreadable_literal)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();
    tracing_subscriber::fmt::init();

    println!("🌐 Testing Ethereum Network Connection");
    println!("=====================================");

    let config = Config::from_env()?;

    println!("📋 Configuration loaded:");
    println!("  Network: {}", config.network.name);
    println!("  Expected Chain ID: {}", config.network.chain_id);
    println!("  RPC URL: {}", config.network.rpc_url);
    println!("  Is testnet: {}", config.network.is_testnet);
    println!();

    let client = EthereumClient::new(config.clone()).await?;
    println!("✅ Client created successfully");

    // Test actual network connection using available methods
    println!("\n🔍 Testing network connection...");

    // Test contract interaction to verify we're on the right network
    match client.get_verifier_key().await {
        Ok(vkey) => {
            println!("✅ Contract is accessible and responsive");
            println!("     Verifier key length: {} bytes", vkey.len());
            println!("✅ This confirms we're connected to the correct network!");
        }
        Err(e) => {
            println!("❌ Failed to read contract: {e}");
            println!("     This could mean:");
            println!("     - Contract doesn't exist at this address on the connected network");
            println!("     - We're connected to the wrong network");
            println!("     - RPC connection issues");
        }
    }

    // Test a state read to further verify contract functionality
    let test_state_id = [0u8; 32];
    match client.get_current_state(test_state_id.into()).await {
        Ok(state) => {
            if let Some(_s) = state {
                println!("✅ Contract state read successful");
            } else {
                println!("ℹ️  State not found (expected for new contract)");
            }
        }
        Err(e) => {
            println!("⚠️  State read failed: {e}");
        }
    }

    println!("\n📊 Network Summary:");
    println!("==================");
    println!(
        "📋 Expected Network: {} (Chain ID: {})",
        if config.network.chain_id == 11155111 {
            "Sepolia Testnet"
        } else if config.network.chain_id == 1 {
            "Ethereum Mainnet"
        } else {
            "Unknown"
        },
        config.network.chain_id
    );

    if config.contract.arda_credit_contract != Address::ZERO {
        println!("📝 Contract: {}", config.contract.arda_credit_contract);
        println!(
            "🔗 Sepolia Explorer: https://sepolia.etherscan.io/address/{}",
            config.contract.arda_credit_contract
        );
        println!(
            "🔗 Mainnet Explorer: https://etherscan.io/address/{}",
            config.contract.arda_credit_contract
        );
    }

    if let Some(signer) = &config.signer {
        println!("🔐 Signer: {}", signer.address);
    }

    Ok(())
}
