//! Business logic module
//!
//! This module contains business logic for:
//! - Company access control and management
//! - Deal marketplace operations
//! - Deal ownership transfers

pub mod company;
pub mod marketplace;
pub mod transfers;

// Re-export commonly used types
pub use company::*;
pub use marketplace::*;
pub use transfers::*;
