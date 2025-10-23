/// Delete embeddings for a document while keeping the document metadata
use api::services::document_ingestion::surrealdb_service::SurrealDBService;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    // Get document ID from command line args
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: cargo run --bin delete_embeddings_only <document_id>");
        eprintln!(
            "Example: cargo run --bin delete_embeddings_only a60299b6-7240-4425-b29e-80ba6a0e3c3b"
        );
        std::process::exit(1);
    }

    let document_id_str = &args[1];
    let document_id =
        Uuid::parse_str(document_id_str).map_err(|e| format!("Invalid UUID: {}", e))?;

    println!("🔄 Connecting to SurrealDB...");
    let db = SurrealDBService::from_env().await?;

    println!("🧹 Deleting embeddings for document {}...", document_id);

    // Delete embeddings using raw query
    let delete_query = format!(
        "DELETE FROM document_embeddings WHERE document_id = '{}'",
        document_id
    );

    let result = db.execute_query(&delete_query).await;

    match result {
        Ok(_) => {
            println!("✅ Embeddings deleted successfully!");
            println!("💡 Document metadata preserved - ready for re-processing");
        }
        Err(e) => {
            eprintln!("❌ Failed to delete embeddings: {}", e);
            std::process::exit(1);
        }
    }

    Ok(())
}
