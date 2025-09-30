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
//! - `merchants`: Merchant profiles and operations
//! - `credit`: Credit line management
//! - `pools`: Liquidity pool operations
//! - `finance`: Financial transactions and withdrawals
//! - `ai`: AI-powered features

pub mod ai;
pub mod auth;
pub mod core;
pub mod credit;
pub mod deals;
pub mod finance;
pub mod merchants;
pub mod pools;
pub mod users;

// Re-export all handler functions for backward compatibility
pub use ai::*;
pub use auth::*;
pub use core::*;
pub use credit::*;
pub use deals::*;
pub use finance::*;
pub use merchants::*;
pub use pools::*;
pub use users::*;
