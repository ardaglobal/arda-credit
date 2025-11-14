//! Business logic module
//!
//! This module contains business logic for:
//! - Company access control and management
//! - Company wallet address management
//! - Deal marketplace operations
//! - Deal ownership transfers

pub mod company;
pub mod marketplace;
pub mod metrics;
pub mod transfers;
pub mod wallet;

// Re-export commonly used types
pub use company::*;
pub use marketplace::*;
pub use metrics::*;
pub use transfers::*;
pub use wallet::*;
