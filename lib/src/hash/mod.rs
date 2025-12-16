//! Hash functions for Merkle trees and authenticated data structures
//!
//! This module provides ZK-friendly hash functions that work consistently
//! between the zkVM and off-chain database implementations.

pub mod poseidon;

pub use poseidon::{hash_internal_poseidon, hash_leaf_poseidon};
