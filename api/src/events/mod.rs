//! Event publishing module for collateral intelligence service integration
//!
//! This module provides event types and publishing infrastructure for notifying
//! the arda-collateral-intel service of collateral and deal lifecycle changes.
//!
//! Events are published asynchronously in a fire-and-forget manner, ensuring
//! that failures in the intelligence service do not affect main API operations.

pub mod publisher;
pub mod types;

// Re-export commonly used types
pub use publisher::CollateralEventPublisher;
pub use types::{CollateralEvent, CollateralPayload, DealEvent, DealPayload, FieldChange};
