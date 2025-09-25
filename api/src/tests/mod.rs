//! Integration tests for the Arda Credit API
//!
//! This module contains integration tests for all API endpoints.
//! Each handler module has its own test file for focused testing.
//!
//! ## Test Organization
//!
//! - `auth_tests.rs` - Authentication and authorization tests
//! - `email_service_test.rs` - Email service integration tests
//! - `user_tests.rs` - User management tests
//! - `deal_tests.rs` - Deal lifecycle tests (converted from loan_tests.rs)
//! - `pool_tests.rs` - Liquidity pool tests
//! - `credit_line_tests.rs` - Credit line tests
//! - `financial_tests.rs` - Financial transaction tests
//! - `admin_tests.rs` - Administrative function tests
//!
//! ## Recent Changes
//!
//! Converted from loan-based tests to deal-based tests:
//! - `loan_tests.rs` → `deal_tests.rs`
//! - All loan references updated to deal terminology
//! - Marketplace and transfer tests consolidated into deal tests
//! - All endpoints updated to use deal terminology
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
//! # Run deal tests
//! cargo test --test integration deal_tests
//!
//! # Run with database logging
//! DATABASE_URL=postgres://user:pass@localhost:5432/test_db cargo test --test integration
//! ```

pub mod auth_tests;
pub mod email_service_test;
pub mod user_tests;
pub mod deal_tests;
pub mod pool_tests;
pub mod credit_line_tests;
pub mod financial_tests;
pub mod admin_tests;
pub mod magic_link_integration_test;
pub mod magic_link_security_test;
pub mod magic_link_error_test;
pub mod magic_link_performance_test;

// Re-export common test utilities
pub use crate::test_utils::*;
