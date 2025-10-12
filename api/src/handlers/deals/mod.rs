//! Deal handlers
//!
//! This module contains handlers for deal operations including
//! deal creation, payments, marketplace, transfers, and documents.

pub mod deals;
pub mod documents;

// Re-export commonly used items
pub use deals::*;
pub use documents::*;
