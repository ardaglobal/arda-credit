/// Apply schema updates to SurrealDB
/// This script updates the document_embeddings table to:
/// 1. Remove the redundant embedding_id field (it's in the record ID)
/// 2. Update the vector index dimension from 1536 to 4096
use api::services::document_ingestion::surrealdb_service::SurrealDBService;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("🔄 Connecting to SurrealDB...");
    let db_service = SurrealDBService::from_env().await?;

    println!("📋 Applying schema updates...\n");

    // Step 0: Add missing storage_path field to documents table
    println!("0️⃣  Adding storage_path field to documents table...");
    let result = db_service
        .execute_query("DEFINE FIELD storage_path ON documents TYPE option<string>")
        .await;
    match result {
        Ok(_) => println!("   ✅ Added storage_path field to documents"),
        Err(e) => println!("   ⚠️  Field might already exist: {}", e),
    }

    // Step 1: Remove the old embedding_id field definition
    println!("1️⃣  Removing redundant embedding_id field definition...");
    let result = db_service
        .execute_query("REMOVE FIELD embedding_id ON document_embeddings")
        .await;
    match result {
        Ok(_) => println!("   ✅ Removed embedding_id field definition"),
        Err(e) => println!("   ⚠️  Field might not exist: {}", e),
    }

    // Step 2: Drop the old vector index (if it exists)
    println!("\n2️⃣  Removing old vector index...");
    let result = db_service
        .execute_query("REMOVE INDEX idx_embedding_vector ON document_embeddings")
        .await;
    match result {
        Ok(_) => println!("   ✅ Removed old vector index"),
        Err(e) => println!("   ⚠️  Index might not exist: {}", e),
    }

    // Step 3: Recreate the vector index with correct dimension (4096 for Qwen3-Embedding-8B)
    println!("\n3️⃣  Creating new vector index with 4096 dimensions...");
    db_service
        .execute_query(
            "DEFINE INDEX idx_embedding_vector ON document_embeddings FIELDS embedding MTREE DIMENSION 4096 DIST COSINE"
        )
        .await?;
    println!("   ✅ Created vector index with 4096 dimensions");

    // Step 4: Verify the schema
    println!("\n4️⃣  Verifying schema...");
    let mut result = db_service
        .execute_query("INFO FOR TABLE document_embeddings")
        .await?;

    // SurrealDB returns results as surrealdb::sql::Value
    use surrealdb::sql::Value;
    let info: Option<Value> = result.take(0)?;

    if let Some(value) = info {
        println!("   ✅ Schema verification:");
        println!("{:#?}", value);
    } else {
        println!("   ⚠️  No schema information returned");
    }

    println!("\n✅ Schema update completed successfully!");
    println!("\n💡 Note: Existing embeddings will continue to work. The embedding_id");
    println!("   is now stored in the record ID (e.g., document_embeddings:⟨uuid⟩)");

    Ok(())
}
