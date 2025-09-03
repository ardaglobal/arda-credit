#![allow(
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::wildcard_imports
)]

pub mod client;
pub mod config;
pub mod contracts;
pub mod deposit_monitor;
pub mod error;
pub mod event_manager;
pub mod types;
pub mod withdrawal_monitor;

pub use client::{EthereumClient, EventCallback, EventFilter, LoanEvent, SubscriptionId};
pub use config::{Config, NetworkConfig};
pub use deposit_monitor::{DepositMonitor, DepositMonitorConfig, DepositRecord, DepositStatus};
pub use error::{EthereumError, Result};
pub use event_manager::{EventFilterBuilder, EventHandler, EventManager, VAppEventHandler};
pub use types::*;
pub use withdrawal_monitor::{
    WithdrawalMonitor, WithdrawalMonitorConfig, WithdrawalRecord, WithdrawalStatus,
};

#[cfg(feature = "database")]
pub mod cache;

#[cfg(feature = "database")]
pub use cache::EthereumCache;
