/// Reset document status in SurrealDB from "failed" back to "uploaded"
/// This allows failed documents to be reprocessed after fixing the issue
use api::services::document_ingestion::surrealdb_service::SurrealDBService;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    // Get document ID from command line args
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: cargo run --bin reset_document_status <document_id>");
        eprintln!(
            "Example: cargo run --bin reset_document_status 8e1496c0-6d03-47a9-8961-d0840d70dab0"
        );
        std::process::exit(1);
    }

    let document_id_str = &args[1];
    let document_id =
        uuid::Uuid::parse_str(document_id_str).map_err(|e| format!("Invalid UUID: {}", e))?;

    println!("🔄 Connecting to SurrealDB...");
    let db = SurrealDBService::from_env().await?;

    println!("📄 Fetching document {}...", document_id);
    let document = db.get_document(document_id).await?;

    match document {
        Some(doc) => {
            println!(
                "✅ Found document: {} (status: {})",
                doc.filename, doc.status
            );

            if doc.status == "uploaded" || doc.status == "queued" {
                println!(
                    "ℹ️  Document is already in processable status: {}",
                    doc.status
                );
                println!("💡 No changes needed");
                return Ok(());
            }

            println!(
                "🔄 Resetting document status from '{}' to 'uploaded'...",
                doc.status
            );
            db.update_document_status(document_id, "uploaded", Some(0.0), None)
                .await?;

            println!("✅ Document status reset successfully!");
            println!("💡 The job queue worker will pick it up on the next poll cycle");
        }
        None => {
            println!("❌ Document not found in SurrealDB!");
            println!("💡 Make sure the document exists and the UUID is correct");
            std::process::exit(1);
        }
    }

    Ok(())
}
