use axum::{
    http::StatusCode,
    response::{Html, IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use std::time::Duration;

use sqlx::PgPool;
use tower_http::{
    compression::CompressionLayer, cors::CorsLayer, limit::RequestBodyLimitLayer,
    timeout::TimeoutLayer, trace::TraceLayer,
};
use tracing::{info, instrument, warn};

use crate::api::openapi::ApiDoc;
use crate::config::types::{ApiConfig, ApiState};
use crate::middleware::middleware::{EmailRateLimiter, RateLimiter};
use crate::services::batch_processor::{create_batch_processor_config, start_batch_processor};
use crate::services::deployment_config::get_deployment;
use crate::services::email_service::EmailService;
use crate::services::magic_link_service::{MagicLinkConfig, MagicLinkService};
use crate::services::session_service::SessionService;
use crate::utils::session_cleanup::SessionCleanupService;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use arda_credit_db::{init_db, AdsConfig, AdsServiceFactory, IndexedMerkleTreeADS};
use std::sync::Arc;
use tokio::sync::RwLock;

// ============================================================================
// API SERVER CONFIGURATION
// ============================================================================

/// Configuration for the batch processing API server
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone)]
pub struct ApiServerConfig {
    /// Base API configuration
    pub api_config: ApiConfig,

    /// Server binding configuration
    pub host: String,
    pub port: u16,

    /// Feature flags
    pub enable_rest: bool,
    pub enable_playground: bool,

    /// Middleware configuration
    pub enable_compression: bool,
    pub enable_cors: bool,
    pub request_timeout_seconds: u64,
    pub max_request_size_bytes: usize,

    /// Security configuration
    pub cors_origins: Vec<String>,
    pub rate_limit_per_minute: u32,
}

impl Default for ApiServerConfig {
    fn default() -> Self {
        Self {
            api_config: ApiConfig::default(),
            host: "0.0.0.0".to_string(),
            port: 8080,
            enable_rest: true,
            enable_playground: true,
            enable_compression: true,
            enable_cors: true,
            request_timeout_seconds: 30,
            max_request_size_bytes: 100 * 1024 * 1024, // 100MB (for document uploads)
            cors_origins: vec!["*".to_string()],
            rate_limit_per_minute: 100,
        }
    }
}

// ============================================================================
// API SERVER IMPLEMENTATION
// ============================================================================

/// Batch processing API server
pub struct ApiServer {
    config: ApiServerConfig,
    state: ApiState,
}

impl ApiServer {
    /// Create new API server with database connection
    ///
    /// # Errors
    /// Returns error if database initialization fails
    #[instrument(skip(database_url), level = "info")]
    pub async fn new(
        database_url: Option<String>,
        config: ApiServerConfig,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        info!("🚀 Initializing batch processing API server");

        // Pre-load deployment configuration for local/staging environments
        let environment =
            std::env::var("SERVER_ENVIRONMENT").unwrap_or_else(|_| "local".to_string());

        if matches!(environment.as_str(), "local" | "staging") {
            info!(
                "📁 Pre-loading deployment configuration for environment: {}",
                environment
            );
            match get_deployment() {
                Ok(deployment) => {
                    info!(
                        "✅ Deployment configuration loaded: ARDA={}, Chain={}",
                        deployment.config.arda, deployment.config.chain_id
                    );
                }
                Err(e) => {
                    warn!("⚠️  Failed to load deployment configuration: {}", e);
                    warn!(
                        "💡 Run 'make deploy-contracts' to deploy contracts for local development"
                    );
                }
            }
        }

        // Initialize database connection
        let pool = if let Some(url) = database_url {
            arda_credit_db::init_db_with_url(&url).await?
        } else {
            init_db().await?
        };

        // Initialize ADS service with recovery from database
        info!("🔐 Initializing ADS service with database recovery");
        let ads_service = Self::initialize_ads_service(pool.clone()).await?;

        // Start background batch processor
        let batch_processor_config = create_batch_processor_config(&config.api_config);
        let batch_processor_handle =
            start_batch_processor(pool.clone(), batch_processor_config, ads_service.clone()).await;

        // Initialize session service (replaces JWT auth)
        let session_service = Arc::new(SessionService::new(pool.clone()));

        // Start session cleanup background service
        let cleanup_service = SessionCleanupService::new(session_service.clone());
        cleanup_service.start();

        // Initialize withdrawal and balance services
        let withdrawal_service = crate::WithdrawalTransactionService::new(pool.clone());
        let balance_service = crate::BalanceVerificationService::new(pool.clone());

        // Initialize document storage service
        info!("📁 Initializing document storage service");
        let document_storage_config =
            crate::services::document_storage::DocumentStorageConfig::default();
        let document_storage_service = std::sync::Arc::new(
            crate::services::document_storage::DocumentStorageService::new(document_storage_config),
        );

        // Initialize Prometheus metrics
        info!("📊 Initializing Prometheus metrics");
        let metrics_handle = crate::utils::metrics::MetricsHandle::new().map_err(|e| {
            warn!("Failed to initialize metrics: {}", e);
            e
        })?;

        // Initialize SurrealDB service for document storage
        info!("🌐 Initializing SurrealDB service");
        let surrealdb_service = match crate::services::document_ingestion::surrealdb_service::SurrealDBService::from_env().await {
            Ok(service) => {
                info!("✅ SurrealDB service initialized successfully");
                // Initialize schema on startup
                if let Err(e) = service.init_schema().await {
                    warn!("⚠️  Failed to initialize SurrealDB schema: {}", e);
                    warn!("💡 Schema initialization is optional, continuing without it");
                }
                Some(Arc::new(service))
            }
            Err(e) => {
                warn!("⚠️  Failed to initialize SurrealDB service: {}", e);
                warn!("💡 Set SURREALDB_URL, SURREALDB_USERNAME, and SURREALDB_PASSWORD to enable document storage");
                None
            }
        };

        // Create API state
        let state = ApiState {
            pool,
            config: config.api_config.clone(),
            batch_processor: Some(batch_processor_handle),
            ads_service,
            session_service,
            magic_link_service: None,      // Will be set in create_router
            team_invitation_service: None, // Will be set in create_router
            withdrawal_service,
            balance_service,
            withdrawal_executor: None, // Not configured in this constructor
            document_service: None,    // Document service not configured in basic constructor
            document_storage_service: Some(document_storage_service),
            metrics_handle: Some(Arc::new(metrics_handle)),
            email_rate_limiter: None, // Will be set in create_router
            surrealdb_service,        // SurrealDB service initialized from environment
        };

        let server = Self { config, state };

        info!("✅ API server initialized successfully");
        Ok(server)
    }

    /// Create new API server with existing database pool
    pub async fn with_pool(
        pool: PgPool,
        config: ApiServerConfig,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        info!("🚀 Creating API server with existing database pool");

        // Initialize ADS service with recovery from database
        info!("🔐 Initializing ADS service with database recovery");
        let ads_service = Self::initialize_ads_service(pool.clone()).await?;

        // Start background batch processor
        let batch_processor_config = create_batch_processor_config(&config.api_config);
        let batch_processor_handle =
            start_batch_processor(pool.clone(), batch_processor_config, ads_service.clone()).await;

        // Initialize session service (replaces JWT auth)
        let session_service = Arc::new(SessionService::new(pool.clone()));

        // Start session cleanup background service
        let cleanup_service = SessionCleanupService::new(session_service.clone());
        cleanup_service.start();

        // Initialize withdrawal and balance services
        let withdrawal_service = crate::WithdrawalTransactionService::new(pool.clone());
        let balance_service = crate::BalanceVerificationService::new(pool.clone());

        // Initialize document storage service
        info!("📁 Initializing document storage service");
        let document_storage_config =
            crate::services::document_storage::DocumentStorageConfig::default();
        let document_storage_service = std::sync::Arc::new(
            crate::services::document_storage::DocumentStorageService::new(document_storage_config),
        );

        // Initialize simple withdrawal executor service if Ethereum config is available
        let withdrawal_executor = if let Some(executor_config) =
            crate::services::simple_withdrawal_executor::create_simple_withdrawal_executor_config()
        {
            info!("🔧 Initializing simple withdrawal executor service");
            info!(
                "📋 Executor config: batch_size={}, gas_limit={}, dry_run={}",
                executor_config.batch_size, executor_config.gas_limit, executor_config.dry_run
            );

            // Create minimal ethereum config for withdrawal executor
            // The config function already verified that required environment variables exist
            let config = ethereum_client::config::Config {
                network: ethereum_client::config::NetworkConfig {
                    name: "sepolia".to_string(),
                    chain_id: 11155111,
                    rpc_url: std::env::var("ETHEREUM_RPC_URL")
                        .and_then(|url| {
                            url::Url::parse(&url).map_err(|_| std::env::VarError::NotPresent)
                        })
                        .unwrap_or_else(|_| url::Url::parse("http://localhost:8545").unwrap()),
                    ws_url: None,
                    explorer_url: None,
                    is_testnet: true,
                },
                contract: ethereum_client::config::ContractConfig {
                    arda_credit_contract: alloy_primitives::Address::from_slice(
                        &hex::decode(
                            std::env::var("ETHEREUM_CONTRACT_ADDRESS")
                                .unwrap()
                                .strip_prefix("0x")
                                .unwrap_or(&std::env::var("ETHEREUM_CONTRACT_ADDRESS").unwrap()),
                        )
                        .unwrap_or_else(|_| vec![0; 20])[..20],
                    ),
                    verifier_contract: alloy_primitives::Address::ZERO,
                    deployment_block: None,
                },
                rpc: ethereum_client::config::RpcConfig {
                    notify_addresses: vec![],
                    rate_limit_per_second: 10,
                },
                signer: Some(ethereum_client::config::SignerConfig {
                    private_key: std::env::var("ETHEREUM_WALLET_PRIVATE_KEY").unwrap(),
                    address: alloy_primitives::Address::ZERO, // Will be derived from private key
                }),
                monitoring: ethereum_client::config::MonitoringConfig {
                    enable_event_monitoring: false,
                    polling_interval_seconds: 30,
                    max_block_range: 1000,
                    retry_attempts: 3,
                    timeout_seconds: 30,
                },
            };

            match ethereum_client::client::EthereumClient::new_without_validation(config).await {
                Ok(ethereum_client) => {
                    let service =
                        crate::services::simple_withdrawal_executor::SimpleWithdrawalExecutorService::new(
                            pool.clone(),
                            ethereum_client,
                            executor_config,
                        );
                    let handle =
                        crate::services::simple_withdrawal_executor::SimpleWithdrawalExecutorHandle::new(
                            service,
                        );
                    info!("✅ Simple withdrawal executor service initialized");
                    Some(handle)
                }
                Err(e) => {
                    tracing::warn!(
                        "⚠️ Failed to initialize ethereum client for withdrawal executor: {}",
                        e
                    );
                    tracing::warn!("💡 Withdrawals will need to be processed manually");
                    None
                }
            }
        } else {
            info!(
                "ℹ️ Withdrawal executor disabled: missing required Ethereum environment variables"
            );
            info!("💡 Set ETHEREUM_RPC_URL, ETHEREUM_CONTRACT_ADDRESS, and ETHEREUM_WALLET_PRIVATE_KEY to enable automatic withdrawal execution");
            None
        };
        // Initialize Prometheus metrics
        info!("📊 Initializing Prometheus metrics");
        let metrics_handle = crate::utils::metrics::MetricsHandle::new().map_err(|e| {
            warn!("Failed to initialize metrics: {}", e);
            e
        })?;

        // Initialize SurrealDB service for document storage
        info!("🌐 Initializing SurrealDB service");
        let surrealdb_service = match crate::services::document_ingestion::surrealdb_service::SurrealDBService::from_env().await {
            Ok(service) => {
                info!("✅ SurrealDB service initialized successfully");
                // Initialize schema on startup
                if let Err(e) = service.init_schema().await {
                    warn!("⚠️  Failed to initialize SurrealDB schema: {}", e);
                    warn!("💡 Schema initialization is optional, continuing without it");
                }
                Some(Arc::new(service))
            }
            Err(e) => {
                warn!("⚠️  Failed to initialize SurrealDB service: {}", e);
                warn!("💡 Set SURREALDB_URL, SURREALDB_USERNAME, and SURREALDB_PASSWORD to enable document storage");
                None
            }
        };

        let state = ApiState {
            pool,
            config: config.api_config.clone(),
            batch_processor: Some(batch_processor_handle),
            ads_service,
            session_service,
            magic_link_service: None,      // Will be set in create_router
            team_invitation_service: None, // Will be set in create_router
            withdrawal_service,
            balance_service,
            withdrawal_executor,
            document_service: None, // Document service not configured in this constructor
            document_storage_service: Some(document_storage_service),
            metrics_handle: Some(Arc::new(metrics_handle)),
            email_rate_limiter: None, // Will be set in create_router
            surrealdb_service,        // SurrealDB service initialized from environment
        };

        Ok(Self { config, state })
    }

    /// Initialize ADS service with recovery from database state
    #[instrument(skip(pool), level = "info")]
    async fn initialize_ads_service(
        pool: PgPool,
    ) -> Result<Arc<RwLock<IndexedMerkleTreeADS>>, Box<dyn std::error::Error + Send + Sync>> {
        info!("🔐 Creating ADS service configuration");

        // Create ADS configuration for production use
        let ads_config = AdsConfig {
            settlement_contract: "0x742d35cc6640CA5AaAaB2AAD9d8e7f2B6E37b5D1".to_string(),
            chain_id: 1, // Mainnet - should be configurable
            audit_enabled: true,
            metrics_enabled: true,
            cache_size_limit: 50_000,
            batch_size_limit: 1_000,
            gas_price: 20_000_000_000, // 20 gwei
        };

        info!("🏭 Creating ADS service factory");
        let factory = AdsServiceFactory::with_config(pool.clone(), ads_config);

        info!("🌳 Initializing IndexedMerkleTreeADS (with database recovery)");
        let ads_service = factory
            .create_indexed_merkle_tree()
            .await
            .map_err(|e| format!("Failed to create ADS service: {}", e))?;

        let ads_service = Arc::new(RwLock::new(ads_service));

        info!("✅ ADS service initialized successfully with database recovery");
        Ok(ads_service)
    }

    /// Build the complete router with all endpoints
    #[instrument(skip(self), level = "info")]
    pub async fn create_router(&self) -> Router {
        info!("🔧 Building API router");

        // Initialize email rate limiter for magic link spam prevention
        let email_rate_limiter = Arc::new(EmailRateLimiter::new());

        // Initialize email service for magic links
        let email_config =
            crate::config::email_config::EmailConfig::from_env().unwrap_or_else(|_| {
                crate::config::email_config::EmailConfig {
                    enabled: false,
                    ses_region: "us-east-1".to_string(),
                    email_from: "test@example.com".to_string(),
                    email_reply_to: "noreply@example.com".to_string(),
                    configuration_set: None,
                    magic_base_url: "https://app.arda.xyz/auth/verify".to_string(),
                    debug_mode: false,
                }
            });

        let email_service = Arc::new(EmailService::new(email_config).await.unwrap_or_else(|e| {
            warn!("Failed to initialize email service ({}), using mock", e);
            // For testing/development, create a basic disabled config
            // This will be handled by the EmailService internally
            panic!("Email service initialization failed - check AWS SES configuration")
        }));
        let magic_link_config = MagicLinkConfig::default();
        let magic_link_service = Arc::new(MagicLinkService::new(
            self.state.pool.clone(),
            magic_link_config.clone(),
            email_service.clone(),
            Arc::new(RateLimiter::new(180)), // 180 requests per minute for magic links
            self.state.session_service.clone(),
        ));

        // Initialize team invitation service
        let team_invitation_service = Arc::new(
            crate::services::team_invitation_service::TeamInvitationService::new(
                self.state.pool.clone(),
                email_service,
                magic_link_config,
            ),
        );

        // Create API state with email rate limiter
        let api_state = ApiState {
            pool: self.state.pool.clone(),
            config: self.state.config.clone(),
            batch_processor: self.state.batch_processor.clone(),
            ads_service: self.state.ads_service.clone(),
            session_service: self.state.session_service.clone(),
            magic_link_service: Some(magic_link_service),
            team_invitation_service: Some(team_invitation_service),
            withdrawal_service: self.state.withdrawal_service.clone(),
            balance_service: self.state.balance_service.clone(),
            withdrawal_executor: self.state.withdrawal_executor.clone(),
            metrics_handle: self.state.metrics_handle.clone(),
            email_rate_limiter: Some(email_rate_limiter),
            document_service: None,
            document_storage_service: self.state.document_storage_service.clone(),
            surrealdb_service: self.state.surrealdb_service.clone(),
        };

        // Use REST API routes (includes health endpoints and state)
        let router = if self.config.enable_rest {
            info!("📡 Adding batch processing REST API routes");
            crate::api::router::create_router(api_state.clone())
        } else {
            Router::new()
                .route("/health", get(health_check))
                .route("/", get(api_info_handler))
                .with_state(api_state.clone())
        };

        // TODO: Apply session authentication middleware (replaces JWT middleware)
        // Currently commented out due to axum middleware complexity.
        // Session authentication should be implemented at the handler level
        // or using a different middleware approach.
        // let api_state_arc = Arc::new(api_state.clone());
        // let router = router.layer(axum::middleware::from_fn({
        //     let api_state = Arc::clone(&api_state_arc);
        //     move |cookies, mut request, next| {
        //         let api_state = (*api_state).clone();
        //         async move {
        //             match session_auth_middleware(api_state, cookies, request, next).await {
        //                 Ok(response) => response,
        //                 Err(_) => create_unauthorized_response("Authentication error"),
        //             }
        //         }
        //     }
        // }));

        // Add other middleware layers
        let router = self.add_middleware_with_state(router);

        info!("✅ API router built successfully with session authentication");
        router
    }

    /// Add middleware layers to the router
    fn add_middleware_with_state(&self, router: Router) -> Router {
        let mut router = router
            .layer(axum::middleware::from_fn(
                crate::utils::metrics::metrics_middleware,
            ))
            .layer(TraceLayer::new_for_http())
            .layer(RequestBodyLimitLayer::new(
                self.config.max_request_size_bytes,
            ))
            .layer(TimeoutLayer::new(Duration::from_secs(
                self.config.request_timeout_seconds,
            )));

        if self.config.enable_compression {
            router = router.layer(CompressionLayer::new());
        }

        if self.config.enable_cors {
            use tower_http::cors::AllowOrigin;

            // Parse configured origins or use default localhost
            let origins: Vec<_> = self
                .config
                .cors_origins
                .iter()
                .filter_map(|origin| {
                    if origin == "*" {
                        // Skip wildcards when credentials are used
                        None
                    } else {
                        origin.parse::<axum::http::HeaderValue>().ok()
                    }
                })
                .collect();

            let allow_origin = if origins.is_empty() {
                // Default to allowing localhost origins for development
                AllowOrigin::list([
                    "http://localhost:5173".parse().unwrap(),
                    "http://localhost:3000".parse().unwrap(),
                    "http://localhost:3001".parse().unwrap(),
                    "http://127.0.0.1:5173".parse().unwrap(),
                    "http://127.0.0.1:3000".parse().unwrap(),
                    "http://127.0.0.1:3001".parse().unwrap(),
                ])
            } else {
                AllowOrigin::list(origins)
            };

            let cors = CorsLayer::new()
                .allow_origin(allow_origin)
                .allow_methods([
                    axum::http::Method::GET,
                    axum::http::Method::POST,
                    axum::http::Method::PUT,
                    axum::http::Method::DELETE,
                    axum::http::Method::OPTIONS,
                ])
                .allow_headers([
                    axum::http::header::CONTENT_TYPE,
                    axum::http::header::AUTHORIZATION,
                    axum::http::header::ACCEPT,
                    axum::http::header::ORIGIN,
                    axum::http::header::USER_AGENT,
                    axum::http::header::CACHE_CONTROL,
                    axum::http::header::HeaderName::from_static("x-requested-with"),
                ])
                .allow_credentials(true);

            router = router.layer(cors);
        }

        router
    }

    /// Get server binding address
    #[must_use]
    pub fn bind_address(&self) -> String {
        format!("{}:{}", self.config.host, self.config.port)
    }

    /// Get API state reference
    #[must_use]
    pub const fn state(&self) -> &ApiState {
        &self.state
    }

    /// Get server configuration
    #[must_use]
    pub const fn config(&self) -> &ApiServerConfig {
        &self.config
    }
}

// ============================================================================
// HANDLERS
// ============================================================================

/// Health check endpoint
#[instrument(level = "info")]
async fn health_check() -> impl IntoResponse {
    info!("Health check requested");
    "OK"
}

/// API information handler
#[instrument(level = "info")]
async fn api_info_handler() -> Html<&'static str> {
    info!("API info page requested");
    Html(
        r#"
<!DOCTYPE html>
<html>
<head>
    <title>Batch Processing API</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 40px; }
        .header { color: #2c3e50; border-bottom: 2px solid #3498db; padding-bottom: 20px; }
        .feature { background: #ecf0f1; padding: 15px; margin: 10px 0; border-radius: 5px; }
        .endpoint { font-family: monospace; background: #2c3e50; color: #ecf0f1; padding: 5px; border-radius: 3px; }
        .specs { background: #e8f5e8; padding: 15px; border-left: 4px solid #27ae60; margin: 20px 0; }
    </style>
</head>
<body>
    <div class="header">
        <h1>🔄 Batch Processing API</h1>
        <p>Zero-knowledge proof system for batched transaction processing</p>
    </div>

    <div class="specs">
        <h3>📊 System Overview</h3>
        <ul>
            <li><strong>Architecture:</strong> Continuous balance tracking with ZK proofs</li>
            <li><strong>Batching:</strong> FIFO transaction processing with configurable batch sizes</li>
            <li><strong>Privacy:</strong> Individual transaction amounts remain private in ZK proofs</li>
            <li><strong>State Transitions:</strong> Proven counter transitions (e.g., 10 → 22)</li>
            <li><strong>Smart Contract Ready:</strong> Merkle roots and ZK proofs for on-chain verification</li>
            <li><strong>Database:</strong> PostgreSQL with atomic batch operations</li>
        </ul>
    </div>

    <div class="feature">
        <h3>🔗 REST API Endpoints</h3>
        <p>Version 2.0 batch processing endpoints:</p>
        <ul>
            <li><span class="endpoint">POST /api/v1/transactions</span> - Submit transaction</li>
            <li><span class="endpoint">GET /api/v1/transactions/pending</span> - View pending transactions</li>
            <li><span class="endpoint">POST /api/v1/batches</span> - Create batch from pending transactions</li>
            <li><span class="endpoint">GET /api/v1/batches</span> - List historical batches</li>
            <li><span class="endpoint">GET /api/v1/batches/:id</span> - Get specific batch</li>
            <li><span class="endpoint">POST /api/v1/batches/:id/proof</span> - Update batch with ZK proof</li>
            <li><span class="endpoint">GET /api/v1/state/current</span> - Get current counter state</li>
            <li><span class="endpoint">GET /api/v1/state/:id/contract</span> - Get contract submission data</li>
            <li><span class="endpoint">GET /api/v1/health</span> - Health check</li>
        </ul>
    </div>

    <div class="feature">
        <h3>🚀 Workflow</h3>
        <ol>
            <li><strong>Submit Transactions:</strong> Users submit integer amounts to be added to counter</li>
            <li><strong>Batch Creation:</strong> Accumulate transactions and create processing batches</li>
            <li><strong>ZK Proof Generation:</strong> Generate proofs showing correct state transitions</li>
            <li><strong>Merkle Tree Updates:</strong> Store authenticated data structure commits</li>
            <li><span class="endpoint">Smart Contract Submission:</span> Prepare data for on-chain verification</li>
        </ol>
    </div>

    <div class="feature">
        <h3>📚 Example Usage</h3>
        <p>Submit a transaction to add 5 to the counter:</p>
        <pre><code>curl -X POST http://localhost:8080/api/v1/transactions \
  -H "Content-Type: application/json" \
  -d '{"amount": 5}'</code></pre>

        <p>Create a batch of pending transactions:</p>
        <pre><code>curl -X POST http://localhost:8080/api/v1/batches \
  -H "Content-Type: application/json" \
  -d '{"batch_size": 10}'</code></pre>
    </div>
</body>
</html>
    "#,
    )
}

/// Create unauthorized response for middleware
#[allow(dead_code)]
fn create_unauthorized_response(message: &str) -> Response {
    let error_body = serde_json::json!({
        "error": "AUTHENTICATION_ERROR",
        "message": message,
        "timestamp": chrono::Utc::now()
    });

    (StatusCode::UNAUTHORIZED, Json(error_body)).into_response()
}

// ============================================================================
// SERVER BUILDER UTILITIES
// ============================================================================

/// Builder for API server configuration
pub struct ApiServerBuilder {
    config: ApiServerConfig,
}

impl ApiServerBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: ApiServerConfig::default(),
        }
    }

    #[must_use]
    pub fn host(mut self, host: impl Into<String>) -> Self {
        self.config.host = host.into();
        self
    }

    #[must_use]
    pub const fn port(mut self, port: u16) -> Self {
        self.config.port = port;
        self
    }

    #[must_use]
    pub fn api_config(mut self, config: ApiConfig) -> Self {
        self.config.api_config = config;
        self
    }

    #[must_use]
    pub const fn enable_rest(mut self, enabled: bool) -> Self {
        self.config.enable_rest = enabled;
        self
    }

    #[must_use]
    pub const fn enable_playground(mut self, enabled: bool) -> Self {
        self.config.enable_playground = enabled;
        self
    }

    #[must_use]
    pub fn cors_origins(mut self, origins: Vec<String>) -> Self {
        self.config.cors_origins = origins;
        self
    }

    #[must_use]
    pub const fn request_timeout(mut self, seconds: u64) -> Self {
        self.config.request_timeout_seconds = seconds;
        self
    }

    #[must_use]
    pub const fn max_request_size(mut self, bytes: usize) -> Self {
        self.config.max_request_size_bytes = bytes;
        self
    }

    /// Build the API server with the current configuration
    ///
    /// # Errors
    /// Returns error if server initialization fails
    pub async fn build(
        self,
        database_url: Option<String>,
    ) -> Result<ApiServer, Box<dyn std::error::Error + Send + Sync>> {
        ApiServer::new(database_url, self.config).await
    }

    /// Build the API server with an existing database pool
    pub async fn build_with_pool(
        self,
        pool: PgPool,
    ) -> Result<ApiServer, Box<dyn std::error::Error + Send + Sync>> {
        ApiServer::with_pool(pool, self.config).await
    }
}

impl Default for ApiServerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// NEW ROUTER IMPLEMENTATION
// ============================================================================

/// Create a new router using the refactored handler modules
/// This replaces the old rest::create_router function
pub fn create_new_router(state: ApiState) -> Router<ApiState> {
    let swagger_ui = SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi());

    Router::new()
        // Health and info endpoints (using new handlers as examples)
        .route("/new-health", get(new_health_check))
        .route("/new-info", get(new_api_info))
        // OpenAPI documentation
        .merge(swagger_ui)
        // User management endpoints from user_handlers.rs
        .route(
            crate::config::paths::USER,
            post(crate::handlers::users::users::register_user_endpoint),
        )
        .route(
            crate::config::paths::USERS,
            get(crate::handlers::users::users::list_identities_endpoint),
        )
        .route(
            crate::config::paths::USER_BY_ID,
            get(crate::handlers::users::users::get_user_details_endpoint),
        )
        // State management endpoints from state_handlers.rs
        .route(
            crate::config::paths::STATE_TRANSACTIONS,
            post(crate::handlers::core::state::submit_transaction_endpoint),
        )
        .route(
            crate::config::paths::STATE_TRANSACTIONS_PENDING,
            get(crate::handlers::core::state::get_pending_transactions_endpoint),
        )
        .route(
            crate::config::paths::STATE_BATCHES,
            post(crate::handlers::core::state::create_batch_endpoint),
        )
        // Admin endpoints from admin_handlers.rs
        .route(
            crate::config::paths::ADMIN_KYC_PENDING,
            get(crate::handlers::users::admin::list_pending_kyc_endpoint),
        )
        // TODO: Continue migrating remaining endpoints from rest.rs
        // For now, some endpoints are still handled by rest::create_router
        .with_state(state)
}

/// New placeholder health check endpoint (different from rest.rs version)
async fn new_health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now(),
        "source": "new_handlers"
    }))
}

/// New placeholder API info endpoint
async fn new_api_info() -> impl IntoResponse {
    Json(serde_json::json!({
        "name": "Arda Credit API",
        "version": "v1",
        "status": "running",
        "source": "new_handlers"
    }))
}
