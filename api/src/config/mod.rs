//! Configuration module
//!
//! This module contains configuration and type definitions:
//! - Email configuration and templates
//! - Magic link types
//! - Telemetry configuration
//! - Application types
//! - API paths

pub mod email_config;
pub mod email_templates;
pub mod magic_link_types;
pub mod paths;
pub mod telemetry;
pub mod types;

// Re-export commonly used items
pub use email_config::*;
pub use email_templates::*;
pub use magic_link_types::*;
pub use paths::*;
pub use telemetry::*;
pub use types::*;
