//! Database layer telemetry and instrumentation utilities
//!
//! This module provides standardized tracing utilities for database operations
//! in the Arda Credit system. It ensures consistent span naming, field extraction,
//! and performance monitoring across all database interactions.

use tracing::{info_span, Span};
use uuid::Uuid;

/// Database operation instrumentation utilities
pub mod database {
    use super::*;

    /// Standard database query instrumentation
    ///
    /// Creates a standardized span for database queries with consistent naming
    /// and field structure for observability.
    ///
    /// # Arguments
    /// * `operation` - The database operation (e.g., "select", "insert", "update")
    /// * `table` - The target table name
    ///
    /// # Example
    /// ```rust
    /// # use db::telemetry::database::instrument_query;
    /// let _span = instrument_query("select", "users").entered();
    /// ```
    pub fn instrument_query(operation: &str, table: &str) -> Span {
        info_span!(
            "db_query",
            operation = operation,
            table = table,
            layer = "database",
            db_type = "postgresql"
        )
    }

    /// Transaction lifecycle instrumentation
    ///
    /// Creates spans for tracking database transactions including batches,
    /// commits, and rollbacks.
    ///
    /// # Arguments
    /// * `transaction_type` - Type of transaction (e.g., "batch", "single", "bulk")
    ///
    /// # Example
    /// ```rust
    /// # use db::telemetry::database::instrument_transaction;
    /// let _span = instrument_transaction("batch_insert").entered();
    /// ```
    pub fn instrument_transaction(transaction_type: &str) -> Span {
        info_span!(
            "db_transaction",
            transaction_type = transaction_type,
            layer = "database",
            db_type = "postgresql"
        )
    }

    /// User-specific database operation instrumentation
    ///
    /// Creates spans for operations that involve user data, including
    /// the user ID for correlation and debugging.
    ///
    /// # Arguments
    /// * `operation` - The operation being performed
    /// * `user_id` - Optional user UUID for correlation
    ///
    /// # Example
    /// ```rust
    /// # use db::telemetry::database::instrument_user_operation;
    /// # use uuid::Uuid;
    /// # let user_uuid = Uuid::new_v4();
    /// let _span = instrument_user_operation("kyc_validation", Some(user_uuid)).entered();
    /// ```
    pub fn instrument_user_operation(operation: &str, user_id: Option<Uuid>) -> Span {
        info_span!("db_user_operation",
            operation = operation,
            user_id = ?user_id,
            layer = "database",
            db_type = "postgresql"
        )
    }

    /// Batch processing instrumentation
    ///
    /// Creates spans for batch operations with size tracking for performance
    /// monitoring and optimization.
    ///
    /// # Arguments
    /// * `operation` - The batch operation type
    /// * `batch_size` - Number of items in the batch
    ///
    /// # Example
    /// ```rust
    /// # use db::telemetry::database::instrument_batch_operation;
    /// let _span = instrument_batch_operation("bulk_insert", 150).entered();
    /// ```
    pub fn instrument_batch_operation(operation: &str, batch_size: usize) -> Span {
        info_span!(
            "db_batch_operation",
            operation = operation,
            batch_size = batch_size,
            layer = "database",
            db_type = "postgresql"
        )
    }

    /// ADS/Merkle tree operation instrumentation
    ///
    /// Creates spans for cryptographic operations including state commits,
    /// proof generation, and tree updates.
    ///
    /// # Arguments
    /// * `operation` - The cryptographic operation
    /// * `tree_type` - Type of tree (e.g., "indexed_merkle_tree", "sparse_merkle_tree")
    ///
    /// # Example
    /// ```rust
    /// # use db::telemetry::database::instrument_ads_operation;
    /// let _span = instrument_ads_operation("state_commit", "indexed_merkle_tree").entered();
    /// ```
    pub fn instrument_ads_operation(operation: &str, tree_type: &str) -> Span {
        info_span!(
            "db_ads_operation",
            operation = operation,
            tree_type = tree_type,
            layer = "database",
            component = "ads"
        )
    }
}

/// Reusable instrumentation macros for database operations
///
/// These macros provide compile-time instrumentation with zero runtime overhead
/// when tracing is disabled.

/// Instrument a database function with standard fields
///
/// # Arguments
/// * `$operation` - String literal for the operation name
/// * `$table` - String literal for the table name
///
/// # Example
/// ```rust,ignore
/// # use sqlx::PgPool;
/// # use serde::{Deserialize, Serialize};
/// # #[derive(Debug, Serialize, Deserialize)]
/// # struct User { id: i32, email: String }
/// # use db::instrument_db_fn;
///
/// #[instrument_db_fn!("create_identity", "users")]
/// pub async fn create_identity(pool: &PgPool, email: String) -> Result<User, sqlx::Error> {
///     // function implementation would go here
///     # Ok(User { id: 1, email })
/// }
/// ```
#[macro_export]
macro_rules! instrument_db_fn {
    ($operation:expr, $table:expr) => {
        #[tracing::instrument(skip_all, fields(
            operation = $operation,
            table = $table,
            layer = "database",
            db_type = "postgresql"
        ), level = "info")]
    };
}

/// Instrument a database function with user context
///
/// # Arguments
/// * `$operation` - String literal for the operation name
/// * `$table` - String literal for the table name
/// * `$user_field` - Field name containing the user ID
///
/// # Example
/// ```rust,ignore
/// # use sqlx::PgPool;
/// # use uuid::Uuid;
/// # use serde::{Deserialize, Serialize};
/// # #[derive(Debug, Serialize, Deserialize)]
/// # struct User { id: i32, email: String }
/// # use db::instrument_db_user_fn;
///
/// #[instrument_db_user_fn!("get_identity_by_id", "users", user_id)]
/// async fn get_identity_by_id(pool: &PgPool, user_id: Uuid) -> Result<Option<User>, sqlx::Error> {
///     // function implementation would go here
///     # Ok(Some(User { id: 1, email: "test@example.com".to_string() }))
/// }
/// ```
#[macro_export]
macro_rules! instrument_db_user_fn {
    ($operation:expr, $table:expr, $user_field:ident) => {
        #[tracing::instrument(skip_all, fields(
            operation = $operation,
            table = $table,
            layer = "database",
            db_type = "postgresql",
            user_id = %$user_field
        ), level = "info")]
    };
}

/// Instrument a database function with batch size tracking
///
/// # Arguments
/// * `$operation` - String literal for the operation name
/// * `$table` - String literal for the table name
/// * `$size_field` - Field name containing the batch size
///
/// # Example
/// ```rust,ignore
/// # use sqlx::PgPool;
/// # use db::instrument_db_batch_fn;
///
/// #[instrument_db_batch_fn!("bulk_insert_transactions", "state_transactions", batch_size)]
/// async fn bulk_insert_transactions(pool: &PgPool, batch_size: usize) -> Result<(), sqlx::Error> {
///     // function implementation would go here
///     # Ok(())
/// }
/// ```
#[macro_export]
macro_rules! instrument_db_batch_fn {
    ($operation:expr, $table:expr, $size_field:ident) => {
        #[tracing::instrument(skip_all, fields(
            operation = $operation,
            table = $table,
            layer = "database",
            db_type = "postgresql",
            batch_size = %$size_field
        ), level = "info")]
    };
}

/// Context utilities for cross-layer trace correlation
pub mod context {
    use tracing::Span;

    /// Attach request context to database operations
    ///
    /// This function helps correlate API requests with their corresponding
    /// database operations for end-to-end tracing.
    ///
    /// # Arguments
    /// * `request_id` - Optional request identifier from the API layer
    /// * `operation` - The database operation being performed
    pub fn with_request_context(request_id: Option<&str>, operation: &str) -> Span {
        match request_id {
            Some(id) => tracing::info_span!(
                "db_operation",
                operation = operation,
                request_id = id,
                layer = "database"
            ),
            None => tracing::info_span!("db_operation", operation = operation, layer = "database"),
        }
    }

    /// Create a child span for nested database operations
    ///
    /// Useful for complex operations that involve multiple database calls.
    ///
    /// # Arguments
    /// * `parent_operation` - The parent operation name
    /// * `child_operation` - The child operation name
    pub fn create_child_span(parent_operation: &str, child_operation: &str) -> Span {
        tracing::info_span!(
            "db_nested_operation",
            parent_operation = parent_operation,
            child_operation = child_operation,
            layer = "database"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_instrument_query_creates_correct_span() {
        let span = database::instrument_query("select", "users");
        assert_eq!(span.metadata().unwrap().name(), "db_query");
    }

    #[test]
    fn test_instrument_user_operation_with_uuid() {
        let user_id = Uuid::new_v4();
        let span = database::instrument_user_operation("create_user", Some(user_id));
        assert_eq!(span.metadata().unwrap().name(), "db_user_operation");
    }

    #[test]
    fn test_instrument_batch_operation_with_size() {
        let span = database::instrument_batch_operation("bulk_insert", 100);
        assert_eq!(span.metadata().unwrap().name(), "db_batch_operation");
    }
}
