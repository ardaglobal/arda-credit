//! Collateral Database Operations
//!
//! Provides CRUD operations for collateral assets with duplicate detection support.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tracing::instrument;
use uuid::Uuid;

use crate::error::DbError;

/// Collateral record with full attributes
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Collateral {
    pub id: Uuid,
    pub name: String,
    pub collateral_type: Option<String>,
    pub description: Option<String>,
    pub address: Option<String>,
    pub custom_id: Option<String>,
    pub double_pledged: Option<Uuid>,
    pub has_potential_duplicates: Option<bool>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: Option<Uuid>,
    pub metadata: Option<serde_json::Value>,
    // Detection metadata from arda-collateral-intel
    pub has_detection: bool,
    pub detection_count: i32,
    pub last_detection_at: Option<DateTime<Utc>>,
    pub detection_status: Option<String>,
}

/// Request to create a collateral record
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateCollateralRequest {
    pub name: String, // Required field
    pub collateral_type: Option<String>,
    pub description: Option<String>,
    pub address: Option<String>,
    pub custom_id: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub created_by: Option<Uuid>,
}

/// Request to update a collateral record
#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateCollateralRequest {
    pub name: Option<String>,
    pub collateral_type: Option<String>,
    pub description: Option<String>,
    pub address: Option<String>,
    pub custom_id: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

/// Create a new collateral record and initiate duplicate check
#[instrument(skip(pool))]
pub async fn create_collateral(
    pool: &PgPool,
    req: CreateCollateralRequest,
) -> Result<Collateral, DbError> {
    let id = Uuid::new_v4();
    let now = Utc::now();

    let record = sqlx::query_as!(
        Collateral,
        r#"
        INSERT INTO collateral (
            id, name, collateral_type, description, address, custom_id,
            created_at, updated_at, created_by, metadata
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        RETURNING
            id, name, collateral_type, description, address, custom_id,
            double_pledged, has_potential_duplicates,
            created_at as "created_at!", updated_at as "updated_at!",
            created_by, metadata,
            has_detection as "has_detection!", detection_count as "detection_count!",
            last_detection_at, detection_status
        "#,
        id,
        req.name,
        req.collateral_type,
        req.description,
        req.address,
        req.custom_id,
        now,
        now,
        req.created_by,
        req.metadata,
    )
    .fetch_one(pool)
    .await
    .map_err(DbError::Database)?;

    Ok(record)
}

/// Get collateral by ID
#[instrument(skip(pool))]
pub async fn get_collateral_by_id(pool: &PgPool, id: Uuid) -> Result<Collateral, DbError> {
    sqlx::query_as!(
        Collateral,
        r#"
        SELECT
            id, name, collateral_type, description, address, custom_id,
            double_pledged, has_potential_duplicates,
            created_at as "created_at!", updated_at as "updated_at!",
            created_by, metadata,
            has_detection as "has_detection!", detection_count as "detection_count!",
            last_detection_at, detection_status
        FROM collateral
        WHERE id = $1
        "#,
        id
    )
    .fetch_one(pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => DbError::NotFound(format!("Collateral {} not found", id)),
        _ => DbError::Database(e),
    })
}

/// Get collateral by custom ID
#[instrument(skip(pool))]
pub async fn get_collateral_by_custom_id(
    pool: &PgPool,
    custom_id: &str,
) -> Result<Collateral, DbError> {
    sqlx::query_as!(
        Collateral,
        r#"
        SELECT
            id, name, collateral_type, description, address, custom_id,
            double_pledged, has_potential_duplicates,
            created_at as "created_at!", updated_at as "updated_at!",
            created_by, metadata,
            has_detection as "has_detection!", detection_count as "detection_count!",
            last_detection_at, detection_status
        FROM collateral
        WHERE custom_id = $1
        "#,
        custom_id
    )
    .fetch_one(pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => {
            DbError::NotFound(format!("Collateral with custom_id {} not found", custom_id))
        }
        _ => DbError::Database(e),
    })
}

/// List collateral with optional filtering
#[instrument(skip(pool))]
pub async fn list_collateral(
    pool: &PgPool,
    collateral_type: Option<String>,
    created_by: Option<Uuid>,
    limit: i64,
    offset: i64,
) -> Result<Vec<Collateral>, DbError> {
    sqlx::query_as!(
        Collateral,
        r#"
        SELECT
            id, name, collateral_type, description, address, custom_id,
            double_pledged, has_potential_duplicates,
            created_at as "created_at!", updated_at as "updated_at!",
            created_by, metadata,
            has_detection as "has_detection!", detection_count as "detection_count!",
            last_detection_at, detection_status
        FROM collateral
        WHERE
            ($1::text IS NULL OR collateral_type = $1) AND
            ($2::uuid IS NULL OR created_by = $2)
        ORDER BY created_at DESC
        LIMIT $3 OFFSET $4
        "#,
        collateral_type,
        created_by,
        limit,
        offset
    )
    .fetch_all(pool)
    .await
    .map_err(DbError::Database)
}

/// Update collateral
#[instrument(skip(pool))]
pub async fn update_collateral(
    pool: &PgPool,
    id: Uuid,
    req: UpdateCollateralRequest,
) -> Result<Collateral, DbError> {
    // Build dynamic update query
    let mut query = String::from("UPDATE collateral SET ");
    let mut updates = Vec::new();
    let mut param_count = 1;

    if req.name.is_some() {
        updates.push(format!("name = ${}", param_count));
        param_count += 1;
    }
    if req.collateral_type.is_some() {
        updates.push(format!("collateral_type = ${}", param_count));
        param_count += 1;
    }
    if req.description.is_some() {
        updates.push(format!("description = ${}", param_count));
        param_count += 1;
    }
    if req.address.is_some() {
        updates.push(format!("address = ${}", param_count));
        param_count += 1;
    }
    if req.custom_id.is_some() {
        updates.push(format!("custom_id = ${}", param_count));
        param_count += 1;
    }
    if req.metadata.is_some() {
        updates.push(format!("metadata = ${}", param_count));
        param_count += 1;
    }

    if updates.is_empty() {
        return get_collateral_by_id(pool, id).await;
    }

    query.push_str(&updates.join(", "));
    query.push_str(&format!(" WHERE id = ${}", param_count));
    query.push_str(" RETURNING id, name, collateral_type, description, address, custom_id, double_pledged, has_potential_duplicates, created_at, updated_at, created_by, metadata, has_detection, detection_count, last_detection_at, detection_status");

    let mut query_builder = sqlx::query_as::<_, Collateral>(&query);

    if let Some(name) = req.name {
        query_builder = query_builder.bind(name);
    }
    if let Some(collateral_type) = req.collateral_type {
        query_builder = query_builder.bind(collateral_type);
    }
    if let Some(description) = req.description {
        query_builder = query_builder.bind(description);
    }
    if let Some(address) = req.address {
        query_builder = query_builder.bind(address);
    }
    if let Some(custom_id) = req.custom_id {
        query_builder = query_builder.bind(custom_id);
    }
    if let Some(metadata) = req.metadata {
        query_builder = query_builder.bind(metadata);
    }

    query_builder = query_builder.bind(id);

    query_builder.fetch_one(pool).await.map_err(|e| match e {
        sqlx::Error::RowNotFound => DbError::NotFound(format!("Collateral {} not found", id)),
        _ => DbError::TransactionFailed(format!("Failed to update collateral: {}", e)),
    })
}

/// Delete collateral
#[instrument(skip(pool))]
pub async fn delete_collateral(pool: &PgPool, id: Uuid) -> Result<(), DbError> {
    let result = sqlx::query!(
        r#"
        DELETE FROM collateral WHERE id = $1
        "#,
        id
    )
    .execute(pool)
    .await
    .map_err(|e| DbError::TransactionFailed(format!("Failed to delete collateral: {}", e)))?;

    if result.rows_affected() == 0 {
        return Err(DbError::NotFound(format!("Collateral {} not found", id)));
    }

    Ok(())
}

/// Get collateral by deal ID
#[instrument(skip(pool))]
pub async fn get_collateral_by_deal_id(
    pool: &PgPool,
    deal_id: Uuid,
) -> Result<Option<Collateral>, DbError> {
    sqlx::query_as!(
        Collateral,
        r#"
        SELECT
            c.id, c.name, c.collateral_type, c.description, c.address, c.custom_id,
            c.double_pledged, c.has_potential_duplicates,
            c.created_at as "created_at!", c.updated_at as "updated_at!",
            c.created_by, c.metadata,
            c.has_detection as "has_detection!", c.detection_count as "detection_count!",
            c.last_detection_at, c.detection_status
        FROM collateral c
        INNER JOIN deals d ON d.collateral_id = c.id
        WHERE d.id = $1
        "#,
        deal_id
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| DbError::TransactionFailed(format!("Failed to get collateral by deal_id: {}", e)))
}

/// Batch create collateral records
#[instrument(skip(pool))]
pub async fn create_collateral_batch(
    pool: &PgPool,
    requests: Vec<CreateCollateralRequest>,
) -> Result<Vec<Collateral>, DbError> {
    let mut results = Vec::new();

    for req in requests {
        let collateral = create_collateral(pool, req).await?;
        results.push(collateral);
    }

    Ok(results)
}

/// Update collateral detection status based on detection results
///
/// Sets detection metadata from arda-collateral-intel and optionally sets double_pledged to highest match
#[instrument(skip(pool))]
pub async fn update_collateral_detection_status(
    pool: &PgPool,
    collateral_id: Uuid,
    has_detection: bool,
    detection_count: i32,
    detection_status: Option<String>,
    last_detection_at: DateTime<Utc>,
    highest_match_id: Option<Uuid>,
) -> Result<Collateral, DbError> {
    // Also set has_potential_duplicates for backward compatibility
    let has_duplicates = detection_count > 0;

    sqlx::query_as!(
        Collateral,
        r#"
        UPDATE collateral
        SET
            has_detection = $2,
            detection_count = $3,
            detection_status = $4,
            last_detection_at = $5,
            has_potential_duplicates = $6,
            double_pledged = $7
        WHERE id = $1
        RETURNING
            id, name, collateral_type, description, address, custom_id,
            double_pledged, has_potential_duplicates,
            created_at as "created_at!", updated_at as "updated_at!",
            created_by, metadata,
            has_detection as "has_detection!", detection_count as "detection_count!",
            last_detection_at, detection_status
        "#,
        collateral_id,
        has_detection,
        detection_count,
        detection_status,
        last_detection_at,
        has_duplicates,
        highest_match_id
    )
    .fetch_one(pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => {
            DbError::NotFound(format!("Collateral {} not found", collateral_id))
        }
        _ => DbError::TransactionFailed(format!(
            "Failed to update collateral detection status: {}",
            e
        )),
    })
}

/// Collateral match record from detection system
#[derive(Debug, Clone)]
pub struct CollateralMatchInsert {
    pub matched_collateral_id: Uuid,
    pub match_score: f32,
    pub match_type: String,
}

/// Store collateral matches from detection results
///
/// Inserts or updates matches in collateral_matches table
/// Skips matches where the matched_collateral_id doesn't exist (handles stale data gracefully)
#[instrument(skip(pool))]
pub async fn store_collateral_matches(
    pool: &PgPool,
    collateral_id: Uuid,
    matches: Vec<CollateralMatchInsert>,
) -> Result<usize, DbError> {
    let mut count = 0;

    for m in matches {
        // Detect self-double-pledge case (same collateral UUID in multiple deals)
        let is_self_match = collateral_id == m.matched_collateral_id;

        // For self-matches, override match_type to 'self_double_pledge'
        // This is the purest form of double-pledge detection
        let match_type = if is_self_match {
            "self_double_pledge".to_string()
        } else {
            m.match_type.clone()
        };

        // Check if the matched collateral exists before inserting
        // Skip this check for self-matches (we already know the collateral exists)
        if !is_self_match {
            let matched_exists = sqlx::query!(
                "SELECT EXISTS(SELECT 1 FROM collateral WHERE id = $1) as exists",
                m.matched_collateral_id
            )
            .fetch_one(pool)
            .await
            .map_err(|e| DbError::Database(e))?
            .exists
            .unwrap_or(false);

            if !matched_exists {
                // Skip this match - the matched collateral doesn't exist in our database
                // This can happen when collateral-intel has stale data from previous test runs
                tracing::debug!(
                    "Skipping match {} -> {} (matched collateral not found in database)",
                    collateral_id,
                    m.matched_collateral_id
                );
                continue;
            }
        }

        // Use ON CONFLICT to handle duplicates (upsert)
        sqlx::query!(
            r#"
            INSERT INTO collateral_matches (collateral_id, matched_collateral_id, match_score, match_type)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (collateral_id, matched_collateral_id)
            DO UPDATE SET
                match_score = EXCLUDED.match_score,
                match_type = EXCLUDED.match_type,
                created_at = NOW()
            "#,
            collateral_id,
            m.matched_collateral_id,
            m.match_score as f64, // Convert to f64 for DECIMAL type
            match_type  // Use corrected match_type (self_double_pledge for self-matches)
        )
        .execute(pool)
        .await
        .map_err(|e| DbError::TransactionFailed(format!("Failed to store collateral match: {}", e)))?;

        count += 1;
    }

    Ok(count)
}

/// Get all deal IDs that reference a specific collateral
#[instrument(skip(pool))]
pub async fn get_deals_by_collateral_id(
    pool: &PgPool,
    collateral_id: Uuid,
) -> Result<Vec<Uuid>, DbError> {
    let rows = sqlx::query!(
        r#"
        SELECT id
        FROM deals
        WHERE collateral_id = $1
        ORDER BY created_at DESC
        "#,
        collateral_id
    )
    .fetch_all(pool)
    .await
    .map_err(DbError::Database)?;

    Ok(rows.into_iter().map(|r| r.id).collect())
}

/// Collateral with additional deal count metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollateralWithDealCount {
    pub collateral: Collateral,
    pub deal_count: i64,
    pub deal_ids: Vec<Uuid>,
}

/// Get collateral by ID with deal count
#[instrument(skip(pool))]
pub async fn get_collateral_with_deal_count(
    pool: &PgPool,
    collateral_id: Uuid,
) -> Result<CollateralWithDealCount, DbError> {
    let collateral = get_collateral_by_id(pool, collateral_id).await?;
    let deal_ids = get_deals_by_collateral_id(pool, collateral_id).await?;
    let deal_count = deal_ids.len() as i64;

    Ok(CollateralWithDealCount {
        collateral,
        deal_count,
        deal_ids,
    })
}

/// List collateral that is used in multiple deals (potential self double-pledge)
#[instrument(skip(pool))]
pub async fn list_multi_deal_collateral(
    pool: &PgPool,
    limit: i64,
    offset: i64,
) -> Result<Vec<CollateralWithDealCount>, DbError> {
    #[derive(sqlx::FromRow)]
    struct CollateralWithCount {
        id: Uuid,
        name: String,
        collateral_type: Option<String>,
        description: Option<String>,
        address: Option<String>,
        custom_id: Option<String>,
        double_pledged: Option<Uuid>,
        has_potential_duplicates: Option<bool>,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
        created_by: Option<Uuid>,
        metadata: Option<serde_json::Value>,
        has_detection: bool,
        detection_count: i32,
        last_detection_at: Option<DateTime<Utc>>,
        detection_status: Option<String>,
        deal_count: i64,
        deal_ids: Option<Vec<Uuid>>,
    }

    let rows = sqlx::query_as!(
        CollateralWithCount,
        r#"
        SELECT 
            c.id, c.name, c.collateral_type, c.description, c.address, 
            c.custom_id, c.double_pledged, c.has_potential_duplicates,
            c.created_at as "created_at!", c.updated_at as "updated_at!",
            c.created_by, c.metadata,
            c.has_detection as "has_detection!", c.detection_count as "detection_count!",
            c.last_detection_at, c.detection_status,
            COUNT(d.id) as "deal_count!",
            ARRAY_AGG(d.id) FILTER (WHERE d.id IS NOT NULL) as "deal_ids"
        FROM collateral c
        LEFT JOIN deals d ON d.collateral_id = c.id
        GROUP BY c.id, c.name, c.collateral_type, c.description, c.address, 
                 c.custom_id, c.double_pledged, c.has_potential_duplicates,
                 c.created_at, c.updated_at, c.created_by, c.metadata,
                 c.has_detection, c.detection_count, c.last_detection_at, c.detection_status
        HAVING COUNT(d.id) > 1
        ORDER BY COUNT(d.id) DESC, c.created_at DESC
        LIMIT $1 OFFSET $2
        "#,
        limit,
        offset
    )
    .fetch_all(pool)
    .await
    .map_err(DbError::Database)?;

    Ok(rows
        .into_iter()
        .map(|row| {
            let collateral = Collateral {
                id: row.id,
                name: row.name,
                collateral_type: row.collateral_type,
                description: row.description,
                address: row.address,
                custom_id: row.custom_id,
                double_pledged: row.double_pledged,
                has_potential_duplicates: row.has_potential_duplicates,
                created_at: row.created_at,
                updated_at: row.updated_at,
                created_by: row.created_by,
                metadata: row.metadata,
                has_detection: row.has_detection,
                detection_count: row.detection_count,
                last_detection_at: row.last_detection_at,
                detection_status: row.detection_status,
            };
            CollateralWithDealCount {
                collateral,
                deal_count: row.deal_count,
                deal_ids: row.deal_ids.unwrap_or_default(),
            }
        })
        .collect())
}

// ============================================================================
// COLLATERAL TRANSACTION RECORDING (Audit Trail)
// ============================================================================

/// Collateral transaction record for audit trail
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CollateralTransaction {
    pub transaction_id: Uuid,
    pub collateral_id: Uuid,
    pub transaction_type: String,
    pub created_at: DateTime<Utc>,
    pub created_by: Option<Uuid>,
    pub deal_id: Option<Uuid>,
    pub metadata: Option<serde_json::Value>,
}

/// Collateral transaction with user details (for audit trail display)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollateralTransactionWithUser {
    pub transaction_id: Uuid,
    pub collateral_id: Uuid,
    pub transaction_type: String,
    pub created_at: DateTime<Utc>,
    pub created_by: Option<Uuid>,
    pub user_email: Option<String>,
    pub deal_id: Option<Uuid>,
    pub metadata: Option<serde_json::Value>,
}

/// Generic function to record a collateral transaction
#[instrument(skip(pool))]
pub async fn record_collateral_transaction(
    pool: &PgPool,
    collateral_id: Uuid,
    transaction_type: &str,
    created_by: Option<Uuid>,
    deal_id: Option<Uuid>,
    metadata: Option<serde_json::Value>,
) -> Result<CollateralTransaction, DbError> {
    let transaction = sqlx::query_as!(
        CollateralTransaction,
        r#"
        INSERT INTO collateral_transactions (
            collateral_id, transaction_type, created_by, deal_id, metadata
        )
        VALUES ($1, $2, $3, $4, $5)
        RETURNING
            transaction_id,
            collateral_id,
            transaction_type,
            created_at as "created_at!",
            created_by,
            deal_id,
            metadata
        "#,
        collateral_id,
        transaction_type,
        created_by,
        deal_id,
        metadata
    )
    .fetch_one(pool)
    .await
    .map_err(|e| {
        DbError::TransactionFailed(format!("Failed to record collateral transaction: {}", e))
    })?;

    Ok(transaction)
}

/// Record collateral creation transaction
#[instrument(skip(pool))]
pub async fn record_collateral_creation_transaction(
    pool: &PgPool,
    collateral_id: Uuid,
    created_by: Option<Uuid>,
    initial_properties: serde_json::Value,
) -> Result<CollateralTransaction, DbError> {
    let metadata = serde_json::json!({
        "initial_properties": initial_properties
    });

    record_collateral_transaction(
        pool,
        collateral_id,
        "collateral_creation",
        created_by,
        None,
        Some(metadata),
    )
    .await
}

/// Record collateral update transaction with field changes
#[instrument(skip(pool))]
pub async fn record_collateral_update_transaction(
    pool: &PgPool,
    collateral_id: Uuid,
    updated_by: Option<Uuid>,
    changes: serde_json::Value,
) -> Result<CollateralTransaction, DbError> {
    let metadata = serde_json::json!({
        "changes": changes
    });

    record_collateral_transaction(
        pool,
        collateral_id,
        "collateral_update",
        updated_by,
        None,
        Some(metadata),
    )
    .await
}

/// Record collateral-deal association transaction
#[instrument(skip(pool))]
pub async fn record_collateral_deal_association_transaction(
    pool: &PgPool,
    collateral_id: Uuid,
    deal_id: Uuid,
    created_by: Option<Uuid>,
) -> Result<CollateralTransaction, DbError> {
    let metadata = serde_json::json!({
        "deal_id": deal_id.to_string(),
        "action": "associated"
    });

    record_collateral_transaction(
        pool,
        collateral_id,
        "collateral_deal_association",
        created_by,
        Some(deal_id),
        Some(metadata),
    )
    .await
}

/// Record collateral-deal disassociation transaction
#[instrument(skip(pool))]
pub async fn record_collateral_deal_disassociation_transaction(
    pool: &PgPool,
    collateral_id: Uuid,
    deal_id: Uuid,
    created_by: Option<Uuid>,
) -> Result<CollateralTransaction, DbError> {
    let metadata = serde_json::json!({
        "deal_id": deal_id.to_string(),
        "action": "disassociated"
    });

    record_collateral_transaction(
        pool,
        collateral_id,
        "collateral_deal_disassociation",
        created_by,
        Some(deal_id),
        Some(metadata),
    )
    .await
}

/// Record collateral detection request transaction
#[instrument(skip(pool))]
pub async fn record_collateral_detection_request_transaction(
    pool: &PgPool,
    collateral_id: Uuid,
    requested_by: Option<Uuid>,
    trigger_type: &str,
) -> Result<CollateralTransaction, DbError> {
    let metadata = serde_json::json!({
        "trigger_type": trigger_type,
        "requested_by": requested_by.map(|u| u.to_string())
    });

    record_collateral_transaction(
        pool,
        collateral_id,
        "collateral_detection_request",
        requested_by,
        None,
        Some(metadata),
    )
    .await
}

/// Record collateral detection update transaction
#[instrument(skip(pool))]
pub async fn record_collateral_detection_update_transaction(
    pool: &PgPool,
    collateral_id: Uuid,
    detection_status: &str,
    detection_count: i32,
) -> Result<CollateralTransaction, DbError> {
    let metadata = serde_json::json!({
        "detection_status": detection_status,
        "detection_count": detection_count
    });

    record_collateral_transaction(
        pool,
        collateral_id,
        "collateral_detection_update",
        None, // System event
        None,
        Some(metadata),
    )
    .await
}

/// Get collateral transaction history with user details
#[instrument(skip(pool))]
pub async fn get_collateral_transaction_history(
    pool: &PgPool,
    collateral_id: Uuid,
    limit: Option<i64>,
) -> Result<Vec<CollateralTransactionWithUser>, DbError> {
    let limit = limit.unwrap_or(100);

    let rows = sqlx::query!(
        r#"
        SELECT 
            ct.transaction_id,
            ct.collateral_id,
            ct.transaction_type,
            ct.created_at,
            ct.created_by,
            u.email as "user_email?",
            ct.deal_id,
            ct.metadata
        FROM collateral_transactions ct
        LEFT JOIN users u ON ct.created_by = u.id
        WHERE ct.collateral_id = $1
        ORDER BY ct.created_at ASC
        LIMIT $2
        "#,
        collateral_id,
        limit
    )
    .fetch_all(pool)
    .await
    .map_err(DbError::Database)?;

    let transactions = rows
        .into_iter()
        .map(|row| CollateralTransactionWithUser {
            transaction_id: row.transaction_id,
            collateral_id: row.collateral_id,
            transaction_type: row.transaction_type,
            created_at: row.created_at,
            created_by: row.created_by,
            user_email: row.user_email,
            deal_id: row.deal_id,
            metadata: row.metadata,
        })
        .collect();

    Ok(transactions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires database
    async fn test_create_collateral() {
        let pool = crate::init_db().await.unwrap();

        let req = CreateCollateralRequest {
            name: "Test Collateral".to_string(),
            collateral_type: Some("real_estate".to_string()),
            description: Some("Test property".to_string()),
            address: Some("123 Main St".to_string()),
            custom_id: Some("CUST-001".to_string()),
            metadata: None,
            created_by: None,
        };

        let result = create_collateral(&pool, req).await;
        assert!(result.is_ok());

        let collateral = result.unwrap();
        assert_eq!(collateral.name, "Test Collateral");
        assert_eq!(collateral.collateral_type, Some("real_estate".to_string()));
    }
}
