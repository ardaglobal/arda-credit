//! Authentication handlers
//!
//! This module contains handlers for user authentication,
//! including standard authentication and magic link flows.

pub mod authentication;
pub mod magic_link;

// Re-export commonly used items
pub use authentication::*;
pub use magic_link::*;
