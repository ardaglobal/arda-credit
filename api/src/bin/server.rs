//! Batch Processing API Server
//!
//! This server provides a REST API for submitting transactions, creating batches,
//! and managing ZK proofs for the batch processing system.
//!
//! Run this server using:
//! ```shell
//! cd api && cargo run --bin server
//! ```

use api::{init_tracing, shutdown_telemetry, ApiConfig, ApiServer, ApiServerConfig};
use arda_credit_db::init_db;
use clap::Parser;
use tracing::{error, info};

/// Command line arguments for the server
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Host to bind to
    #[arg(long, default_value = "0.0.0.0")]
    host: String,

    /// Port to bind to
    #[arg(long, default_value = "8080")]
    port: u16,

    /// Enable CORS for all origins
    #[arg(long, default_value = "true")]
    cors: bool,

    /// Maximum batch size for transaction processing
    #[arg(long, default_value = "50", value_parser = clap::value_parser!(u32))]
    max_batch_size: u32,

    /// Maximum request size in bytes (100MB)
    #[arg(long, default_value = "104857600")]
    max_request_size: usize,

    /// Request timeout in seconds
    #[arg(long, default_value = "30")]
    request_timeout: u64,

    /// Enable debug endpoints
    #[arg(long, default_value = "false")]
    debug: bool,
}

#[tokio::main]
#[allow(clippy::too_many_lines)]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Load environment variables from .env file if present
    dotenv::dotenv().ok();

    // Initialize complete tracing setup with OpenTelemetry integration
    init_tracing().map_err(|e| {
        error!("Failed to initialize tracing: {}", e);
        e
    })?;

    // Parse command line arguments
    let args = Args::parse();

    info!("🚀 Starting Batch Processing API Server");
    info!("📍 Server will bind to {}:{}", args.host, args.port);

    // Initialize database connection
    info!("📊 Initializing database connection...");
    let pool = match init_db().await {
        Ok(pool) => {
            info!("✅ Database connection established");
            pool
        }
        Err(e) => {
            error!("❌ Failed to initialize database: {}", e);
            error!("💡 Make sure PostgreSQL is running and DATABASE_URL is set");
            std::process::exit(1);
        }
    };

    // Test database connection
    match sqlx::query("SELECT 1").fetch_one(&pool).await {
        Ok(_) => info!("✅ Database connectivity verified"),
        Err(e) => {
            error!("❌ Database connectivity test failed: {}", e);
            std::process::exit(1);
        }
    }

    // Create API configuration (using defaults for metrics settings from env vars)
    let api_config = ApiConfig {
        max_batch_size: args.max_batch_size,
        enable_debug_endpoints: args.debug,
        ..Default::default()
    };

    // Create server configuration
    let server_config = ApiServerConfig {
        api_config,
        host: args.host.clone(),
        port: args.port,
        enable_rest: true,
        enable_playground: false, // Disabled for batch processing
        enable_compression: true,
        enable_cors: args.cors,
        request_timeout_seconds: args.request_timeout,
        max_request_size_bytes: args.max_request_size,
        cors_origins: if args.cors {
            vec!["*".to_string()]
        } else {
            vec![]
        },
        rate_limit_per_minute: 1000, // Allow high throughput for batch processing
    };

    // Create API server
    let server = ApiServer::with_pool(pool, server_config).await?;

    // Create router
    let app = server.create_router().await;

    // Bind address
    let bind_address = server.bind_address();
    info!("🎯 Binding to address: {}", bind_address);

    let listener = match tokio::net::TcpListener::bind(&bind_address).await {
        Ok(listener) => {
            info!("✅ Successfully bound to {bind_address}");
            listener
        }
        Err(e) => {
            error!("❌ Failed to bind to {bind_address}: {e}");
            error!("💡 Make sure the port is not already in use");
            std::process::exit(1);
        }
    };

    // Print startup information
    println!();
    println!("🌟 Batch Processing API Server Running!");
    println!("📍 Server Address: http://{bind_address}");
    println!("🔄 Background Batch Processing: ENABLED");
    println!("   ⏰ Timer Trigger: Every 1 minute");
    println!("   📊 Count Trigger: 10+ pending transactions");
    println!("   🔧 Manual Trigger: Available via API");
    println!();
    println!("📖 API Documentation:");
    println!("   • Swagger UI:     http://{bind_address}/api-docs/");
    println!("   • OpenAPI JSON:   http://{bind_address}/api-docs/openapi.json");
    println!();
    println!("🎊 Server ready for requests!");
    println!();

    // Setup graceful shutdown
    let shutdown_signal = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install CTRL+C signal handler");
        info!("Shutdown signal received");
        shutdown_telemetry().await;
    };

    // Start server with graceful shutdown
    info!("🚀 Starting server on {}", bind_address);
    if let Err(e) = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal)
        .await
    {
        error!("❌ Server error: {e}");
        shutdown_telemetry().await;
        std::process::exit(1);
    }

    Ok(())
}
