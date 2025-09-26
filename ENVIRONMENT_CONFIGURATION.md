# Environment Configuration Strategy

## Overview

This document outlines the environment configuration strategy to handle different deployment environments (local development, staging, sandbox, production) with appropriate feature flags for contract posting, ZK proof types, and external service usage.

## Environment Hierarchy

### Development (Local)
**Purpose**: Fast iteration, minimal external dependencies, cost-effective testing
- **ZK Proofs**: SP1 Core proofs (local, fast ~8 seconds)
- **Contract Posting**: Disabled (no blockchain interaction)
- **External Services**: Minimized (mock Sindri, no email sending)
- **Database**: Local PostgreSQL
- **Features**: All enabled for testing

### Staging  
**Purpose**: Production-like testing, CI/CD validation, integration testing
- **ZK Proofs**: Groth16 via Sindri (production-ready proofs)
- **Contract Posting**: Enabled to Sepolia testnet
- **External Services**: Real services with test accounts
- **Database**: Cloud PostgreSQL with staging data
- **Features**: MVP + selected advanced features

### Sandbox
**Purpose**: External user testing, demo environment, pre-production validation
- **ZK Proofs**: Groth16 via Sindri 
- **Contract Posting**: Enabled to Sepolia or dedicated testnet
- **External Services**: Production services with sandbox accounts
- **Database**: Production-grade database with synthetic data
- **Features**: MVP features only

### Production
**Purpose**: Live system serving real users and transactions
- **ZK Proofs**: Groth16 via Sindri with production API keys
- **Contract Posting**: Enabled to Ethereum mainnet
- **External Services**: Production services with monitoring
- **Database**: Highly available PostgreSQL with backups
- **Features**: MVP features with gradual rollout of advanced features

## Environment Variables Schema

### Core Environment Configuration
```bash
# Environment Identification
NODE_ENV=development|staging|sandbox|production
ENVIRONMENT_NAME=local-dev|staging|sandbox|prod
DEPLOYMENT_STAGE=dev|staging|sandbox|production

# Feature Flags
MVP_MODE=true|false                  # When true, only MVP features enabled
ADVANCED_FEATURES=false|true         # Advanced analytics, AI, etc.
DEBUG_MODE=true|false                # Enhanced logging and debug endpoints
```

### ZK Proof Configuration
```bash
# Proof Type Selection
PROOF_TYPE=core|groth16               # core for dev/test, groth16 for staging+
ZK_PROOF_ENABLED=true|false           # Master switch for ZK functionality
LOCAL_PROOF_GENERATION=true|false     # Use local SP1 vs external service

# Sindri Configuration (External ZK Service)
ENABLE_SINDRI_PROOFS=false|true       # Use Sindri for proof generation
SINDRI_API_KEY=your_api_key_here      # Only set for staging/sandbox/prod
SINDRI_CIRCUIT_TAG=v1.0.0             # Circuit version
SINDRI_TIMEOUT_SECONDS=300            # Proof generation timeout
```

### Blockchain & Contract Configuration
```bash
# Contract Posting Control
ENABLE_CONTRACT_POSTING=false|true    # Master switch for blockchain interaction
SKIP_CONTRACT_VERIFICATION=true|false # Skip contract calls in development

# Ethereum Network Configuration
ETHEREUM_NETWORK=sepolia|mainnet      # Blockchain network
ETHEREUM_RPC_URL=https://...          # RPC endpoint
ETHEREUM_PRIVATE_KEY=0x...            # Signing key for contract interactions
ETHEREUM_GAS_LIMIT=500000            # Default gas limit
ETHEREUM_GAS_PRICE_GWEI=20           # Gas price in Gwei

# Contract Addresses
ETHEREUM_ARDA_CONTRACT_ADDRESS=0x...  # Main contract address
ETHEREUM_USDC_CONTRACT_ADDRESS=0x...  # USDC contract for deposits/withdrawals
```

### Database Configuration  
```bash
# Database Connection
DATABASE_URL=postgresql://...         # Connection string
DATABASE_MAX_CONNECTIONS=10          # Connection pool size
DATABASE_TIMEOUT_SECONDS=30          # Query timeout

# Migration Control
AUTO_RUN_MIGRATIONS=true|false       # Run migrations on startup
REQUIRE_MIGRATION_LOCK=true|false    # Prevent concurrent migrations
```

### External Services
```bash
# Email Configuration
EMAIL_ENABLED=true|false             # Master switch for email functionality
EMAIL_DEBUG_MODE=false|true          # Log emails instead of sending
EMAIL_SERVICE=aws_ses|sendgrid|debug # Email provider
AWS_SES_REGION=us-east-1            # AWS SES region
AWS_SES_ACCESS_KEY_ID=...           # AWS credentials
AWS_SES_SECRET_ACCESS_KEY=...       # AWS credentials

# API Client Configuration
ARDA_CREDIT_API_URL=http://localhost:8080  # API base URL for CLI
API_TIMEOUT_SECONDS=30               # Default API timeout
```

### Security & Authentication
```bash
# Session Management
SESSION_SECRET=your_session_secret    # Session encryption key
SESSION_DURATION_HOURS=24            # Session lifetime
MAGIC_LINK_EXPIRY_MINUTES=10        # Magic link validity

# Rate Limiting
ENABLE_RATE_LIMITING=true|false      # Enable request rate limiting
RATE_LIMIT_REQUESTS_PER_MINUTE=60   # Requests per minute per IP
```

### Monitoring & Observability
```bash
# Logging Configuration
LOG_LEVEL=debug|info|warn|error      # Logging verbosity
LOG_FORMAT=json|text                 # Log output format
ENABLE_REQUEST_LOGGING=true|false    # Log all HTTP requests

# Metrics & Monitoring
PROMETHEUS_ENABLED=false|true        # Enable Prometheus metrics
METRICS_BIND_ADDRESS=127.0.0.1:9090 # Metrics server address
HEALTH_CHECK_ENABLED=true           # Health check endpoint
```

## Environment-Specific Configurations

### Development Environment (`.env.development`)
```bash
# Development Configuration - Fast iteration, no external costs
NODE_ENV=development
ENVIRONMENT_NAME=local-dev
MVP_MODE=false
ADVANCED_FEATURES=true
DEBUG_MODE=true

# ZK Proof Configuration - Use fast local proofs
PROOF_TYPE=core
ZK_PROOF_ENABLED=true
LOCAL_PROOF_GENERATION=true
ENABLE_SINDRI_PROOFS=false
# SINDRI_API_KEY not set - save costs

# Blockchain Configuration - No real blockchain interaction
ENABLE_CONTRACT_POSTING=false
SKIP_CONTRACT_VERIFICATION=true
ETHEREUM_NETWORK=sepolia
# No need for real RPC URL or private keys in development

# Database - Local PostgreSQL
DATABASE_URL=postgresql://postgres:password@localhost:5432/arda_credit_dev
DATABASE_MAX_CONNECTIONS=5
AUTO_RUN_MIGRATIONS=true

# Email - Debug mode to avoid external service calls
EMAIL_ENABLED=true
EMAIL_DEBUG_MODE=true
EMAIL_SERVICE=debug

# Security - Relaxed for development  
SESSION_DURATION_HOURS=72
MAGIC_LINK_EXPIRY_MINUTES=60

# Monitoring - Enhanced logging for debugging
LOG_LEVEL=debug
LOG_FORMAT=text
ENABLE_REQUEST_LOGGING=true
PROMETHEUS_ENABLED=false
```

### Staging Environment (`.env.staging`)
```bash
# Staging Configuration - Production-like with test data
NODE_ENV=staging
ENVIRONMENT_NAME=staging
MVP_MODE=false
ADVANCED_FEATURES=true
DEBUG_MODE=true

# ZK Proof Configuration - Use production proof type with Sindri
PROOF_TYPE=groth16
ZK_PROOF_ENABLED=true
LOCAL_PROOF_GENERATION=false
ENABLE_SINDRI_PROOFS=true
SINDRI_API_KEY=staging_api_key_here
SINDRI_CIRCUIT_TAG=v1.0.0
SINDRI_TIMEOUT_SECONDS=300

# Blockchain Configuration - Sepolia testnet
ENABLE_CONTRACT_POSTING=true
SKIP_CONTRACT_VERIFICATION=false
ETHEREUM_NETWORK=sepolia
ETHEREUM_RPC_URL=https://sepolia.infura.io/v3/your_project_id
ETHEREUM_PRIVATE_KEY=0x_staging_private_key
ETHEREUM_ARDA_CONTRACT_ADDRESS=0x_staging_contract_address

# Database - Cloud PostgreSQL
DATABASE_URL=postgresql://user:pass@staging-db.amazonaws.com:5432/arda_credit_staging
DATABASE_MAX_CONNECTIONS=10
AUTO_RUN_MIGRATIONS=true
REQUIRE_MIGRATION_LOCK=true

# Email - Real email service with staging templates
EMAIL_ENABLED=true
EMAIL_DEBUG_MODE=false
EMAIL_SERVICE=aws_ses
AWS_SES_REGION=us-east-1
AWS_SES_ACCESS_KEY_ID=staging_access_key
AWS_SES_SECRET_ACCESS_KEY=staging_secret_key

# Security - Production-like settings
SESSION_DURATION_HOURS=24
MAGIC_LINK_EXPIRY_MINUTES=10

# Monitoring - Structured logging
LOG_LEVEL=info
LOG_FORMAT=json
PROMETHEUS_ENABLED=true
METRICS_BIND_ADDRESS=0.0.0.0:9090
```

### Sandbox Environment (`.env.sandbox`)
```bash
# Sandbox Configuration - External testing environment
NODE_ENV=sandbox
ENVIRONMENT_NAME=sandbox  
MVP_MODE=true           # Only MVP features for external users
ADVANCED_FEATURES=false
DEBUG_MODE=false

# ZK Proof Configuration - Production proofs
PROOF_TYPE=groth16
ZK_PROOF_ENABLED=true
ENABLE_SINDRI_PROOFS=true
SINDRI_API_KEY=sandbox_api_key_here
SINDRI_CIRCUIT_TAG=v1.0.0

# Blockchain Configuration - Dedicated testnet or Sepolia
ENABLE_CONTRACT_POSTING=true
ETHEREUM_NETWORK=sepolia
ETHEREUM_RPC_URL=https://sepolia.infura.io/v3/sandbox_project_id
ETHEREUM_PRIVATE_KEY=0x_sandbox_private_key
ETHEREUM_ARDA_CONTRACT_ADDRESS=0x_sandbox_contract_address

# Database - Production-grade with synthetic data
DATABASE_URL=postgresql://user:pass@sandbox-db.amazonaws.com:5432/arda_credit_sandbox
DATABASE_MAX_CONNECTIONS=15
AUTO_RUN_MIGRATIONS=false  # Migrations handled separately
REQUIRE_MIGRATION_LOCK=true

# Email - Production email service
EMAIL_ENABLED=true
EMAIL_DEBUG_MODE=false
EMAIL_SERVICE=aws_ses
AWS_SES_REGION=us-east-1

# Security - Production settings
SESSION_DURATION_HOURS=12
MAGIC_LINK_EXPIRY_MINUTES=10
ENABLE_RATE_LIMITING=true
RATE_LIMIT_REQUESTS_PER_MINUTE=30

# Monitoring - Production monitoring
LOG_LEVEL=info
LOG_FORMAT=json
PROMETHEUS_ENABLED=true
HEALTH_CHECK_ENABLED=true
```

### Production Environment (`.env.production`)
```bash
# Production Configuration - Live system
NODE_ENV=production
ENVIRONMENT_NAME=prod
MVP_MODE=true           # Start with MVP, gradually enable advanced features
ADVANCED_FEATURES=false
DEBUG_MODE=false

# ZK Proof Configuration - Production Sindri with monitoring
PROOF_TYPE=groth16
ZK_PROOF_ENABLED=true
ENABLE_SINDRI_PROOFS=true
SINDRI_API_KEY=production_api_key_here
SINDRI_CIRCUIT_TAG=v1.0.0
SINDRI_TIMEOUT_SECONDS=600  # Longer timeout for production

# Blockchain Configuration - Ethereum mainnet
ENABLE_CONTRACT_POSTING=true
ETHEREUM_NETWORK=mainnet
ETHEREUM_RPC_URL=https://mainnet.infura.io/v3/production_project_id
ETHEREUM_PRIVATE_KEY=0x_production_private_key  # Highly secure key management
ETHEREUM_ARDA_CONTRACT_ADDRESS=0x_production_contract_address
ETHEREUM_GAS_PRICE_GWEI=25  # Conservative gas pricing

# Database - Highly available PostgreSQL
DATABASE_URL=postgresql://user:pass@prod-db.amazonaws.com:5432/arda_credit_prod
DATABASE_MAX_CONNECTIONS=25
AUTO_RUN_MIGRATIONS=false  # Manual migration process
REQUIRE_MIGRATION_LOCK=true

# Email - Production AWS SES
EMAIL_ENABLED=true
EMAIL_DEBUG_MODE=false
EMAIL_SERVICE=aws_ses
AWS_SES_REGION=us-east-1

# Security - Maximum security settings
SESSION_DURATION_HOURS=8
MAGIC_LINK_EXPIRY_MINUTES=5
ENABLE_RATE_LIMITING=true
RATE_LIMIT_REQUESTS_PER_MINUTE=20

# Monitoring - Full observability stack
LOG_LEVEL=warn
LOG_FORMAT=json
PROMETHEUS_ENABLED=true
METRICS_BIND_ADDRESS=0.0.0.0:9090
HEALTH_CHECK_ENABLED=true
```

## Implementation in Code

### Environment Configuration Loading
```rust
// api/src/config.rs
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    // Environment identification
    pub node_env: Environment,
    pub environment_name: String,
    pub mvp_mode: bool,
    pub advanced_features: bool,
    pub debug_mode: bool,
    
    // ZK Proof configuration
    pub proof_type: ProofType,
    pub zk_proof_enabled: bool,
    pub local_proof_generation: bool,
    pub enable_sindri_proofs: bool,
    pub sindri_api_key: Option<String>,
    pub sindri_circuit_tag: String,
    
    // Blockchain configuration
    pub enable_contract_posting: bool,
    pub skip_contract_verification: bool,
    pub ethereum_network: BlockchainNetwork,
    pub ethereum_rpc_url: Option<String>,
    pub ethereum_private_key: Option<String>,
    pub ethereum_arda_contract_address: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub enum Environment {
    Development,
    Staging,
    Sandbox,
    Production,
}

#[derive(Debug, Clone, Deserialize)]
pub enum ProofType {
    Core,     // Fast local proofs for development
    Groth16,  // Production-ready proofs via Sindri
}

#[derive(Debug, Clone, Deserialize)]
pub enum BlockchainNetwork {
    Sepolia,
    Mainnet,
}

impl AppConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        let mut config = Config::new();
        config.merge(Environment::with_prefix("ARDA"))?;
        config.try_into()
    }
    
    pub fn should_post_to_contract(&self) -> bool {
        self.enable_contract_posting && 
        !matches!(self.node_env, Environment::Development) &&
        self.ethereum_rpc_url.is_some() &&
        self.ethereum_private_key.is_some()
    }
    
    pub fn should_use_sindri(&self) -> bool {
        self.enable_sindri_proofs && 
        self.sindri_api_key.is_some() &&
        matches!(self.proof_type, ProofType::Groth16)
    }
    
    pub fn should_generate_local_proofs(&self) -> bool {
        self.local_proof_generation || 
        matches!(self.proof_type, ProofType::Core) ||
        !self.should_use_sindri()
    }
}
```

### Feature Flag Implementation
```rust
// api/src/features.rs
pub struct FeatureFlags {
    config: AppConfig,
}

impl FeatureFlags {
    pub fn new(config: AppConfig) -> Self {
        Self { config }
    }
    
    pub fn is_mvp_mode(&self) -> bool {
        self.config.mvp_mode || 
        matches!(self.config.node_env, Environment::Production | Environment::Sandbox)
    }
    
    pub fn enable_advanced_analytics(&self) -> bool {
        self.config.advanced_features && 
        !self.is_mvp_mode()
    }
    
    pub fn enable_ai_features(&self) -> bool {
        self.config.advanced_features && 
        !self.is_mvp_mode() &&
        matches!(self.config.node_env, Environment::Development | Environment::Staging)
    }
    
    pub fn enable_credit_lines(&self) -> bool {
        self.config.advanced_features && 
        !self.is_mvp_mode()
    }
    
    pub fn should_skip_expensive_operations(&self) -> bool {
        matches!(self.config.node_env, Environment::Development) ||
        self.config.debug_mode
    }
}
```

### Conditional Service Initialization
```rust
// api/src/lib.rs  
pub async fn create_app_with_config(config: AppConfig) -> Result<App, AppError> {
    let features = FeatureFlags::new(config.clone());
    let mut app_builder = AppBuilder::new();
    
    // Core services (always enabled)
    app_builder
        .with_database(&config.database_url)
        .with_authentication()
        .with_deal_management();
    
    // ZK Proof service - conditional configuration
    if config.should_use_sindri() {
        app_builder.with_sindri_proof_service(&config.sindri_api_key.unwrap());
    } else {
        app_builder.with_local_proof_service();
    }
    
    // Contract posting - conditional
    if config.should_post_to_contract() {
        app_builder.with_ethereum_client(
            &config.ethereum_rpc_url.unwrap(),
            &config.ethereum_private_key.unwrap()
        );
    }
    
    // Advanced features - conditional
    if features.enable_advanced_analytics() {
        app_builder.with_analytics_service();
    }
    
    if features.enable_ai_features() {
        app_builder.with_ai_analysis_service();
    }
    
    app_builder.build().await
}
```

### Environment Detection Utilities
```rust
// utils/src/environment.rs
pub fn detect_environment() -> Environment {
    match std::env::var("NODE_ENV").as_deref() {
        Ok("production") => Environment::Production,
        Ok("staging") => Environment::Staging,
        Ok("sandbox") => Environment::Sandbox,
        _ => Environment::Development,
    }
}

pub fn is_cost_conscious_environment() -> bool {
    matches!(detect_environment(), Environment::Development)
}

pub fn requires_high_security() -> bool {
    matches!(detect_environment(), Environment::Production | Environment::Sandbox)
}
```

## Deployment & Infrastructure

### Docker Compose Environment Override
```yaml
# docker-compose.development.yml
version: '3.8'
services:
  arda-credit:
    environment:
      - NODE_ENV=development
      - PROOF_TYPE=core
      - ENABLE_CONTRACT_POSTING=false
      - EMAIL_DEBUG_MODE=true
    ports:
      - "8080:8080"
      - "9090:9090"  # Prometheus metrics for development debugging

# docker-compose.staging.yml  
version: '3.8'
services:
  arda-credit:
    environment:
      - NODE_ENV=staging
      - PROOF_TYPE=groth16
      - ENABLE_CONTRACT_POSTING=true
      - ETHEREUM_NETWORK=sepolia
    env_file:
      - .env.staging
```

### Kubernetes ConfigMaps
```yaml
# k8s/configmap-production.yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: arda-credit-config-prod
data:
  NODE_ENV: "production"
  ENVIRONMENT_NAME: "prod"
  MVP_MODE: "true"
  PROOF_TYPE: "groth16"
  ENABLE_CONTRACT_POSTING: "true"
  ETHEREUM_NETWORK: "mainnet"
  LOG_LEVEL: "warn"
  LOG_FORMAT: "json"
```

### Environment Validation
```rust
// api/src/startup.rs
pub fn validate_environment_config(config: &AppConfig) -> Result<(), ConfigError> {
    // Validate production requirements
    if matches!(config.node_env, Environment::Production) {
        if config.debug_mode {
            return Err(ConfigError::new("Debug mode cannot be enabled in production"));
        }
        
        if config.should_post_to_contract() && config.ethereum_private_key.is_none() {
            return Err(ConfigError::new("Production requires ethereum private key"));
        }
    }
    
    // Validate Sindri configuration
    if config.enable_sindri_proofs && config.sindri_api_key.is_none() {
        return Err(ConfigError::new("Sindri API key required when Sindri proofs enabled"));
    }
    
    // Validate blockchain configuration
    if config.enable_contract_posting {
        if config.ethereum_rpc_url.is_none() {
            return Err(ConfigError::new("Ethereum RPC URL required for contract posting"));
        }
        if config.ethereum_arda_contract_address.is_none() {
            return Err(ConfigError::new("Contract address required for contract posting"));
        }
    }
    
    Ok(())
}
```

This environment configuration strategy provides clear separation of concerns across deployment environments while optimizing for cost-effectiveness in development and reliability in production.
