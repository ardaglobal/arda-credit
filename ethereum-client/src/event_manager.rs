use crate::{
    client::{EthereumClient, EventFilter, LoanEvent, SubscriptionId},
    error::Result,
};
use alloy_primitives::{Address, FixedBytes};
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, error, info, warn};

/// Event handler trait for vApp server integration
pub trait EventHandler: Send + Sync {
    fn handle_state_updated(
        &self,
        state_id: FixedBytes<32>,
        new_state: FixedBytes<32>,
        proof_id: FixedBytes<32>,
        updater: Address,
        block_number: u64,
    );

    fn handle_batch_state_updated(
        &self,
        state_ids: Vec<FixedBytes<32>>,
        new_states: Vec<FixedBytes<32>>,
        updater: Address,
        block_number: u64,
    );

    fn handle_proof_stored(
        &self,
        proof_id: FixedBytes<32>,
        state_id: FixedBytes<32>,
        submitter: Address,
        block_number: u64,
    );

    fn handle_proof_verified(&self, proof_id: FixedBytes<32>, success: bool, block_number: u64);

    fn handle_state_read_requested(
        &self,
        state_id: FixedBytes<32>,
        reader: Address,
        block_number: u64,
    );

    fn handle_proof_read_requested(
        &self,
        proof_id: FixedBytes<32>,
        reader: Address,
        block_number: u64,
    );

    // USDC Deposit event handlers
    fn handle_deposit(
        &self,
        user: Address,
        amount: alloy_primitives::U256,
        transaction_hash: FixedBytes<32>,
        timestamp: u64,
        block_number: u64,
    );

    fn handle_loan_payment(
        &self,
        loan_id: FixedBytes<32>,
        amount: alloy_primitives::U256,
        payer: Address,
        block_number: u64,
    );

    fn handle_loan_closed(
        &self,
        loan_id: FixedBytes<32>,
        final_amount: alloy_primitives::U256,
        block_number: u64,
    );

    fn handle_withdrawal(&self, user: Address, amount: alloy_primitives::U256, block_number: u64);
}

/// Event manager for vApp server integration
pub struct EventManager {
    ethereum_client: Arc<EthereumClient>,
    event_handlers: Vec<Arc<dyn EventHandler>>,
    global_event_receiver: Option<broadcast::Receiver<LoanEvent>>,
}

impl EventManager {
    /// Create new event manager with Ethereum client
    #[must_use]
    pub fn new(ethereum_client: Arc<EthereumClient>) -> Self {
        Self {
            global_event_receiver: Some(ethereum_client.get_event_stream()),
            ethereum_client,
            event_handlers: Vec::new(),
        }
    }

    /// Register an event handler for all events
    pub fn register_handler(&mut self, handler: Arc<dyn EventHandler>) {
        self.event_handlers.push(handler);
        info!("Registered new event handler");
    }

    /// Subscribe to specific state events
    pub async fn subscribe_to_state_events(
        &self,
        state_ids: Vec<FixedBytes<32>>,
        handler: Arc<dyn EventHandler>,
    ) -> Result<SubscriptionId> {
        let filter = EventFilter {
            event_types: Some(vec![
                "StateUpdated".to_string(),
                "StateReadRequested".to_string(),
            ]),
            state_ids: Some(state_ids.clone()),
            addresses: None,
            from_block: None,
            to_block: None,
        };

        let callback = Arc::new(move |event: LoanEvent| match event {
            LoanEvent::StateUpdated {
                state_id,
                new_state,
                proof_id,
                updater,
                block_number,
                ..
            } => {
                handler.handle_state_updated(state_id, new_state, proof_id, updater, block_number);
            }
            LoanEvent::StateReadRequested {
                state_id,
                reader,
                block_number,
                ..
            } => {
                handler.handle_state_read_requested(state_id, reader, block_number);
            }
            _ => {
                debug!("Ignoring non-state event in state subscription");
            }
        });

        self.ethereum_client
            .subscribe_to_events(filter, callback)
            .await
    }

    /// Subscribe to all proof events
    pub async fn subscribe_to_proof_events(
        &self,
        handler: Arc<dyn EventHandler>,
    ) -> Result<SubscriptionId> {
        let filter = EventFilter {
            event_types: Some(vec![
                "ProofStored".to_string(),
                "ProofVerified".to_string(),
                "ProofReadRequested".to_string(),
            ]),
            state_ids: None,
            addresses: None,
            from_block: None,
            to_block: None,
        };

        let callback = Arc::new(move |event: LoanEvent| match event {
            LoanEvent::ProofStored {
                proof_id,
                state_id,
                submitter,
                block_number,
                ..
            } => {
                handler.handle_proof_stored(proof_id, state_id, submitter, block_number);
            }
            LoanEvent::ProofVerified {
                proof_id,
                success,
                block_number,
                ..
            } => {
                handler.handle_proof_verified(proof_id, success, block_number);
            }
            LoanEvent::ProofReadRequested {
                proof_id,
                reader,
                block_number,
                ..
            } => {
                handler.handle_proof_read_requested(proof_id, reader, block_number);
            }
            _ => {
                debug!("Ignoring non-proof event in proof subscription");
            }
        });

        self.ethereum_client
            .subscribe_to_events(filter, callback)
            .await
    }

    /// Subscribe to batch operation events
    pub async fn subscribe_to_batch_events(
        &self,
        handler: Arc<dyn EventHandler>,
    ) -> Result<SubscriptionId> {
        let filter = EventFilter {
            event_types: Some(vec!["BatchStateUpdated".to_string()]),
            state_ids: None,
            addresses: None,
            from_block: None,
            to_block: None,
        };

        let callback = Arc::new(move |event: LoanEvent| match event {
            LoanEvent::BatchStateUpdated {
                state_ids,
                new_states,
                updater,
                block_number,
                ..
            } => {
                handler.handle_batch_state_updated(state_ids, new_states, updater, block_number);
            }
            _ => {
                debug!("Ignoring non-batch event in batch subscription");
            }
        });

        self.ethereum_client
            .subscribe_to_events(filter, callback)
            .await
    }

    /// Subscribe to events from specific addresses
    pub async fn subscribe_to_address_events(
        &self,
        addresses: Vec<Address>,
        handler: Arc<dyn EventHandler>,
    ) -> Result<SubscriptionId> {
        let filter = EventFilter {
            event_types: None,
            state_ids: None,
            addresses: Some(addresses),
            from_block: None,
            to_block: None,
        };

        let callback = Arc::new(move |event: LoanEvent| {
            Self::dispatch_event_to_handler(&*handler, event);
        });

        self.ethereum_client
            .subscribe_to_events(filter, callback)
            .await
    }

    /// Start processing global event stream
    #[allow(clippy::cognitive_complexity)]
    pub async fn start_processing_events(&mut self) -> Result<()> {
        if let Some(mut receiver) = self.global_event_receiver.take() {
            info!("Starting global event processing loop");

            loop {
                match receiver.recv().await {
                    Ok(event) => {
                        debug!("Processing global event: {:?}", event);

                        // Dispatch to all registered handlers
                        for handler in &self.event_handlers {
                            Self::dispatch_event_to_handler(&**handler, event.clone());
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(count)) => {
                        warn!("Event stream lagged by {} events", count);
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        error!("Event stream closed");
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    /// Dispatch event to a specific handler
    fn dispatch_event_to_handler(handler: &dyn EventHandler, event: LoanEvent) {
        match event {
            LoanEvent::StateUpdated {
                state_id,
                new_state,
                proof_id,
                updater,
                block_number,
                ..
            } => {
                handler.handle_state_updated(state_id, new_state, proof_id, updater, block_number);
            }
            LoanEvent::BatchStateUpdated {
                state_ids,
                new_states,
                updater,
                block_number,
                ..
            } => {
                handler.handle_batch_state_updated(state_ids, new_states, updater, block_number);
            }
            LoanEvent::ProofStored {
                proof_id,
                state_id,
                submitter,
                block_number,
                ..
            } => {
                handler.handle_proof_stored(proof_id, state_id, submitter, block_number);
            }
            LoanEvent::ProofVerified {
                proof_id,
                success,
                block_number,
                ..
            } => {
                handler.handle_proof_verified(proof_id, success, block_number);
            }
            LoanEvent::StateReadRequested {
                state_id,
                reader,
                block_number,
                ..
            } => {
                handler.handle_state_read_requested(state_id, reader, block_number);
            }
            LoanEvent::ProofReadRequested {
                proof_id,
                reader,
                block_number,
                ..
            } => {
                handler.handle_proof_read_requested(proof_id, reader, block_number);
            }
            LoanEvent::Deposit {
                user,
                amount,
                transaction_hash,
                timestamp,
                block_number,
                ..
            } => {
                handler.handle_deposit(user, amount, transaction_hash, timestamp, block_number);
            }
            LoanEvent::LoanPayment {
                loan_id,
                amount,
                payer,
                block_number,
                ..
            } => {
                handler.handle_loan_payment(loan_id, amount, payer, block_number);
            }
            LoanEvent::LoanClosed {
                loan_id,
                final_amount,
                block_number,
                ..
            } => {
                handler.handle_loan_closed(loan_id, final_amount, block_number);
            }
            LoanEvent::Withdrawal {
                user,
                amount,
                block_number,
                ..
            } => {
                handler.handle_withdrawal(user, amount, block_number);
            }
        }
    }

    /// Unsubscribe from events
    pub async fn unsubscribe(&self, subscription_id: &SubscriptionId) -> Result<bool> {
        self.ethereum_client.unsubscribe(subscription_id).await
    }

    /// Get reference to underlying Ethereum client
    #[must_use]
    pub const fn ethereum_client(&self) -> &Arc<EthereumClient> {
        &self.ethereum_client
    }
}

/// Builder pattern for creating event filters
pub struct EventFilterBuilder {
    filter: EventFilter,
}

impl EventFilterBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self {
            filter: EventFilter::default(),
        }
    }

    #[must_use]
    pub fn with_event_types(mut self, event_types: Vec<String>) -> Self {
        self.filter.event_types = Some(event_types);
        self
    }

    #[must_use]
    pub fn with_state_ids(mut self, state_ids: Vec<FixedBytes<32>>) -> Self {
        self.filter.state_ids = Some(state_ids);
        self
    }

    #[must_use]
    pub fn with_addresses(mut self, addresses: Vec<Address>) -> Self {
        self.filter.addresses = Some(addresses);
        self
    }

    #[must_use]
    pub const fn with_block_range(mut self, from_block: u64, to_block: u64) -> Self {
        self.filter.from_block = Some(from_block);
        self.filter.to_block = Some(to_block);
        self
    }

    #[must_use]
    pub fn build(self) -> EventFilter {
        self.filter
    }
}

impl Default for EventFilterBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Example implementation of `EventHandler` for vApp servers
pub struct VAppEventHandler {
    server_name: String,
}

impl VAppEventHandler {
    #[must_use]
    pub const fn new(server_name: String) -> Self {
        Self { server_name }
    }
}

impl EventHandler for VAppEventHandler {
    fn handle_state_updated(
        &self,
        state_id: FixedBytes<32>,
        new_state: FixedBytes<32>,
        proof_id: FixedBytes<32>,
        updater: Address,
        block_number: u64,
    ) {
        info!(
            "[{}] State updated: id={}, new_state={}, proof={}, updater={}, block={}",
            self.server_name,
            hex::encode(state_id.as_slice()),
            hex::encode(new_state.as_slice()),
            hex::encode(proof_id.as_slice()),
            updater,
            block_number
        );
    }

    fn handle_batch_state_updated(
        &self,
        state_ids: Vec<FixedBytes<32>>,
        _new_states: Vec<FixedBytes<32>>,
        updater: Address,
        block_number: u64,
    ) {
        info!(
            "[{}] Batch state updated: {} states, updater={}, block={}",
            self.server_name,
            state_ids.len(),
            updater,
            block_number
        );
    }

    fn handle_proof_stored(
        &self,
        proof_id: FixedBytes<32>,
        state_id: FixedBytes<32>,
        submitter: Address,
        block_number: u64,
    ) {
        info!(
            "[{}] Proof stored: proof={}, state={}, submitter={}, block={}",
            self.server_name,
            hex::encode(proof_id.as_slice()),
            hex::encode(state_id.as_slice()),
            submitter,
            block_number
        );
    }

    fn handle_proof_verified(&self, proof_id: FixedBytes<32>, success: bool, block_number: u64) {
        info!(
            "[{}] Proof verified: proof={}, success={}, block={}",
            self.server_name,
            hex::encode(proof_id.as_slice()),
            success,
            block_number
        );
    }

    fn handle_state_read_requested(
        &self,
        state_id: FixedBytes<32>,
        reader: Address,
        block_number: u64,
    ) {
        debug!(
            "[{}] State read requested: state={}, reader={}, block={}",
            self.server_name,
            hex::encode(state_id.as_slice()),
            reader,
            block_number
        );
    }

    fn handle_proof_read_requested(
        &self,
        proof_id: FixedBytes<32>,
        reader: Address,
        block_number: u64,
    ) {
        debug!(
            "[{}] Proof read requested: proof={}, reader={}, block={}",
            self.server_name,
            hex::encode(proof_id.as_slice()),
            reader,
            block_number
        );
    }

    fn handle_deposit(
        &self,
        user: Address,
        amount: alloy_primitives::U256,
        transaction_hash: FixedBytes<32>,
        timestamp: u64,
        block_number: u64,
    ) {
        info!(
            "[{}] USDC Deposit: user={}, amount={}, tx_hash={}, timestamp={}, block={}",
            self.server_name,
            user,
            amount,
            hex::encode(transaction_hash.as_slice()),
            timestamp,
            block_number
        );
    }

    fn handle_loan_payment(
        &self,
        loan_id: FixedBytes<32>,
        amount: alloy_primitives::U256,
        payer: Address,
        block_number: u64,
    ) {
        info!(
            "[{}] Loan payment: loan_id={}, amount={}, payer={}, block={}",
            self.server_name,
            hex::encode(loan_id.as_slice()),
            amount,
            payer,
            block_number
        );
    }

    fn handle_loan_closed(
        &self,
        loan_id: FixedBytes<32>,
        final_amount: alloy_primitives::U256,
        block_number: u64,
    ) {
        info!(
            "[{}] Loan closed: loan_id={}, final_amount={}, block={}",
            self.server_name,
            hex::encode(loan_id.as_slice()),
            final_amount,
            block_number
        );
    }

    fn handle_withdrawal(&self, user: Address, amount: alloy_primitives::U256, block_number: u64) {
        info!(
            "[{}] Withdrawal: user={}, amount={}, block={}",
            self.server_name, user, amount, block_number
        );
    }
}
