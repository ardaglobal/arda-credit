use crate::{
    config::Config,
    contracts::{ContractAddresses, ILoan, ILoanInstance, ISP1Verifier},
    error::{EthereumError, Result},
    types::{
        BatchStateUpdate, InclusionProof, NetworkStats, ProofVerificationResult, StateHistory,
        StateResponse, StateUpdate,
    },
};
use alloy_network::EthereumWallet;
use alloy_primitives::{keccak256, Address, Bytes, FixedBytes, U256};
use alloy_provider::{
    fillers::{
        BlobGasFiller, ChainIdFiller, FillProvider, GasFiller, JoinFill, NonceFiller, WalletFiller,
    },
    Identity, Provider, ProviderBuilder, RootProvider,
};
use alloy_rpc_types_eth::{Filter, TransactionReceipt};
use alloy_signer_local::PrivateKeySigner;
use alloy_sol_types::SolEvent;
use hex;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tokio::time::{sleep, Duration};
use tracing::{debug, error, info, warn};

#[cfg(feature = "database")]
use uuid;

// Use a simpler provider type that works with the current Alloy version
type EthProvider = FillProvider<
    JoinFill<
        JoinFill<
            Identity,
            JoinFill<GasFiller, JoinFill<BlobGasFiller, JoinFill<NonceFiller, ChainIdFiller>>>,
        >,
        WalletFiller<EthereumWallet>,
    >,
    RootProvider,
>;

#[cfg(feature = "database")]
use crate::cache::EthereumCache;
#[cfg(feature = "database")]
use sqlx;

/// Event data structures for comprehensive event handling
#[derive(Debug, Clone)]
pub enum LoanEvent {
    StateUpdated {
        state_id: FixedBytes<32>,
        new_state: FixedBytes<32>,
        proof_id: FixedBytes<32>,
        updater: Address,
        timestamp: u64,
        block_number: u64,
        tx_hash: FixedBytes<32>,
        log_index: u64,
    },
    BatchStateUpdated {
        state_ids: Vec<FixedBytes<32>>,
        new_states: Vec<FixedBytes<32>>,
        updater: Address,
        timestamp: u64,
        block_number: u64,
        tx_hash: FixedBytes<32>,
        log_index: u64,
    },
    ProofStored {
        proof_id: FixedBytes<32>,
        state_id: FixedBytes<32>,
        submitter: Address,
        timestamp: u64,
        block_number: u64,
        tx_hash: FixedBytes<32>,
        log_index: u64,
    },
    ProofVerified {
        proof_id: FixedBytes<32>,
        success: bool,
        result: Bytes,
        timestamp: u64,
        block_number: u64,
        tx_hash: FixedBytes<32>,
        log_index: u64,
    },
    StateReadRequested {
        state_id: FixedBytes<32>,
        reader: Address,
        timestamp: u64,
        block_number: u64,
        tx_hash: FixedBytes<32>,
        log_index: u64,
    },
    ProofReadRequested {
        proof_id: FixedBytes<32>,
        reader: Address,
        timestamp: u64,
        block_number: u64,
        tx_hash: FixedBytes<32>,
        log_index: u64,
    },
    // Loan-specific events
    Deposit {
        user: Address,
        amount: U256,
        loan_id: FixedBytes<32>,
        timestamp: u64,
        block_number: u64,
        tx_hash: FixedBytes<32>,
        log_index: u64,
    },
    LoanPayment {
        loan_id: FixedBytes<32>,
        amount: U256,
        payer: Address,
        timestamp: u64,
        block_number: u64,
        tx_hash: FixedBytes<32>,
        log_index: u64,
    },
    LoanClosed {
        loan_id: FixedBytes<32>,
        final_amount: U256,
        timestamp: u64,
        block_number: u64,
        tx_hash: FixedBytes<32>,
        log_index: u64,
    },
    Withdrawal {
        user: Address,
        amount: U256,
        timestamp: u64,
        block_number: u64,
        tx_hash: FixedBytes<32>,
        log_index: u64,
    },
}

/// Event callback function type
pub type EventCallback = Arc<dyn Fn(LoanEvent) + Send + Sync>;

/// Event filter configuration
#[derive(Debug, Clone, Default)]
pub struct EventFilter {
    pub event_types: Option<Vec<String>>,
    pub state_ids: Option<Vec<FixedBytes<32>>>,
    pub addresses: Option<Vec<Address>>,
    pub from_block: Option<u64>,
    pub to_block: Option<u64>,
}

/// Event subscription handle
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SubscriptionId(pub String);

/// Event listener configuration
#[derive(Debug, Clone)]
pub struct EventListenerConfig {
    pub poll_interval: Duration,
    pub max_blocks_per_query: u64,
    pub enable_persistence: bool,
    pub retry_attempts: u32,
    pub retry_delay: Duration,
}

impl Default for EventListenerConfig {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_secs(12),
            max_blocks_per_query: 1000,
            enable_persistence: true,
            retry_attempts: 3,
            retry_delay: Duration::from_secs(5),
        }
    }
}

pub struct EthereumClient {
    #[allow(dead_code)]
    config: Config,
    http_provider: EthProvider,
    #[allow(dead_code)]
    contracts: ContractAddresses,
    signer: PrivateKeySigner,

    #[cfg(feature = "database")]
    cache: Option<EthereumCache>,

    // Event system components
    event_callbacks: Arc<RwLock<HashMap<SubscriptionId, (EventFilter, EventCallback)>>>,
    event_broadcaster: broadcast::Sender<LoanEvent>,
    #[allow(dead_code)]
    event_receiver: broadcast::Receiver<LoanEvent>,
    listener_config: EventListenerConfig,
    last_processed_block: Arc<RwLock<u64>>,
}

impl EthereumClient {
    pub async fn new(config: Config) -> Result<Self> {
        config.validate()?;

        let signer_config = config
            .signer
            .as_ref()
            .ok_or_else(|| EthereumError::Config("Signer required".to_string()))?;

        let signer = PrivateKeySigner::from_bytes(&FixedBytes::<32>::try_from(
            hex::decode(&signer_config.private_key)?.as_slice(),
        )?)
        .map_err(|e| EthereumError::Signer(e.to_string()))?;

        let wallet = EthereumWallet::from(signer.clone());

        // Create provider with wallet for signing
        let http_provider = ProviderBuilder::new()
            .wallet(wallet)
            .connect_http(config.network.rpc_url.clone());

        let contracts = ContractAddresses::new(
            config.contract.arda_credit_contract,
            config.contract.verifier_contract,
        );

        #[cfg(feature = "database")]
        let cache = if let Ok(database_url) = std::env::var("DATABASE_URL") {
            let pool = sqlx::PgPool::connect(&database_url).await?;
            Some(EthereumCache::new(pool))
        } else {
            None
        };

        // Initialize event system
        let (event_broadcaster, event_receiver) = broadcast::channel(1000);
        let event_callbacks = Arc::new(RwLock::new(HashMap::new()));
        let listener_config = EventListenerConfig::default();
        let last_processed_block = Arc::new(RwLock::new(0));

        // Create a temporary client instance to validate verification key
        let temp_client = Self {
            config: config.clone(),
            http_provider: http_provider.clone(),
            contracts: contracts.clone(),
            signer: signer.clone(),

            #[cfg(feature = "database")]
            cache: cache.clone(),

            event_callbacks: Arc::clone(&event_callbacks),
            event_broadcaster: event_broadcaster.clone(),
            event_receiver: event_receiver.resubscribe(),
            listener_config: listener_config.clone(),
            last_processed_block: Arc::clone(&last_processed_block),
        };

        // Validate verification key compatibility on startup
        if let Err(e) = temp_client.validate_verification_key_compatibility().await {
            error!(
                "Verification key validation failed during client initialization: {}",
                e
            );
            return Err(e);
        }

        Ok(Self {
            config,
            http_provider,
            contracts,
            signer,

            #[cfg(feature = "database")]
            cache,

            event_callbacks,
            event_broadcaster,
            event_receiver,
            listener_config,
            last_processed_block,
        })
    }

    pub async fn verify_proof(&self, public_values: Bytes, proof: Bytes) -> Result<i32> {
        let contract = ILoan::new(self.contracts.loan, &self.http_provider);
        let call_builder = contract
            .verifyLoanProof(public_values.clone(), proof.clone())
            .from(self.signer.address());

        let tx_result = call_builder.call().await.map_err(|e| {
            error!("Failed to send verify proof transaction: {e}");
            EthereumError::from_contract_error(&format!("Transaction failed: {e}"))
        })?;

        Ok(tx_result)
    }

    pub async fn update_state(
        &self,
        state_id: FixedBytes<32>,
        new_state_root: FixedBytes<32>,
        proof: Bytes,
        public_values: Bytes,
    ) -> Result<StateUpdate> {
        // Create contract instance
        let contract = ILoan::new(self.contracts.loan, &self.http_provider);

        // Send transaction (wallet handles signing automatically)
        let tx_result = contract
            .updateState(
                state_id,
                new_state_root,
                proof.clone(),
                public_values.clone(),
            )
            .send()
            .await
            .map_err(|e| {
                error!("Failed to send state update transaction: {e}");
                EthereumError::from_contract_error(&format!("Transaction failed: {e}"))
            })?;

        info!("State update transaction sent: {}", tx_result.tx_hash());

        // Wait for confirmation
        let receipt = tx_result.get_receipt().await.map_err(|e| {
            error!("Failed to get transaction receipt: {e}");
            EthereumError::Transaction(format!("Receipt error: {e}"))
        })?;

        // Verify transaction was successful
        if !receipt.status() {
            error!("Transaction failed on-chain with status: false");
            return Err(EthereumError::Transaction(
                "Transaction failed on-chain".to_string(),
            ));
        }

        let block_number = receipt.block_number.unwrap_or(0);
        let tx_hash = receipt.transaction_hash;

        info!(
            "State update confirmed successfully in block {}: {}",
            block_number,
            hex::encode(tx_hash.as_slice())
        );

        let state_update = StateUpdate {
            state_id,
            new_state_root,
            proof,
            public_values,
            block_number: Some(block_number),
            transaction_hash: Some(tx_hash),
        };

        #[cfg(feature = "database")]
        if let Some(cache) = &self.cache {
            cache.store_state_update(&state_update).await?;
        }

        Ok(state_update)
    }

    pub async fn is_authorized(&self, account: Address) -> Result<bool> {
        let contract = ILoan::new(self.contracts.loan, &self.http_provider);
        let call_builder = contract.isAuthorized(account).from(self.signer.address());
        let tx_result = call_builder.call().await.map_err(|e| {
            error!("Failed to send is authorized transaction: {e}");
            EthereumError::from_contract_error(&format!("Transaction failed: {e}"))
        })?;

        Ok(tx_result)
    }

    pub async fn check_proof_exists(&self, proof: Bytes) -> Result<bool> {
        let contract = ILoan::new(self.contracts.loan, &self.http_provider);
        let call_builder = contract.proofExists(proof).from(self.signer.address());
        let tx_result = call_builder.call().await.map_err(|e| {
            error!("Failed to send check proof exists transaction: {e}");
            EthereumError::from_contract_error(&format!("Transaction failed: {e}"))
        })?;
        Ok(tx_result)
    }

    /// Validate that the local verification key is compatible with the deployed contract
    #[allow(clippy::cognitive_complexity)]
    pub async fn validate_verification_key_compatibility(&self) -> Result<()> {
        info!("🔍 Validating verification key compatibility with smart contract...");

        // First, get the contract's verification key
        let contract = ILoan::new(self.contracts.loan, &self.http_provider);
        let contract_vkey = contract
            .getProgramVerificationKey()
            .call()
            .await
            .map_err(|e| {
                error!("Failed to query contract verification key: {e}");
                EthereumError::from_contract_error(&format!("Failed to get contract vkey: {e}"))
            })?;

        info!(
            "📍 Smart contract verification key: 0x{}",
            hex::encode(contract_vkey)
        );

        // Get the local verification key from vk.json if it exists
        let local_vkey = match Self::load_local_verification_key() {
            Ok(vkey) => {
                info!(
                    "📁 Local verification key (from vk.json): 0x{}",
                    hex::encode(vkey)
                );
                Some(vkey)
            }
            Err(e) => {
                warn!("⚠️ Could not load local verification key: {}", e);
                warn!("   This is optional - will skip local key validation");
                None
            }
        };

        // If we have a local key, validate compatibility
        if let Some(local_vkey) = local_vkey {
            if contract_vkey != local_vkey {
                let error_msg = format!(
                    "Verification key mismatch!\n\
                    Smart Contract VKey: 0x{}\n\
                    Local VKey (vk.json): 0x{}\n\
                    \n\
                    This means proofs generated with your current circuit will be REJECTED by the smart contract.\n\
                    \n\
                    Solutions:\n\
                    1. Deploy a new contract with the correct verification key: 0x{}\n\
                    2. Use a circuit that matches the contract's verification key: 0x{}",
                    hex::encode(contract_vkey),
                    hex::encode(local_vkey),
                    hex::encode(local_vkey),
                    hex::encode(contract_vkey)
                );

                error!("❌ {}", error_msg);
                return Err(EthereumError::Config(error_msg));
            }

            info!("✅ Verification keys match! Proofs will be compatible with the smart contract.");
        }

        // Test the contract's validation function with a mock proof
        match self.test_contract_validation_function(contract_vkey).await {
            Ok(()) => {
                info!("✅ Smart contract validation functions are working correctly");
            }
            Err(e) => {
                warn!(
                    "⚠️ Contract validation test failed (this is non-critical): {}",
                    e
                );
            }
        }

        info!("🎉 Verification key validation completed successfully!");
        Ok(())
    }

    /// Load verification key from local vk.json file
    fn load_local_verification_key() -> Result<FixedBytes<32>> {
        use serde_json::Value;

        let vk_path = std::path::Path::new("vk.json");
        if !vk_path.exists() {
            return Err(EthereumError::Config("vk.json file not found".to_string()));
        }

        let vk_content = std::fs::read_to_string(vk_path)
            .map_err(|e| EthereumError::Config(format!("Failed to read vk.json: {e}")))?;

        let vk_data: Value = serde_json::from_str(&vk_content)
            .map_err(|e| EthereumError::Config(format!("Failed to parse vk.json: {e}")))?;

        // Extract the commit value array (8 32-bit integers)
        let commit_values = vk_data
            .get("vk")
            .and_then(|vk| vk.get("commit"))
            .and_then(|commit| commit.get("value"))
            .and_then(|value| value.as_array())
            .ok_or_else(|| {
                EthereumError::Config("Invalid vk.json format: missing vk.commit.value".to_string())
            })?;

        if commit_values.len() != 8 {
            return Err(EthereumError::Config(format!(
                "Invalid vk.json format: expected 8 values, got {}",
                commit_values.len()
            )));
        }

        // Convert each 32-bit integer to 4 bytes in little-endian format
        let mut bytes = Vec::with_capacity(32);
        for value in commit_values {
            let u64_value = value
                .as_u64()
                .ok_or_else(|| EthereumError::Config("Invalid integer in vk.json".to_string()))?;

            if u64_value > u64::from(u32::MAX) {
                return Err(EthereumError::Config(
                    "Integer value in vk.json exceeds u32::MAX".to_string(),
                ));
            }

            let int32 = u32::try_from(u64_value).map_err(|_| {
                EthereumError::Config("Integer value in vk.json exceeds u32::MAX".to_string())
            })?;

            // Convert to little-endian bytes
            bytes.extend_from_slice(&int32.to_le_bytes());
        }

        // Convert to FixedBytes<32>
        let vkey_bytes: [u8; 32] = bytes
            .try_into()
            .map_err(|_| EthereumError::Config("Failed to convert vkey to 32 bytes".to_string()))?;

        Ok(FixedBytes::from(vkey_bytes))
    }

    /// Test the contract's validation function with mock data
    async fn test_contract_validation_function(&self, contract_vkey: FixedBytes<32>) -> Result<()> {
        let contract = ILoan::new(self.contracts.loan, &self.http_provider);

        let mock_proof = Bytes::from(vec![0x12, 0x34, 0x56, 0x78]);
        let mock_public_values = Bytes::from(vec![0xab, 0xcd, 0xef]);

        let validation_result = contract
            .validateProofCompatibility(contract_vkey, mock_public_values, mock_proof)
            .call()
            .await
            .map_err(|e| {
                EthereumError::from_contract_error(&format!("Contract validation call failed: {e}"))
            })?;

        let is_valid = validation_result.isValid;
        let message = validation_result.message;

        if is_valid {
            info!("✅ Contract validation test passed: {}", message);
        } else {
            warn!("⚠️ Contract validation test returned false: {}", message);
        }

        Ok(())
    }

    pub async fn batch_update_states(
        &self,
        updates: Vec<(FixedBytes<32>, FixedBytes<32>, Bytes, Bytes)>,
    ) -> Result<BatchStateUpdate> {
        if updates.is_empty() {
            return Err(EthereumError::Config("No updates provided".to_string()));
        }

        let mut state_ids = Vec::new();
        let mut new_state_roots = Vec::new();
        let mut proofs = Vec::new();
        let mut results = Vec::new();

        for (state_id, new_state_root, proof, result) in updates {
            state_ids.push(state_id);
            new_state_roots.push(new_state_root);
            proofs.push(proof);
            results.push(result);
        }

        info!("Batch updating {} states", state_ids.len());

        // Create contract instance
        let contract = ILoan::new(self.contracts.loan, &self.http_provider);

        // Send batch transaction (wallet handles signing automatically)
        let tx_result = contract
            .batchUpdateStates(
                state_ids.clone(),
                new_state_roots.clone(),
                proofs.clone(),
                results.clone(),
            )
            .send()
            .await
            .map_err(|e| {
                error!("Failed to send batch update transaction: {e}");
                EthereumError::from_contract_error(&format!("Batch transaction failed: {e}"))
            })?;

        info!("Batch update transaction sent: {}", tx_result.tx_hash());

        // Wait for confirmation
        let receipt = tx_result.get_receipt().await.map_err(|e| {
            error!("Failed to get batch transaction receipt: {e}");
            EthereumError::Transaction(format!("Batch receipt error: {e}"))
        })?;

        let block_number = receipt.block_number.unwrap_or(0);
        let tx_hash = receipt.transaction_hash;
        let gas_used = U256::from(receipt.gas_used);

        info!(
            "Batch update confirmed in block {}: {} (gas used: {})",
            block_number,
            hex::encode(tx_hash.as_slice()),
            gas_used
        );

        // For now, assume all updates succeeded
        // TODO: Parse transaction logs to determine individual success/failure
        let success_flags = vec![true; state_ids.len()];

        let batch_update = BatchStateUpdate {
            state_ids,
            new_state_roots,
            proofs,
            results,
            transaction_hash: tx_hash,
            block_number,
            gas_used,
            success_flags,
        };

        Ok(batch_update)
    }

    pub async fn get_current_state(
        &self,
        state_id: FixedBytes<32>,
    ) -> Result<Option<StateResponse>> {
        let contract = ILoan::new(self.contracts.loan, &self.http_provider);

        let state_root = contract
            .getCurrentState(state_id)
            .call()
            .await
            .map_err(|e| {
                warn!("Failed to get current state: {e}");
                EthereumError::from_contract_error(&format!("State query failed: {e}"))
            })?;

        let current_block = self.http_provider.get_block_number().await?;

        // Get the actual block timestamp
        let timestamp = if let Ok(Some(block)) = self
            .http_provider
            .get_block_by_number(current_block.into())
            .await
        {
            block.header.timestamp
        } else {
            // Fallback to current system time if block timestamp unavailable
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        };

        // Try to find associated proof_id from recent state updates
        #[cfg(feature = "database")]
        let proof_id = if let Some(cache) = &self.cache {
            if let Ok(updates) = cache.get_state_updates_by_state_id(state_id, Some(1)).await {
                updates.first().and_then(|update| {
                    // Extract proof_id from proof bytes (simplified approach)
                    if !update.proof.is_empty() {
                        Some(FixedBytes::from_slice(&keccak256(&update.proof).0))
                    } else {
                        None
                    }
                })
            } else {
                None
            }
        } else {
            None
        };

        #[cfg(not(feature = "database"))]
        let proof_id = None;

        Ok(Some(StateResponse {
            state_id,
            state_root,
            block_number: current_block,
            timestamp,
            proof_id,
        }))
    }

    pub async fn get_previous_state_root(
        &self,
        state_id: FixedBytes<32>,
    ) -> Result<FixedBytes<32>> {
        let contract = ILoan::new(self.contracts.loan, &self.http_provider);

        let previous_state_root = contract
            .getPreviousStateRoot(state_id)
            .call()
            .await
            .map_err(|e| {
                warn!("Failed to get previous state root: {e}");
                EthereumError::from_contract_error(&format!("Previous state query failed: {e}"))
            })?;

        Ok(previous_state_root)
    }

    /// Get the previous total balance from the latest batch public values
    pub async fn get_previous_total_balance(&self, state_id: FixedBytes<32>) -> Result<u128> {
        let contract = ILoan::new(self.contracts.loan, &self.http_provider);

        let previous_balance = contract
            .getPreviousTotalBalance(state_id)
            .call()
            .await
            .map_err(|e| {
                warn!("Failed to get previous total balance: {e}");
                EthereumError::from_contract_error(&format!("Previous balance query failed: {e}"))
            })?;

        Ok(previous_balance)
    }

    /// Get the current total balance from the latest batch public values
    pub async fn get_current_total_balance(&self, state_id: FixedBytes<32>) -> Result<u128> {
        let contract = ILoan::new(self.contracts.loan, &self.http_provider);

        let current_balance = contract
            .getCurrentTotalBalance(state_id)
            .call()
            .await
            .map_err(|e| {
                warn!("Failed to get current total balance: {e}");
                EthereumError::from_contract_error(&format!("Current balance query failed: {e}"))
            })?;

        Ok(current_balance)
    }

    /// Get the previous state root from the latest batch public values
    pub async fn get_previous_state_root_from_batch(
        &self,
        state_id: FixedBytes<32>,
    ) -> Result<FixedBytes<32>> {
        let contract = ILoan::new(self.contracts.loan, &self.http_provider);

        let previous_state_root = contract
            .getPreviousStateRootFromBatch(state_id)
            .call()
            .await
            .map_err(|e| {
                warn!("Failed to get previous state root from batch: {e}");
                EthereumError::from_contract_error(&format!(
                    "Previous state root from batch query failed: {e}"
                ))
            })?;

        Ok(previous_state_root)
    }

    /// Get the new state root from the latest batch public values
    pub async fn get_new_state_root_from_batch(
        &self,
        state_id: FixedBytes<32>,
    ) -> Result<FixedBytes<32>> {
        let contract = ILoan::new(self.contracts.loan, &self.http_provider);

        let new_state_root = contract
            .getNewStateRootFromBatch(state_id)
            .call()
            .await
            .map_err(|e| {
                warn!("Failed to get new state root from batch: {e}");
                EthereumError::from_contract_error(&format!(
                    "New state root from batch query failed: {e}"
                ))
            })?;

        Ok(new_state_root)
    }

    #[cfg(feature = "database")]
    pub fn with_cache(mut self, cache: EthereumCache) -> Result<Self> {
        self.cache = Some(cache);
        Ok(self)
    }

    /// Subscribe to specific events with callback
    pub async fn subscribe_to_events(
        &self,
        filter: EventFilter,
        callback: EventCallback,
    ) -> Result<SubscriptionId> {
        #[cfg(feature = "database")]
        let subscription_id = SubscriptionId(format!("sub_{}", uuid::Uuid::new_v4()));
        #[cfg(not(feature = "database"))]
        let subscription_id = SubscriptionId(format!(
            "sub_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        ));

        self.event_callbacks
            .write()
            .await
            .insert(subscription_id.clone(), (filter, callback));

        info!("Created event subscription: {:?}", subscription_id);
        Ok(subscription_id)
    }

    /// Unsubscribe from events
    pub async fn unsubscribe(&self, subscription_id: &SubscriptionId) -> Result<bool> {
        let removed = self
            .event_callbacks
            .write()
            .await
            .remove(subscription_id)
            .is_some();

        if removed {
            info!("Removed event subscription: {:?}", subscription_id);
        }

        Ok(removed)
    }

    /// Get global event stream
    #[must_use]
    pub fn get_event_stream(&self) -> broadcast::Receiver<LoanEvent> {
        self.event_broadcaster.subscribe()
    }

    /// Start comprehensive event monitoring with enhanced capabilities
    #[allow(clippy::cognitive_complexity)]
    pub async fn start_event_monitoring(&self) -> Result<()> {
        info!(
            "Starting enhanced event monitoring for contract: {}",
            self.contracts.loan
        );

        // Initialize last processed block if not set
        {
            let mut last_block = self.last_processed_block.write().await;
            if *last_block == 0 {
                *last_block = self.http_provider.get_block_number().await?;
                info!("Initialized event monitoring from block: {}", *last_block);
            }
        }

        // Start monitoring loop
        loop {
            match self.process_new_events().await {
                Ok(processed_count) => {
                    if processed_count > 0 {
                        debug!("Processed {} new events", processed_count);
                    }
                }
                Err(e) => {
                    error!("Error in event monitoring: {}", e);
                    // Retry after delay
                    sleep(self.listener_config.retry_delay).await;
                }
            }

            sleep(self.listener_config.poll_interval).await;
        }
    }

    /// Process new events since last check
    async fn process_new_events(&self) -> Result<usize> {
        let current_block = self.http_provider.get_block_number().await?;
        let last_processed = {
            let last_block = self.last_processed_block.read().await;
            *last_block
        };

        if current_block <= last_processed {
            return Ok(0);
        }

        let from_block = last_processed + 1;
        let to_block = std::cmp::min(
            current_block,
            last_processed + self.listener_config.max_blocks_per_query,
        );

        debug!(
            "Processing events from block {} to {}",
            from_block, to_block
        );

        let events = self.fetch_events_in_range(from_block, to_block).await?;
        let event_count = events.len();

        // Process events and notify subscribers
        for event in events {
            self.process_and_dispatch_event(event).await;
        }

        // Update last processed block
        {
            let mut last_block = self.last_processed_block.write().await;
            *last_block = to_block;
        }

        Ok(event_count)
    }

    /// Fetch events in block range
    async fn fetch_events_in_range(
        &self,
        from_block: u64,
        to_block: u64,
    ) -> Result<Vec<LoanEvent>> {
        let filter = Filter::new()
            .address(self.contracts.loan)
            .from_block(from_block)
            .to_block(to_block);

        let logs = self.http_provider.get_logs(&filter).await.map_err(|e| {
            error!("Failed to fetch logs: {}", e);
            EthereumError::External(format!("Log retrieval failed: {e}"))
        })?;

        let mut events = Vec::new();
        for log in logs {
            if let Some(event) = Self::convert_log_to_event(&log) {
                events.push(event);
            }
        }

        Ok(events)
    }

    /// Convert RPC log to `LoanEvent`
    fn convert_log_to_event(log: &alloy_rpc_types_eth::Log) -> Option<LoanEvent> {
        let primitive_log = Self::convert_rpc_log_to_primitive(log);
        let block_number = log.block_number.unwrap_or(0);
        let tx_hash = log.transaction_hash.unwrap_or_default();
        let log_index = log.log_index.unwrap_or(0);

        // Decode different event types
        if let Ok(event) = ILoan::StateUpdated::decode_log(&primitive_log) {
            return Some(LoanEvent::StateUpdated {
                state_id: event.stateId,
                new_state: event.newState,
                proof_id: event.proofId,
                updater: event.updater,
                timestamp: event.timestamp.try_into().unwrap_or(0),
                block_number,
                tx_hash,
                log_index,
            });
        }

        if let Ok(event) = ILoan::BatchStateUpdated::decode_log(&primitive_log) {
            return Some(LoanEvent::BatchStateUpdated {
                state_ids: event.stateIds.clone(),
                new_states: event.newStates.clone(),
                updater: event.updater,
                timestamp: event.timestamp.try_into().unwrap_or(0),
                block_number,
                tx_hash,
                log_index,
            });
        }

        if let Ok(event) = ILoan::ProofStored::decode_log(&primitive_log) {
            return Some(LoanEvent::ProofStored {
                proof_id: event.proofId,
                state_id: event.stateId,
                submitter: event.submitter,
                timestamp: event.timestamp.try_into().unwrap_or(0),
                block_number,
                tx_hash,
                log_index,
            });
        }

        if let Ok(event) = ILoan::ProofVerified::decode_log(&primitive_log) {
            return Some(LoanEvent::ProofVerified {
                proof_id: event.proofId,
                success: event.success,
                result: event.result.clone(),
                timestamp: event.timestamp.try_into().unwrap_or(0),
                block_number,
                tx_hash,
                log_index,
            });
        }

        if let Ok(event) = ILoan::StateReadRequested::decode_log(&primitive_log) {
            return Some(LoanEvent::StateReadRequested {
                state_id: event.stateId,
                reader: event.reader,
                timestamp: event.timestamp.try_into().unwrap_or(0),
                block_number,
                tx_hash,
                log_index,
            });
        }

        if let Ok(event) = ILoan::ProofReadRequested::decode_log(&primitive_log) {
            return Some(LoanEvent::ProofReadRequested {
                proof_id: event.proofId,
                reader: event.reader,
                timestamp: event.timestamp.try_into().unwrap_or(0),
                block_number,
                tx_hash,
                log_index,
            });
        }

        debug!("Unknown event type in log: {:?}", log);
        None
    }

    /// Process event and dispatch to subscribers
    #[allow(clippy::cognitive_complexity)]
    async fn process_and_dispatch_event(&self, event: LoanEvent) {
        // Broadcast to global stream
        if let Err(e) = self.event_broadcaster.send(event.clone()) {
            warn!("Failed to broadcast event: {}", e);
        }

        // Check subscribers and call matching callbacks
        for (sub_id, (filter, callback)) in self.event_callbacks.read().await.iter() {
            if Self::event_matches_filter(&event, filter) {
                debug!("Dispatching event to subscription: {:?}", sub_id);
                callback(event.clone());
            }
        }

        // Persist event if enabled
        #[cfg(feature = "database")]
        if self.listener_config.enable_persistence {
            if let Some(cache) = &self.cache {
                if let Err(e) = Self::persist_event(&event, cache).await {
                    warn!("Failed to persist event to database: {}", e);
                }
            }
        }
    }

    /// Check if event matches filter criteria
    fn event_matches_filter(event: &LoanEvent, filter: &EventFilter) -> bool {
        // Check event type filter
        if let Some(event_types) = &filter.event_types {
            let event_type = match event {
                LoanEvent::StateUpdated { .. } => "StateUpdated",
                LoanEvent::BatchStateUpdated { .. } => "BatchStateUpdated",
                LoanEvent::ProofStored { .. } => "ProofStored",
                LoanEvent::ProofVerified { .. } => "ProofVerified",
                LoanEvent::StateReadRequested { .. } => "StateReadRequested",
                LoanEvent::ProofReadRequested { .. } => "ProofReadRequested",
                LoanEvent::Deposit { .. } => "Deposit",
                LoanEvent::LoanPayment { .. } => "LoanPayment",
                LoanEvent::LoanClosed { .. } => "LoanClosed",
                LoanEvent::Withdrawal { .. } => "Withdrawal",
            };
            if !event_types.contains(&event_type.to_string()) {
                return false;
            }
        }

        // Check state ID filter
        if let Some(state_ids) = &filter.state_ids {
            let event_state_id = match event {
                LoanEvent::StateUpdated { state_id, .. }
                | LoanEvent::ProofStored { state_id, .. }
                | LoanEvent::StateReadRequested { state_id, .. } => Some(*state_id),
                LoanEvent::Deposit { loan_id, .. }
                | LoanEvent::LoanPayment { loan_id, .. }
                | LoanEvent::LoanClosed { loan_id, .. } => Some(*loan_id), // Use loan_id as state_id for filtering
                _ => None,
            };
            if let Some(state_id) = event_state_id {
                if !state_ids.contains(&state_id) {
                    return false;
                }
            }
        }

        // Check address filter
        if let Some(addresses) = &filter.addresses {
            let event_address = match event {
                LoanEvent::StateUpdated { updater, .. }
                | LoanEvent::BatchStateUpdated { updater, .. } => Some(*updater),
                LoanEvent::ProofStored { submitter, .. } => Some(*submitter),
                LoanEvent::StateReadRequested { reader, .. }
                | LoanEvent::ProofReadRequested { reader, .. } => Some(*reader),
                LoanEvent::Deposit { user, .. } | LoanEvent::Withdrawal { user, .. } => Some(*user),
                LoanEvent::LoanPayment { payer, .. } => Some(*payer),
                LoanEvent::ProofVerified { .. } | LoanEvent::LoanClosed { .. } => None,
            };
            if let Some(address) = event_address {
                if !addresses.contains(&address) {
                    return false;
                }
            }
        }

        // Check block range filter
        let event_block = match event {
            LoanEvent::StateUpdated { block_number, .. }
            | LoanEvent::BatchStateUpdated { block_number, .. }
            | LoanEvent::ProofStored { block_number, .. }
            | LoanEvent::ProofVerified { block_number, .. }
            | LoanEvent::StateReadRequested { block_number, .. }
            | LoanEvent::ProofReadRequested { block_number, .. }
            | LoanEvent::Deposit { block_number, .. }
            | LoanEvent::LoanPayment { block_number, .. }
            | LoanEvent::LoanClosed { block_number, .. }
            | LoanEvent::Withdrawal { block_number, .. } => *block_number,
        };

        if let Some(from_block) = filter.from_block {
            if event_block < from_block {
                return false;
            }
        }

        if let Some(to_block) = filter.to_block {
            if event_block > to_block {
                return false;
            }
        }

        true
    }

    /// Persist event to database cache
    #[cfg(feature = "database")]
    async fn persist_event(event: &LoanEvent, cache: &EthereumCache) -> Result<()> {
        let contract_event = Self::convert_loan_event_to_contract_event(event);

        match cache.store_event(&contract_event).await {
            Ok(id) => {
                debug!("Successfully persisted event to database with ID: {}", id);
                Ok(())
            }
            Err(e) => {
                error!("Failed to persist event to database: {}", e);
                Err(e)
            }
        }
    }

    /// Convert LoanEvent to ContractEvent for database storage
    #[cfg(feature = "database")]
    fn convert_loan_event_to_contract_event(event: &LoanEvent) -> crate::types::ContractEvent {
        use crate::types::ContractEvent;

        match event {
            LoanEvent::StateUpdated {
                state_id,
                new_state: _,
                proof_id,
                updater: _,
                timestamp,
                block_number,
                tx_hash,
                log_index,
            } => ContractEvent {
                event_type: "StateUpdated".to_string(),
                state_id: Some(*state_id),
                proof_id: Some(*proof_id),
                block_number: *block_number,
                transaction_hash: *tx_hash,
                log_index: *log_index,
                timestamp: *timestamp,
            },
            LoanEvent::BatchStateUpdated {
                state_ids: _,
                new_states: _,
                updater: _,
                timestamp,
                block_number,
                tx_hash,
                log_index,
            } => ContractEvent {
                event_type: "BatchStateUpdated".to_string(),
                state_id: None, // Batch events don't have single state_id
                proof_id: None,
                block_number: *block_number,
                transaction_hash: *tx_hash,
                log_index: *log_index,
                timestamp: *timestamp,
            },
            LoanEvent::ProofStored {
                proof_id,
                state_id,
                submitter: _,
                timestamp,
                block_number,
                tx_hash,
                log_index,
            } => ContractEvent {
                event_type: "ProofStored".to_string(),
                state_id: Some(*state_id),
                proof_id: Some(*proof_id),
                block_number: *block_number,
                transaction_hash: *tx_hash,
                log_index: *log_index,
                timestamp: *timestamp,
            },
            LoanEvent::ProofVerified {
                proof_id,
                success: _,
                result: _,
                timestamp,
                block_number,
                tx_hash,
                log_index,
            } => ContractEvent {
                event_type: "ProofVerified".to_string(),
                state_id: None,
                proof_id: Some(*proof_id),
                block_number: *block_number,
                transaction_hash: *tx_hash,
                log_index: *log_index,
                timestamp: *timestamp,
            },
            LoanEvent::StateReadRequested {
                state_id,
                reader: _,
                timestamp,
                block_number,
                tx_hash,
                log_index,
            } => ContractEvent {
                event_type: "StateReadRequested".to_string(),
                state_id: Some(*state_id),
                proof_id: None,
                block_number: *block_number,
                transaction_hash: *tx_hash,
                log_index: *log_index,
                timestamp: *timestamp,
            },
            LoanEvent::ProofReadRequested {
                proof_id,
                reader: _,
                timestamp,
                block_number,
                tx_hash,
                log_index,
            } => ContractEvent {
                event_type: "ProofReadRequested".to_string(),
                state_id: None,
                proof_id: Some(*proof_id),
                block_number: *block_number,
                transaction_hash: *tx_hash,
                log_index: *log_index,
                timestamp: *timestamp,
            },
            LoanEvent::Deposit {
                user: _,
                amount: _,
                loan_id,
                timestamp,
                block_number,
                tx_hash,
                log_index,
            } => ContractEvent {
                event_type: "Deposit".to_string(),
                state_id: Some(*loan_id), // Use loan_id as state_id for loan events
                proof_id: None,
                block_number: *block_number,
                transaction_hash: *tx_hash,
                log_index: *log_index,
                timestamp: *timestamp,
            },
            LoanEvent::LoanPayment {
                loan_id,
                amount: _,
                payer: _,
                timestamp,
                block_number,
                tx_hash,
                log_index,
            } => ContractEvent {
                event_type: "LoanPayment".to_string(),
                state_id: Some(*loan_id), // Use loan_id as state_id for loan events
                proof_id: None,
                block_number: *block_number,
                transaction_hash: *tx_hash,
                log_index: *log_index,
                timestamp: *timestamp,
            },
            LoanEvent::LoanClosed {
                loan_id,
                final_amount: _,
                timestamp,
                block_number,
                tx_hash,
                log_index,
            } => ContractEvent {
                event_type: "LoanClosed".to_string(),
                state_id: Some(*loan_id), // Use loan_id as state_id for loan events
                proof_id: None,
                block_number: *block_number,
                transaction_hash: *tx_hash,
                log_index: *log_index,
                timestamp: *timestamp,
            },
            LoanEvent::Withdrawal {
                user: _,
                amount: _,
                timestamp,
                block_number,
                tx_hash,
                log_index,
            } => ContractEvent {
                event_type: "Withdrawal".to_string(),
                state_id: None, // Withdrawals don't have specific state_id or loan_id
                proof_id: None,
                block_number: *block_number,
                transaction_hash: *tx_hash,
                log_index: *log_index,
                timestamp: *timestamp,
            },
        }
    }

    pub async fn monitor_events(&self) -> Result<()> {
        warn!("Using deprecated monitor_events. Use start_event_monitoring instead.");
        self.start_event_monitoring().await
    }

    pub async fn check_for_new_events(
        &self,
        last_block: u64,
        _contract: &ILoanInstance<&EthProvider>,
    ) -> Result<u64> {
        warn!("Using deprecated check_for_new_events. Use process_new_events instead.");
        let current_block = self.http_provider.get_block_number().await?;

        if current_block <= last_block {
            return Ok(last_block);
        }

        debug!(
            "Checking for events from block {} to {}",
            last_block + 1,
            current_block
        );

        // Create filter for new blocks
        let filter = Filter::new()
            .address(self.contracts.loan)
            .from_block(last_block + 1)
            .to_block(current_block);

        let logs = self.http_provider.get_logs(&filter).await.map_err(|e| {
            error!("Failed to get logs: {}", e);
            EthereumError::External(format!("Log retrieval failed: {e}"))
        })?;

        let log_count = logs.len();
        for log in &logs {
            self.process_event_log(log);
        }

        info!(
            "Processed {} events from blocks {} to {}",
            log_count,
            last_block + 1,
            current_block
        );
        Ok(current_block)
    }

    pub fn process_event_log(&self, log: &alloy_rpc_types_eth::Log) {
        let primitive_log = Self::convert_rpc_log_to_primitive(log);
        self.decode_and_log_event(&primitive_log, log);
    }

    fn convert_rpc_log_to_primitive(log: &alloy_rpc_types_eth::Log) -> alloy_primitives::Log {
        alloy_primitives::Log {
            address: log.address(),
            data: alloy_primitives::LogData::new(log.topics().to_vec(), log.data().data.clone())
                .unwrap_or_else(|| {
                    alloy_primitives::LogData::new_unchecked(
                        log.topics().to_vec(),
                        log.data().data.clone(),
                    )
                }),
        }
    }

    pub fn decode_and_log_event(
        &self,
        primitive_log: &alloy_primitives::Log,
        original_log: &alloy_rpc_types_eth::Log,
    ) {
        if let Ok(event) = ILoan::StateUpdated::decode_log(primitive_log) {
            self.handle_state_updated_event(&event);
        } else if let Ok(event) = ILoan::ProofStored::decode_log(primitive_log) {
            Self::handle_proof_stored_event(&event);
        } else if let Ok(event) = ILoan::ProofVerified::decode_log(primitive_log) {
            Self::handle_proof_verified_event(&event);
        } else {
            debug!("Unknown event type in log: {:?}", original_log);
        }
    }

    pub fn handle_state_updated_event(&self, event: &ILoan::StateUpdated) {
        info!(
            "StateUpdated event: stateId={}, newState={}, proofId={}, updater={}",
            hex::encode(event.stateId.as_slice()),
            hex::encode(event.newState.as_slice()),
            hex::encode(event.proofId.as_slice()),
            event.updater
        );

        #[cfg(feature = "database")]
        if let Some(cache) = &self.cache {
            // Note: This function is not async, so we use the synchronous store_event approach
            // The main event monitoring pipeline handles async persistence via persist_event()
            debug!(
                "StateUpdated event handled: state_id={}",
                hex::encode(event.stateId.as_slice())
            );
            // Events from the main monitoring loop will be persisted automatically
            let _ = cache; // Acknowledge cache is available for future use
        }
    }

    pub fn handle_proof_stored_event(event: &ILoan::ProofStored) {
        info!(
            "ProofStored event: proofId={}, stateId={}, submitter={}",
            hex::encode(event.proofId.as_slice()),
            hex::encode(event.stateId.as_slice()),
            event.submitter
        );
    }

    pub fn handle_proof_verified_event(event: &ILoan::ProofVerified) {
        info!(
            "ProofVerified event: proofId={}, success={}, result={}",
            hex::encode(event.proofId.as_slice()),
            event.success,
            hex::encode(event.result.as_ref())
        );
    }

    pub async fn publish_state_root(
        &self,
        state_id: FixedBytes<32>,
        state_root: FixedBytes<32>,
        proof: Bytes,
        public_values: Bytes,
    ) -> Result<StateUpdate> {
        self.update_state(state_id, state_root, proof, public_values)
            .await
    }

    pub async fn verify_zk_proof(
        &self,
        proof: Bytes,
        public_values: Bytes,
    ) -> Result<ProofVerificationResult> {
        let current_block = self.http_provider.get_block_number().await?;

        // Call the on-chain verifier
        match self
            .verify_proof(public_values.clone(), proof.clone())
            .await
        {
            Ok(_result) => {
                // Proof verification succeeded
                Ok(ProofVerificationResult {
                    proof_id: FixedBytes::from_slice(&keccak256(&proof).0),
                    verified: true,
                    result: Some(public_values),
                    block_number: current_block,
                    gas_used: U256::ZERO, // TODO: Get actual gas usage from transaction receipt
                    error_message: None,
                })
            }
            Err(e) => {
                // Proof verification failed
                Ok(ProofVerificationResult {
                    proof_id: FixedBytes::from_slice(&keccak256(&proof).0),
                    verified: false,
                    result: None,
                    block_number: current_block,
                    gas_used: U256::ZERO,
                    error_message: Some(format!("Proof verification failed: {e}")),
                })
            }
        }
    }

    pub async fn get_verifier_key(&self) -> Result<Bytes> {
        let contract = ILoan::new(self.contracts.loan, &self.http_provider);

        let vkey = contract.loanProgramVKey().call().await.map_err(|e| {
            warn!("Failed to get verifier key: {}", e);
            EthereumError::from_contract_error(&format!("Verifier key query failed: {e}"))
        })?;

        // Convert FixedBytes<32> to Bytes
        Ok(Bytes::from(vkey.as_slice().to_vec()))
    }

    pub async fn get_proof_result(&self, proof_id: FixedBytes<32>) -> Result<Option<Bytes>> {
        let contract = ILoan::new(self.contracts.loan, &self.http_provider);

        let result = contract
            .getStoredResult(proof_id)
            .call()
            .await
            .map_err(|e| {
                warn!("Failed to get proof result: {}", e);
                EthereumError::from_contract_error(&format!("Proof result query failed: {e}"))
            })?;

        if result.is_empty() {
            Ok(None)
        } else {
            Ok(Some(result))
        }
    }

    pub const fn get_verification_data(&self, _proof_id: FixedBytes<32>) -> Result<Option<Bytes>> {
        // TODO: Implement verification data retrieval from contract
        Ok(None)
    }

    pub fn verify_proof_independently(
        &self,
        proof_id: FixedBytes<32>,
    ) -> Result<ProofVerificationResult> {
        Err(EthereumError::General(eyre::eyre!(
            "verify_proof_independently: not implemented (proof_id: 0x{})",
            hex::encode(proof_id)
        )))
    }

    pub fn get_historical_states(
        &self,
        state_id: FixedBytes<32>,
        limit: Option<u64>,
    ) -> Result<StateHistory> {
        // TODO: Implement historical states retrieval
        Ok(StateHistory {
            state_id,
            state_roots: vec![FixedBytes::ZERO],
            block_numbers: vec![0],
            timestamps: vec![0],
            proof_ids: vec![Some(FixedBytes::ZERO)],
            limit: limit.unwrap_or(100),
        })
    }

    pub async fn get_network_stats(&self) -> Result<NetworkStats> {
        // TODO: Implement network stats retrieval
        let current_block = self.http_provider.get_block_number().await?;

        Ok(NetworkStats {
            chain_id: 1,
            network_name: "mainnet".to_string(),
            block_number: current_block,
            gas_price: U256::ZERO,
            base_fee: Some(U256::ZERO),
            sync_status: true,
        })
    }

    pub fn check_inclusion_proof(
        &self,
        _leaf_hash: FixedBytes<32>,
        _leaf_index: u64,
        _siblings: Vec<FixedBytes<32>>,
        _root: FixedBytes<32>,
    ) -> Result<InclusionProof> {
        // TODO: Implement inclusion proof checking
        Ok(InclusionProof {
            leaf_hash: FixedBytes::ZERO,
            leaf_index: 0,
            siblings: vec![],
            root: FixedBytes::ZERO,
            verified: false,
        })
    }

    pub async fn get_proof_data(&self, proof_id: FixedBytes<32>) -> Result<Option<Bytes>> {
        let contract = ILoan::new(self.contracts.loan, &self.http_provider);

        let proof_data = contract
            .getStoredProof(proof_id)
            .call()
            .await
            .map_err(|e| {
                warn!("Failed to get proof data: {}", e);
                EthereumError::from_contract_error(&format!("Proof data query failed: {e}"))
            })?;

        if proof_data.is_empty() {
            Ok(None)
        } else {
            Ok(Some(proof_data))
        }
    }

    pub async fn get_state_root(&self, state_id: FixedBytes<32>) -> Result<FixedBytes<32>> {
        let contract = ILoan::new(self.contracts.loan, &self.http_provider);

        let state_root = contract
            .getCurrentState(state_id)
            .call()
            .await
            .map_err(|e| {
                warn!("Failed to get state root: {}", e);
                EthereumError::from_contract_error(&format!("State root query failed: {e}"))
            })?;

        Ok(state_root)
    }

    pub fn get_state_proof_history(
        &self,
        _state_id: FixedBytes<32>,
    ) -> Result<Vec<FixedBytes<32>>> {
        // TODO: Implement state proof history retrieval from contract
        Ok(vec![FixedBytes::ZERO])
    }

    pub async fn get_verifier_version(&self) -> Result<String> {
        let verifier_contract = ISP1Verifier::new(self.contracts.verifier, &self.http_provider);

        let version = verifier_contract.VERSION().call().await.map_err(|e| {
            warn!("Failed to get verifier version: {}", e);
            EthereumError::from_contract_error(&format!("Verifier version query failed: {e}"))
        })?;

        Ok(version)
    }

    /// Create new ethereum client without validation (for CLI use)
    pub async fn new_without_validation(config: Config) -> Result<Self> {
        let signer_config = config
            .signer
            .as_ref()
            .ok_or_else(|| EthereumError::Config("Signer required".to_string()))?;

        let signer = PrivateKeySigner::from_bytes(&FixedBytes::<32>::try_from(
            hex::decode(&signer_config.private_key)?.as_slice(),
        )?)
        .map_err(|e| EthereumError::Signer(e.to_string()))?;

        let wallet = EthereumWallet::from(signer.clone());

        // Create provider with wallet for signing
        let http_provider = ProviderBuilder::new()
            .wallet(wallet)
            .connect_http(config.network.rpc_url.clone());

        let contracts = ContractAddresses::new(
            config.contract.arda_credit_contract,
            config.contract.verifier_contract,
        );

        #[cfg(feature = "database")]
        let cache = if let Ok(database_url) = std::env::var("DATABASE_URL") {
            let pool = sqlx::PgPool::connect(&database_url).await?;
            Some(EthereumCache::new(pool))
        } else {
            None
        };

        // Initialize event system
        let (event_broadcaster, event_receiver) = broadcast::channel(1000);
        let event_callbacks = Arc::new(RwLock::new(HashMap::new()));
        let listener_config = EventListenerConfig::default();
        let last_processed_block = Arc::new(RwLock::new(0));

        Ok(Self {
            config,
            http_provider,
            contracts,
            signer,

            #[cfg(feature = "database")]
            cache,

            event_callbacks,
            event_broadcaster,
            event_receiver,
            listener_config,
            last_processed_block,
        })
    }

    /// Query the contract's current verification key and verifier address
    pub async fn query_contract_verification_key(&self) -> Result<(FixedBytes<32>, Address)> {
        let contract = ILoan::new(self.contracts.loan, &self.http_provider);

        let verification_key = contract
            .getProgramVerificationKey()
            .call()
            .await
            .map_err(|e| {
                EthereumError::from_contract_error(&format!("Failed to get contract vkey: {e}"))
            })?;

        let verifier_address = contract.verifier().call().await.map_err(|e| {
            EthereumError::from_contract_error(&format!("Failed to get verifier address: {e}"))
        })?;

        Ok((verification_key, verifier_address))
    }
}

pub type Receipt = TransactionReceipt;

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::{Address, B256, U256};

    // Type aliases for function signature testing
    type StateUpdatedHandler = fn(&EthereumClient, &ILoan::StateUpdated);
    type EventProcessor = fn(&EthereumClient, &alloy_rpc_types_eth::Log);
    type EventDecoder = fn(&EthereumClient, &alloy_primitives::Log, &alloy_rpc_types_eth::Log);

    #[test]
    fn test_handle_state_updated_event_is_public() {
        // Test that handle_state_updated_event method is public and accessible
        let event = ILoan::StateUpdated {
            stateId: B256::from([1u8; 32]),
            newState: B256::from([2u8; 32]),
            proofId: B256::from([3u8; 32]),
            updater: Address::from([4u8; 20]),
            timestamp: U256::from(1_234_567_890_u64),
        };

        // Since the method is part of the EthereumClient impl, we can't call it statically
        // But we can verify it exists by checking the signature compiles
        // The method just logs info, so this test verifies the event struct is correctly defined
        assert_eq!(event.stateId, B256::from([1u8; 32]));
        assert_eq!(event.newState, B256::from([2u8; 32]));
        assert_eq!(event.proofId, B256::from([3u8; 32]));
        assert_eq!(event.updater, Address::from([4u8; 20]));
    }

    #[test]
    fn test_handle_proof_stored_event_is_public() {
        // Test that handle_proof_stored_event method is public and accessible
        let event = ILoan::ProofStored {
            proofId: B256::from([1u8; 32]),
            stateId: B256::from([2u8; 32]),
            submitter: Address::from([3u8; 20]),
            timestamp: U256::from(1_234_567_890_u64),
        };

        // This test mainly checks that handle_proof_stored_event doesn't panic
        // and verifies the event struct is correctly defined
        EthereumClient::handle_proof_stored_event(&event);

        // Verify event fields
        assert_eq!(event.proofId, B256::from([1u8; 32]));
        assert_eq!(event.stateId, B256::from([2u8; 32]));
        assert_eq!(event.submitter, Address::from([3u8; 20]));
    }

    #[test]
    fn test_handle_proof_verified_event_is_public() {
        // Test that handle_proof_verified_event method is public and accessible
        let event = ILoan::ProofVerified {
            proofId: B256::from([1u8; 32]),
            success: true,
            result: alloy_primitives::Bytes::from(vec![4u8, 5u8, 6u8]),
            timestamp: U256::from(1_234_567_890_u64),
        };

        // This test mainly checks that handle_proof_verified_event doesn't panic
        // and verifies the event struct is correctly defined
        EthereumClient::handle_proof_verified_event(&event);

        // Verify event fields
        assert_eq!(event.proofId, B256::from([1u8; 32]));
        assert!(event.success);
        assert_eq!(
            event.result,
            alloy_primitives::Bytes::from(vec![4u8, 5u8, 6u8])
        );
    }

    #[test]
    fn test_event_listener_methods_are_public() {
        // This test verifies that the event listener methods exist and are public.
        // We test this by verifying the method signatures compile and are accessible.

        // Test proof stored event handling
        let proof_stored = ILoan::ProofStored {
            proofId: B256::from([1u8; 32]),
            stateId: B256::from([2u8; 32]),
            submitter: Address::from([3u8; 20]),
            timestamp: U256::from(1_234_567_890_u64),
        };

        // Verify static method is public and callable
        EthereumClient::handle_proof_stored_event(&proof_stored);

        // Test proof verified event handling
        let proof_verified = ILoan::ProofVerified {
            proofId: B256::from([4u8; 32]),
            success: true,
            result: alloy_primitives::Bytes::from(vec![1u8, 2u8, 3u8]),
            timestamp: U256::from(1_234_567_890_u64),
        };

        // Verify static method is public and callable
        EthereumClient::handle_proof_verified_event(&proof_verified);

        // The main assertion is that these methods are accessible
        // The fact that this test compiles proves the methods are public
    }

    #[test]
    fn test_event_method_signatures() {
        // This test verifies that the method signatures are correct and public
        // by creating function pointers to the methods

        // Test that we can create function pointers to the public methods
        let handle_proof_stored: fn(&ILoan::ProofStored) =
            EthereumClient::handle_proof_stored_event;
        let handle_proof_verified: fn(&ILoan::ProofVerified) =
            EthereumClient::handle_proof_verified_event;

        // Test that instance methods exist (can't easily test without a client instance)
        // but we can verify their signatures by attempting to reference them
        let handle_state_updated: StateUpdatedHandler = EthereumClient::handle_state_updated_event;
        let process_event: EventProcessor = EthereumClient::process_event_log;
        let decode_event: EventDecoder = EthereumClient::decode_and_log_event;

        // Use the function pointers to avoid unused variable warnings
        std::hint::black_box((
            handle_proof_stored,
            handle_proof_verified,
            handle_state_updated,
            process_event,
            decode_event,
        ));

        // The key test is that this compiles, proving all methods are public
    }

    #[test]
    fn test_custom_error_integration() {
        // Test that the EthereumError::from_contract_error method works correctly
        // This ensures the integration between client.rs and error.rs is working

        // Test UnauthorizedAccess error parsing
        let error_msg = "Transaction failed: 0x344fd586";
        let parsed_error = EthereumError::from_contract_error(error_msg);
        match parsed_error {
            EthereumError::UnauthorizedAccess => {}
            _ => panic!("Expected UnauthorizedAccess error, got: {parsed_error:?}"),
        }

        // Test ProofAlreadyExists error parsing
        let error_msg = "Transaction failed: 0xb8cdb9bd";
        let parsed_error = EthereumError::from_contract_error(error_msg);
        match parsed_error {
            EthereumError::ProofAlreadyExists => {}
            _ => panic!("Expected ProofAlreadyExists error, got: {parsed_error:?}"),
        }

        // Test InvalidArrayLength error parsing
        let error_msg = "Batch transaction failed: 0x9d89020a";
        let parsed_error = EthereumError::from_contract_error(error_msg);
        match parsed_error {
            EthereumError::InvalidArrayLength => {}
            _ => panic!("Expected InvalidArrayLength error, got: {parsed_error:?}"),
        }

        // Test fallback to generic Contract error for unknown signatures
        let error_msg = "Transaction failed: 0x12345678";
        let parsed_error = EthereumError::from_contract_error(error_msg);
        match parsed_error {
            EthereumError::Contract(msg) => {
                assert_eq!(msg, "Transaction failed: 0x12345678");
            }
            _ => panic!("Expected Contract error for unknown signature, got: {parsed_error:?}"),
        }
    }

    #[test]
    fn test_error_message_formatting() {
        // Test that error messages are formatted correctly for different contract methods

        // Test verify proof error formatting
        let verify_error = EthereumError::from_contract_error("Transaction failed: 0x344fd586");
        assert_eq!(
            verify_error.to_string(),
            "Unauthorized access: The caller is not authorized to perform this operation"
        );

        // Test batch update error formatting
        let batch_error =
            EthereumError::from_contract_error("Batch transaction failed: 0x9d89020a");
        assert_eq!(
            batch_error.to_string(),
            "Invalid array length: Input arrays have mismatched lengths"
        );

        // Test state query error formatting
        let state_error = EthereumError::from_contract_error("State query failed: 0xfa8d84c7");
        assert_eq!(
            state_error.to_string(),
            "State not found: The requested state does not exist"
        );

        // Test proof query error formatting
        let proof_error = EthereumError::from_contract_error("Proof data query failed: 0x36131e57");
        assert_eq!(
            proof_error.to_string(),
            "Proof not found: The requested proof does not exist"
        );
    }

    #[test]
    fn test_contract_method_error_patterns() {
        // Test the specific error message patterns that would come from different contract methods

        // Pattern from update_state method
        let update_state_pattern = "Transaction failed: execution reverted 0x344fd586";
        let error = EthereumError::from_contract_error(update_state_pattern);
        match error {
            EthereumError::UnauthorizedAccess => {}
            _ => panic!("Expected UnauthorizedAccess for update_state pattern"),
        }

        // Pattern from batch_update_states method
        let batch_update_pattern = "Batch transaction failed: 0x9d89020a InvalidArrayLength()";
        let error = EthereumError::from_contract_error(batch_update_pattern);
        match error {
            EthereumError::InvalidArrayLength => {}
            _ => panic!("Expected InvalidArrayLength for batch_update pattern"),
        }

        // Pattern from get_current_state method
        let get_state_pattern = "State query failed: revert 0xfa8d84c7";
        let error = EthereumError::from_contract_error(get_state_pattern);
        match error {
            EthereumError::ContractStateNotFound => {}
            _ => panic!("Expected ContractStateNotFound for get_state pattern"),
        }

        // Pattern from get_proof_data method
        let get_proof_pattern = "Proof data query failed: 0x36131e57";
        let error = EthereumError::from_contract_error(get_proof_pattern);
        match error {
            EthereumError::ContractProofNotFound => {}
            _ => panic!("Expected ContractProofNotFound for get_proof pattern"),
        }

        // Pattern from verifier version query
        let verifier_pattern = "Verifier version query failed: 0xe55fb509";
        let error = EthereumError::from_contract_error(verifier_pattern);
        match error {
            EthereumError::InvalidLimit => {}
            _ => panic!("Expected InvalidLimit for verifier pattern"),
        }
    }

    #[test]
    fn test_all_error_signatures_unique() {
        // Ensure all error signatures are unique and properly mapped
        let signatures = vec![
            ("0x344fd586", "UnauthorizedAccess"),
            ("0x9d89020a", "InvalidArrayLength"),
            ("0xfa8d84c7", "ContractStateNotFound"),
            ("0x36131e57", "ContractProofNotFound"),
            ("0xe55fb509", "InvalidLimit"),
            ("0x63df8171", "InvalidIndex"),
            ("0xb8cdb9bd", "ProofAlreadyExists"),
            ("0x7fcdd1f4", "ProofInvalid"),
        ];

        for (sig, expected_error) in signatures {
            let error_msg = format!("Test error: {sig}");
            let parsed_error = EthereumError::from_contract_error(&error_msg);

            let error_name = match parsed_error {
                EthereumError::UnauthorizedAccess => "UnauthorizedAccess",
                EthereumError::InvalidArrayLength => "InvalidArrayLength",
                EthereumError::ContractStateNotFound => "ContractStateNotFound",
                EthereumError::ContractProofNotFound => "ContractProofNotFound",
                EthereumError::InvalidLimit => "InvalidLimit",
                EthereumError::InvalidIndex => "InvalidIndex",
                EthereumError::ProofAlreadyExists => "ProofAlreadyExists",
                EthereumError::ProofInvalid => "ProofInvalid",
                _ => "Unknown",
            };

            assert_eq!(
                error_name, expected_error,
                "Signature {sig} should map to {expected_error} but got {error_name}"
            );
        }
    }

    #[test]
    fn test_real_world_error_scenarios() {
        // Test error patterns that might actually occur in real usage

        // Scenario: User tries to update state without authorization
        let unauthorized_msg =
            "Contract call reverted: execution reverted with custom error 0x344fd586";
        let error = EthereumError::from_contract_error(unauthorized_msg);
        assert!(matches!(error, EthereumError::UnauthorizedAccess));

        // Scenario: Batch operation with mismatched array lengths
        let batch_length_msg = "Batch update failed: arrays length mismatch 0x9d89020a";
        let error = EthereumError::from_contract_error(batch_length_msg);
        assert!(matches!(error, EthereumError::InvalidArrayLength));

        // Scenario: Trying to submit duplicate proof
        let duplicate_proof_msg = "Proof submission failed: 0xb8cdb9bd ProofAlreadyExists";
        let error = EthereumError::from_contract_error(duplicate_proof_msg);
        assert!(matches!(error, EthereumError::ProofAlreadyExists));

        // Scenario: Querying non-existent state
        let missing_state_msg = "State retrieval failed: 0xfa8d84c7 state not found";
        let error = EthereumError::from_contract_error(missing_state_msg);
        assert!(matches!(error, EthereumError::ContractStateNotFound));

        // Scenario: Array index out of bounds
        let index_error_msg = "Array access error: index out of bounds 0x63df8171";
        let error = EthereumError::from_contract_error(index_error_msg);
        assert!(matches!(error, EthereumError::InvalidIndex));

        // Scenario: Invalid proof submission (common SP1 verifier error)
        let proof_invalid_msg = "Transaction failed: server returned an error response: error code 3: execution reverted, data: \"0x7fcdd1f4\"";
        let error = EthereumError::from_contract_error(proof_invalid_msg);
        assert!(matches!(error, EthereumError::ProofInvalid));
    }
}
