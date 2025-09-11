//! Integration tests for the Arda Credit API
//!
//! This module contains integration tests for all API endpoints.
//! Each handler module has its own test file for focused testing.
//!
//! ## Test Organization
//!
//! - `auth_tests.rs` - Authentication and authorization tests
//! - `user_tests.rs` - User management tests
//! - `loan_tests.rs` - Loan lifecycle tests
//! - `pool_tests.rs` - Liquidity pool tests
//! - `credit_line_tests.rs` - Credit line tests
//! - `financial_tests.rs` - Financial transaction tests
//! - `admin_tests.rs` - Administrative function tests
//!
//! ## Running Tests
//!
//! ```bash
//! # Run all integration tests
//! cargo test --test integration
//!
//! # Run specific test module
//! cargo test --test integration auth_tests
//!
//! # Run with database logging
//! DATABASE_URL=postgres://user:pass@localhost:5432/test_db cargo test --test integration
//! ```

pub mod auth_tests;
pub mod user_tests;
pub mod loan_tests;
pub mod pool_tests;
pub mod credit_line_tests;
pub mod financial_tests;
pub mod admin_tests;

// Re-export common test utilities
pub use crate::test_utils::*;
