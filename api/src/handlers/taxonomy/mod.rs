//! Taxonomy API Handlers
//!
//! This module contains handlers for Arda taxonomy tables that store
//! structured information extracted from legal and financial documents.
//!
//! ## Organization
//!
//! Each taxonomy table has its own module with standard CRUD operations:
//! - `GET /api/v1/taxonomy/{table}` - List with pagination/filtering
//! - `GET /api/v1/taxonomy/{table}/{id}` - Get single record
//! - `POST /api/v1/taxonomy/{table}` - Create single record
//! - `POST /api/v1/taxonomy/{table}/batch` - Create multiple records
//! - `PUT /api/v1/taxonomy/{table}/{id}` - Update record (full replacement)
//! - `DELETE /api/v1/taxonomy/{table}/{id}` - Delete record
//!
//! ## Taxonomy Tables
//!
//! - `credit_agreements` - Core credit agreement documents
//! - `fee_letters` - Fee structures and schedules
//! - `legal_opinions` - Legal enforceability opinions
//! - `security_agreements` - Collateral and security interests
//! - `ucc_financing_statements` - UCC-1 filings for perfection
//! - `account_control_agreements` - DACA/ACA/SACA for account control
//! - `guaranties` - Guaranty agreements
//! - `assignment_assumption_agreements` - Loan assignment transfers
//! - `participation_agreements` - Economic participation interests
//! - `ssi_records` - Standard Settlement Instructions
//! - `secondary_trades` - Secondary market trading
//!
//! ## Helper Functions
//!
//! The `helpers` module provides specialized endpoints for calculations
//! and validations based on database helper functions.

pub mod account_control_agreements;
pub mod assignments;
pub mod common;
pub mod credit_agreements;
pub mod fee_letters;
pub mod guaranties;
pub mod helpers;
pub mod legal_opinions;
pub mod participations;
pub mod secondary_trades;
pub mod security_agreements;
pub mod ssi_records;
pub mod ucc_financing_statements;

// Re-export commonly used items
pub use account_control_agreements::*;
pub use assignments::*;
pub use common::*;
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
