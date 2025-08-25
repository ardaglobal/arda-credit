use ethereum_client::{Config, EthereumClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load environment variables from .env file
    dotenv::dotenv().ok();
    tracing_subscriber::fmt::init();

    println!("🔍 Testing Ethereum client configuration...");

    // Load configuration
    match Config::from_env() {
        Ok(config) => {
            println!("✅ Configuration loaded successfully");
            println!("  Network: {}", config.network.name);
            println!("  Chain ID: {}", config.network.chain_id);
            println!("  RPC URL: {}", config.network.rpc_url);
            println!("  Contract: {}", config.contract.arda_credit_contract);

            if let Some(signer) = &config.signer {
                println!("✅ Signer configured: {}", signer.address);
            } else {
                println!("⚠️  No signer configured");
            }

            // Try to create client
            match EthereumClient::new(config).await {
                Ok(client) => {
                    println!("✅ Ethereum client created successfully");

                    // Test network connection
                    match client.get_network_stats().await {
                        Ok(stats) => {
                            println!("✅ Network connection successful!");
                            println!("  Chain ID: {}", stats.chain_id);
                            println!("  Current block: {}", stats.block_number);
                            println!("  Gas price: {} wei", stats.gas_price);
                        }
                        Err(e) => {
                            println!("❌ Network connection failed: {e}");
                        }
                    }
                }
                Err(e) => {
                    println!("❌ Failed to create Ethereum client: {e}");
                }
            }
        }
        Err(e) => {
            println!("❌ Failed to load configuration: {e}");
        }
    }

    Ok(())
}
