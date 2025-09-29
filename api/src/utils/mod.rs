//! Utilities module
//!
//! This module contains utility functions and helpers:
//! - Document metrics
//! - Magic link cleanup jobs
//! - Metrics collection
//! - Session cleanup jobs
//! - Test utilities
//! - OpenAPI tests

pub mod document_metrics;
pub mod magic_link_cleanup;
pub mod metrics;
pub mod session_cleanup;

#[cfg(test)]
pub mod openapi_test;

#[cfg(test)]
pub mod test_utils;

// Re-export commonly used items
pub use metrics::*;
