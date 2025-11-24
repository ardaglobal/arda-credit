//! Deal handlers
//!
//! This module contains handlers for deal operations including
//! deal creation, payments, marketplace, transfers, and document extraction.

pub mod deals;
pub mod document_extraction;

// Re-export commonly used items
pub use deals::*;
pub use document_extraction::*;
