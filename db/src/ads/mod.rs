//! Authenticated Data Structures (ADS) module
//!
//! This module contains all ADS-related functionality including:
//! - Indexed Merkle Trees
//! - ADS Service
//! - vApp Integration
//! - Merkle Tree implementations

pub mod ads_service;
pub mod merkle_tree;
pub mod merkle_tree_32;
pub mod vapp_integration;

// Re-export commonly used types
pub use ads_service::*;
pub use merkle_tree::*;
pub use merkle_tree_32::*;
pub use vapp_integration::*;
