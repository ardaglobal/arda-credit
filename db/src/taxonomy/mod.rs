//! Arda Taxonomy Database Operations
//!
//! This module provides database operations for Arda taxonomy tables,
//! which store structured information extracted from legal and financial documents.

pub mod account_control_agreements;
pub mod assignments;
pub mod collateral;
pub mod credit_agreements;
pub mod fee_letters;
pub mod guaranties;
pub mod legal_opinions;
pub mod participations;
pub mod secondary_trades;
pub mod security_agreements;
pub mod ssi_records;
pub mod ucc_financing_statements;

// Re-export implemented types and functions
pub use account_control_agreements::*;
pub use assignments::*;
pub use collateral::*;
pub use credit_agreements::*;
pub use fee_letters::*;
pub use guaranties::*;
pub use legal_opinions::*;
pub use participations::*;
pub use secondary_trades::*;
pub use security_agreements::*;
pub use ssi_records::*;
pub use ucc_financing_statements::*;
// TODO: Add re-exports as other modules are implemented
