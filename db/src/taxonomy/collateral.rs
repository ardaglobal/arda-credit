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

/// Collateral check record for tracking duplicate detection
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CollateralCheck {
    pub id: Uuid,
    pub collateral_id: Uuid,
    pub checked_at: DateTime<Utc>,
    pub potential_matches: Option<Vec<Uuid>>,
    pub match_score: Option<rust_decimal::Decimal>,
    pub check_status: String,
    pub metadata: Option<serde_json::Value>,
}

/// Response with collateral and potential duplicates
#[derive(Debug, Serialize, Deserialize)]
pub struct CollateralWithDuplicates {
    pub collateral: Collateral,
    pub potential_duplicates: Vec<Collateral>,
}

/// Collateral match record from collateral_matches table
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CollateralMatch {
    pub id: Uuid,
    pub collateral_id: Uuid,
    pub matched_collateral_id: Uuid,
    pub match_score: rust_decimal::Decimal,
    pub match_type: String,
    pub created_at: DateTime<Utc>,
}

/// Queue entry for duplicate checks
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CollateralCheckQueueEntry {
    pub id: Uuid,
    pub collateral_id: Uuid,
    pub status: String,
    pub priority: i32,
    pub retry_count: i32,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

/// Response with collateral and all matches with scores
#[derive(Debug, Serialize, Deserialize)]
pub struct CollateralWithMatches {
    pub collateral: Collateral,
    pub matches: Vec<CollateralMatchDetail>,
}

/// Match detail with full collateral info and score
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollateralMatchDetail {
    pub matched_collateral: Collateral,
    pub match_score: rust_decimal::Decimal,
    pub match_type: String,
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

    // Add to duplicate check queue (instead of immediate spawn)
    sqlx::query!(
        r#"
        INSERT INTO collateral_check_queue (collateral_id, status, priority)
        VALUES ($1, 'pending', 0)
        ON CONFLICT (collateral_id) DO NOTHING
        "#,
        record.id
    )
    .execute(pool)
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

/// Check for duplicate collateral (async background process)
#[instrument(skip(pool))]
pub async fn check_for_duplicates(pool: &PgPool, collateral_id: Uuid) -> Result<(), DbError> {
    // Create a check record
    let check_id = Uuid::new_v4();
    sqlx::query!(
        r#"
        INSERT INTO collateral_checks (id, collateral_id, check_status)
        VALUES ($1, $2, 'in_progress')
        "#,
        check_id,
        collateral_id
    )
    .execute(pool)
    .await
    .map_err(|e| {
        DbError::TransactionFailed(format!("Failed to create collateral check record: {}", e))
    })?;

    // Get the collateral to check
    let collateral = get_collateral_by_id(pool, collateral_id).await?;

    // Find potential duplicates using various matching strategies
    let potential_matches = find_potential_matches(pool, &collateral).await?;

    if !potential_matches.is_empty() {
        // Update the original collateral with the first match
        let first_match_id = potential_matches[0];
        sqlx::query!(
            r#"
            UPDATE collateral
            SET double_pledged = $1
            WHERE id = $2
            "#,
            first_match_id,
            collateral_id
        )
        .execute(pool)
        .await
        .map_err(|e| {
            DbError::TransactionFailed(format!("Failed to update double_pledged: {}", e))
        })?;

        // Update check record with results
        sqlx::query!(
            r#"
            UPDATE collateral_checks
            SET
                check_status = 'completed',
                potential_matches = $1,
                match_score = 85.0
            WHERE id = $2
            "#,
            &potential_matches,
            check_id
        )
        .execute(pool)
        .await
        .map_err(|e| DbError::TransactionFailed(format!("Failed to update check record: {}", e)))?;
    } else {
        // No matches found, mark as completed
        sqlx::query!(
            r#"
            UPDATE collateral_checks
            SET check_status = 'completed'
            WHERE id = $1
            "#,
            check_id
        )
        .execute(pool)
        .await
        .map_err(|e| DbError::TransactionFailed(format!("Failed to update check record: {}", e)))?;
    }

    Ok(())
}

/// Find potential matching collateral using flexible matching strategies
#[instrument(skip(pool))]
async fn find_potential_matches(
    pool: &PgPool,
    collateral: &Collateral,
) -> Result<Vec<Uuid>, DbError> {
    let mut matches = Vec::new();

    // Strategy 1: Exact name match (excluding self)
    let exact_name_matches = sqlx::query!(
        r#"
        SELECT id
        FROM collateral
        WHERE LOWER(name) = LOWER($1) AND id != $2
        LIMIT 10
        "#,
        collateral.name,
        collateral.id
    )
    .fetch_all(pool)
    .await
    .map_err(|e| DbError::TransactionFailed(format!("Failed to find exact name matches: {}", e)))?;

    matches.extend(exact_name_matches.iter().map(|r| r.id));

    // Strategy 2: Custom ID match (if present)
    if let Some(custom_id) = &collateral.custom_id {
        let custom_id_matches = sqlx::query!(
            r#"
            SELECT id
            FROM collateral
            WHERE custom_id = $1 AND id != $2
            LIMIT 10
            "#,
            custom_id,
            collateral.id
        )
        .fetch_all(pool)
        .await
        .map_err(|e| {
            DbError::TransactionFailed(format!("Failed to find custom_id matches: {}", e))
        })?;

        matches.extend(custom_id_matches.iter().map(|r| r.id));
    }

    // Strategy 3: Address match (if present)
    if let Some(address) = &collateral.address {
        let address_matches = sqlx::query!(
            r#"
            SELECT id
            FROM collateral
            WHERE LOWER(address) = LOWER($1) AND id != $2
            LIMIT 10
            "#,
            address,
            collateral.id
        )
        .fetch_all(pool)
        .await
        .map_err(|e| {
            DbError::TransactionFailed(format!("Failed to find address matches: {}", e))
        })?;

        matches.extend(address_matches.iter().map(|r| r.id));
    }

    // Strategy 4: Fuzzy name matching using trigram similarity (requires pg_trgm extension)
    // Note: This requires the pg_trgm extension to be installed
    // For now, we'll use a simpler ILIKE-based approach
    let fuzzy_matches = sqlx::query!(
        r#"
        SELECT id
        FROM collateral
        WHERE
            id != $1 AND
            (name ILIKE '%' || $2 || '%' OR $2 ILIKE '%' || name || '%')
        LIMIT 10
        "#,
        collateral.id,
        collateral.name
    )
    .fetch_all(pool)
    .await
    .map_err(|e| DbError::TransactionFailed(format!("Failed to find fuzzy matches: {}", e)))?;

    matches.extend(fuzzy_matches.iter().map(|r| r.id));

    // Remove duplicates and return
    matches.sort();
    matches.dedup();

    Ok(matches)
}

/// Get collateral with its potential duplicates
#[instrument(skip(pool))]
pub async fn get_collateral_with_duplicates(
    pool: &PgPool,
    id: Uuid,
) -> Result<CollateralWithDuplicates, DbError> {
    let collateral = get_collateral_by_id(pool, id).await?;

    let potential_duplicates = if collateral.double_pledged.is_some() {
        // Get the latest check record
        let check = sqlx::query_as!(
            CollateralCheck,
            r#"
            SELECT
                id, collateral_id, checked_at as "checked_at!",
                potential_matches as "potential_matches?: Vec<Uuid>",
                match_score, check_status as "check_status!", metadata
            FROM collateral_checks
            WHERE collateral_id = $1
            ORDER BY checked_at DESC
            LIMIT 1
            "#,
            id
        )
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            DbError::TransactionFailed(format!("Failed to get collateral check: {}", e))
        })?;

        if let Some(check) = check {
            if let Some(match_ids) = check.potential_matches {
                let mut duplicates = Vec::new();
                for match_id in match_ids {
                    if let Ok(dup) = get_collateral_by_id(pool, match_id).await {
                        duplicates.push(dup);
                    }
                }
                duplicates
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    Ok(CollateralWithDuplicates {
        collateral,
        potential_duplicates,
    })
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

/// Get all matches for a collateral with scores
#[instrument(skip(pool))]
pub async fn get_collateral_matches(
    pool: &PgPool,
    collateral_id: Uuid,
) -> Result<Vec<CollateralMatchDetail>, DbError> {
    let rows = sqlx::query!(
        r#"
        SELECT
            cm.matched_collateral_id,
            cm.match_score as "match_score!",
            cm.match_type as "match_type!",
            c.id, c.name, c.collateral_type, c.description, c.address,
            c.custom_id, c.double_pledged, c.has_potential_duplicates,
            c.created_at as "created_at!", c.updated_at as "updated_at!", c.created_by, c.metadata
        FROM collateral_matches cm
        INNER JOIN collateral c ON c.id = cm.matched_collateral_id
        WHERE cm.collateral_id = $1
        ORDER BY cm.match_score DESC
        "#,
        collateral_id
    )
    .fetch_all(pool)
    .await
    .map_err(DbError::Database)?;

    let matches = rows
        .into_iter()
        .map(|row| CollateralMatchDetail {
            matched_collateral: Collateral {
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
            },
            match_score: row.match_score,
            match_type: row.match_type,
        })
        .collect();

    Ok(matches)
}

/// Get the configurable match threshold
#[instrument(skip(pool))]
pub async fn get_match_threshold(pool: &PgPool) -> Result<f64, DbError> {
    let threshold = sqlx::query_scalar!(
        "SELECT match_threshold FROM collateral_check_config ORDER BY id DESC LIMIT 1"
    )
    .fetch_one(pool)
    .await
    .map_err(|e| DbError::TransactionFailed(format!("Failed to get match threshold: {}", e)))?;

    Ok(threshold
        .map(|d| d.to_string().parse().unwrap_or(50.0))
        .unwrap_or(50.0))
}

/// Process duplicate check for a collateral (transaction-wrapped)
/// This is called by the background service
#[instrument(skip(pool))]
pub async fn process_duplicate_check(pool: &PgPool, collateral_id: Uuid) -> Result<(), DbError> {
    // Start transaction
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| DbError::TransactionFailed(format!("Failed to start transaction: {}", e)))?;

    // Get collateral
    let collateral = get_collateral_by_id(pool, collateral_id).await?;

    // Get configurable threshold
    let threshold_decimal = sqlx::query_scalar!(
        "SELECT match_threshold FROM collateral_check_config ORDER BY id DESC LIMIT 1"
    )
    .fetch_one(&mut *tx)
    .await
    .unwrap_or(Some(rust_decimal::Decimal::new(50, 0)))
    .unwrap_or(rust_decimal::Decimal::new(50, 0));

    let threshold = threshold_decimal.to_string().parse::<f64>().unwrap_or(50.0);

    // Find matches with scores
    let matches = find_potential_matches_with_scores(pool, &collateral).await?;

    if !matches.is_empty() {
        // Filter by threshold
        let filtered_matches: Vec<_> = matches
            .into_iter()
            .filter(|(_, score, _)| *score >= threshold)
            .collect();

        if !filtered_matches.is_empty() {
            // Insert all matches into collateral_matches table
            for (matched_id, score, match_type) in &filtered_matches {
                let score_decimal = rust_decimal::Decimal::from_f64_retain(*score)
                    .unwrap_or(rust_decimal::Decimal::new(0, 0));

                sqlx::query!(
                    r#"
                    INSERT INTO collateral_matches (collateral_id, matched_collateral_id, match_score, match_type)
                    VALUES ($1, $2, $3, $4)
                    ON CONFLICT (collateral_id, matched_collateral_id) DO UPDATE
                    SET match_score = EXCLUDED.match_score, match_type = EXCLUDED.match_type
                    "#,
                    collateral_id,
                    matched_id,
                    score_decimal,
                    match_type
                )
                .execute(&mut *tx)
                .await
                .map_err(|e| DbError::TransactionFailed(format!("Failed to insert match: {}", e)))?;
            }

            // Update collateral: set highest match in double_pledged (backward compat)
            let highest_match = filtered_matches
                .iter()
                .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
                .unwrap();

            sqlx::query!(
                r#"
                UPDATE collateral
                SET double_pledged = $1, has_potential_duplicates = TRUE
                WHERE id = $2
                "#,
                highest_match.0,
                collateral_id
            )
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                DbError::TransactionFailed(format!("Failed to update collateral duplicates: {}", e))
            })?;
        }
    }

    // Commit transaction
    tx.commit()
        .await
        .map_err(|e| DbError::TransactionFailed(format!("Failed to commit transaction: {}", e)))?;

    Ok(())
}

/// Find potential matching collateral with scores
#[instrument(skip(pool))]
async fn find_potential_matches_with_scores(
    pool: &PgPool,
    collateral: &Collateral,
) -> Result<Vec<(Uuid, f64, String)>, DbError> {
    use std::collections::HashMap;

    let mut matches: Vec<(Uuid, f64, String)> = Vec::new();

    // Strategy 1: Exact name match (100 points)
    let exact_matches = sqlx::query!(
        r#"
        SELECT id
        FROM collateral
        WHERE LOWER(name) = LOWER($1) AND id != $2
        LIMIT 10
        "#,
        collateral.name,
        collateral.id
    )
    .fetch_all(pool)
    .await
    .map_err(|e| DbError::TransactionFailed(format!("Failed to find exact name matches: {}", e)))?;

    matches.extend(
        exact_matches
            .iter()
            .map(|r| (r.id, 100.0, "exact_name".to_string())),
    );

    // Strategy 2: Custom ID match (100 points)
    if let Some(custom_id) = &collateral.custom_id {
        let custom_matches = sqlx::query!(
            r#"
            SELECT id
            FROM collateral
            WHERE custom_id = $1 AND id != $2
            LIMIT 10
            "#,
            custom_id,
            collateral.id
        )
        .fetch_all(pool)
        .await
        .map_err(|e| {
            DbError::TransactionFailed(format!("Failed to find custom_id matches: {}", e))
        })?;

        matches.extend(
            custom_matches
                .iter()
                .map(|r| (r.id, 100.0, "custom_id".to_string())),
        );
    }

    // Strategy 3: Address match (95 points)
    if let Some(address) = &collateral.address {
        let address_matches = sqlx::query!(
            r#"
            SELECT id
            FROM collateral
            WHERE LOWER(address) = LOWER($1) AND id != $2
            LIMIT 10
            "#,
            address,
            collateral.id
        )
        .fetch_all(pool)
        .await
        .map_err(|e| {
            DbError::TransactionFailed(format!("Failed to find address matches: {}", e))
        })?;

        matches.extend(
            address_matches
                .iter()
                .map(|r| (r.id, 95.0, "address".to_string())),
        );
    }

    // Strategy 4: Fuzzy name matching (60-90 points based on substring)
    // Simple substring matching - can be enhanced with Levenshtein distance
    let fuzzy_matches = sqlx::query!(
        r#"
        SELECT id, name
        FROM collateral
        WHERE
            id != $1 AND
            (name ILIKE '%' || $2 || '%' OR $2 ILIKE '%' || name || '%')
        LIMIT 10
        "#,
        collateral.id,
        collateral.name
    )
    .fetch_all(pool)
    .await
    .map_err(|e| DbError::TransactionFailed(format!("Failed to find fuzzy matches: {}", e)))?;

    for row in fuzzy_matches {
        // Calculate simple similarity score based on length ratio
        let match_name = &row.name;
        let len1 = collateral.name.len() as f64;
        let len2 = match_name.len() as f64;
        let len_ratio = len1.min(len2) / len1.max(len2);

        // Score between 60-90 based on similarity
        let score = 60.0 + (len_ratio * 30.0);

        if score >= 60.0 {
            matches.push((row.id, score, "fuzzy_name".to_string()));
        }
    }

    // Dedupe: keep highest score for each match
    let mut best_matches: HashMap<Uuid, (f64, String)> = HashMap::new();
    for (id, score, match_type) in matches {
        best_matches
            .entry(id)
            .and_modify(|e| {
                if score > e.0 {
                    *e = (score, match_type.clone());
                }
            })
            .or_insert((score, match_type));
    }

    Ok(best_matches
        .into_iter()
        .map(|(id, (score, mt))| (id, score, mt))
        .collect())
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
