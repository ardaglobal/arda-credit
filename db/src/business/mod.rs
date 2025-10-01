//! Business logic module
//!
//! This module contains business logic for:
//! - Deal marketplace operations
//! - Deal ownership transfers

pub mod marketplace;
pub mod transfers;

// Re-export commonly used types
pub use marketplace::*;
pub use transfers::*;
