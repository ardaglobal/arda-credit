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
            double_pledged, has_potential_duplicates, created_at as "created_at!", updated_at as "updated_at!",
            created_by, metadata
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
            double_pledged, has_potential_duplicates, created_at as "created_at!", updated_at as "updated_at!",
            created_by, metadata
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
            double_pledged, has_potential_duplicates, created_at as "created_at!", updated_at as "updated_at!",
            created_by, metadata
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
            double_pledged, has_potential_duplicates, created_at as "created_at!", updated_at as "updated_at!",
            created_by, metadata
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
    query.push_str(" RETURNING id, name, collateral_type, description, address, custom_id, double_pledged, created_at, updated_at, created_by, metadata");

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
            c.double_pledged, c.has_potential_duplicates, c.created_at as "created_at!", c.updated_at as "updated_at!",
            c.created_by, c.metadata
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
/// Sets has_potential_duplicates flag and optionally sets double_pledged to highest match
#[instrument(skip(pool))]
pub async fn update_collateral_detection_status(
    pool: &PgPool,
    collateral_id: Uuid,
    has_duplicates: bool,
    highest_match_id: Option<Uuid>,
) -> Result<Collateral, DbError> {
    sqlx::query_as!(
        Collateral,
        r#"
        UPDATE collateral
        SET
            has_potential_duplicates = $2,
            double_pledged = $3
        WHERE id = $1
        RETURNING
            id, name, collateral_type, description, address, custom_id,
            double_pledged, has_potential_duplicates, created_at as "created_at!", updated_at as "updated_at!",
            created_by, metadata
        "#,
        collateral_id,
        has_duplicates,
        highest_match_id
    )
    .fetch_one(pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => DbError::NotFound(format!("Collateral {} not found", collateral_id)),
        _ => DbError::TransactionFailed(format!("Failed to update collateral detection status: {}", e)),
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
#[instrument(skip(pool))]
pub async fn store_collateral_matches(
    pool: &PgPool,
    collateral_id: Uuid,
    matches: Vec<CollateralMatchInsert>,
) -> Result<usize, DbError> {
    let mut count = 0;

    for m in matches {
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
            m.match_type
        )
        .execute(pool)
        .await
        .map_err(|e| DbError::TransactionFailed(format!("Failed to store collateral match: {}", e)))?;

        count += 1;
    }

    Ok(count)
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
