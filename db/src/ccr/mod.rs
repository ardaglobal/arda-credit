//! CCR (Credit and Compliance Requirements) Module
//!
//! This module provides CCR-specific operations and functionality that extends
//! the existing loan system with enhanced nullifier generation and operation tracking.

pub mod operations;
pub mod transaction_mapper;

pub use operations::CcrOperation;
pub use transaction_mapper::{MappingContext, MappingError, TransactionMapper};