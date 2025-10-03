//! API Request Handlers
//!
//! This module contains all API request handlers organized by domain.
//! Handlers are grouped into logical modules for better organization and maintainability.
//!
//! ## Module Organization
//!
//! - `core`: Core system operations (health, state, contracts)
//! - `auth`: Authentication and authorization
//! - `users`: User management and admin operations
//! - `deals`: Deal operations and marketplace
//! - `companies`: Originator and investor company profiles and documents
//! - `credit`: Credit line management
//! - `finance`: Financial transactions and withdrawals
//! - `ai`: AI-powered features

pub mod ai;
pub mod auth;
pub mod companies;
pub mod core;
pub mod deals;
pub mod faucet;
pub mod finance;
pub mod users;

// Re-export all handler functions for backward compatibility
pub use ai::*;
pub use auth::*;
pub use companies::*;
pub use core::*;
pub use deals::*;
pub use faucet::*;
pub use finance::*;
pub use users::*;
