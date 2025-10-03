//! Financial operation handlers
//!
//! This module contains handlers for financial operations including
//! transaction history and withdrawals.
//!
//! NOTE: The transactions module has been removed in the account-centric migration.
//! All /financial/* endpoints have been replaced with /account/* endpoints.
//! See: api/src/handlers/accounts/accounts.rs

pub mod transaction_history;
pub mod withdrawals;

// Re-export commonly used items
pub use transaction_history::*;
pub use withdrawals::*;
