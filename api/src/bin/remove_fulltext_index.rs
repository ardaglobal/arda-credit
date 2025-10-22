/// Remove the problematic fulltext search index
/// The 'ascii' analyzer doesn't exist in SurrealDB Cloud
use api::services::document_ingestion::surrealdb_service::SurrealDBService;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("🔄 Connecting to SurrealDB...");
    let db_service = SurrealDBService::from_env().await?;

    println!("🗑️  Removing problematic fulltext index...\n");

    // Remove the fulltext index with the 'ascii' analyzer
    println!("Removing idx_embedding_fulltext...");
    let result = db_service
        .execute_query("REMOVE INDEX idx_embedding_fulltext ON document_embeddings")
        .await;

    match result {
        Ok(_) => println!("   ✅ Removed fulltext index successfully"),
        Err(e) => {
            println!("   ⚠️  Index might not exist: {}", e);
            println!("   (This is OK if it's already removed)");
        }
    }

    println!("\n✅ Index removal completed!");
    println!("\n💡 Note: Vector search will still work perfectly.");
    println!("   The fulltext index was for hybrid keyword+semantic search,");
    println!("   which can be added back later with a compatible analyzer.");

    Ok(())
}
