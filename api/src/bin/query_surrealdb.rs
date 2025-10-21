use anyhow::Result;
use api::services::document_ingestion::surrealdb_service::{Document, SurrealDBService};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing for better logging
    tracing_subscriber::fmt::init();

    println!("🌐 Connecting to SurrealDB...\n");

    // Connect to SurrealDB using environment variables
    let surreal_service = SurrealDBService::from_env().await?;

    println!("✅ Connected to SurrealDB!\n");

    // Perform health check
    let is_healthy = surreal_service.health_check().await?;
    if !is_healthy {
        println!("⚠️  Warning: Health check failed, but continuing...\n");
    }

    // Query all documents
    println!("{}", "=".repeat(80));
    println!("📄 ALL DOCUMENTS");
    println!("{}", "=".repeat(80));

    // Use a raw query via SurrealDB SDK directly
    use surrealdb::engine::any;
    use surrealdb::opt::auth::Root;

    let db = any::connect(
        std::env::var("SURREALDB_URL").unwrap_or_else(|_| "wss://cloud.surrealdb.com".to_string()),
    )
    .await?;

    db.use_ns(std::env::var("SURREALDB_NAMESPACE").unwrap_or_else(|_| "arda".to_string()))
        .use_db(std::env::var("SURREALDB_DATABASE").unwrap_or_else(|_| "credit".to_string()))
        .await?;

    db.signin(Root {
        username: &std::env::var("SURREALDB_USERNAME").unwrap_or_else(|_| "root".to_string()),
        password: &std::env::var("SURREALDB_PASSWORD")
            .expect("SURREALDB_PASSWORD environment variable is required"),
    })
    .await?;

    let query = "SELECT * FROM documents ORDER BY upload_timestamp DESC";
    let mut result = db.query(query).await?;
    let documents: Vec<Document> = result.take(0)?;

    if documents.is_empty() {
        println!("❌ No documents found in the database.\n");
    } else {
        println!("Found {} document(s):\n", documents.len());

        for (i, doc) in documents.iter().enumerate() {
            println!("Document #{}", i + 1);
            println!("  ID: {}", doc.document_id);
            println!("  Filename: {}", doc.filename);
            println!("  Type: {}", doc.document_type);
            println!("  Status: {}", doc.status);
            println!("  File Size: {} bytes", doc.file_size);
            println!("  Pages: {:?}", doc.page_count);
            println!("  Company ID: {}", doc.company_id);
            if let Some(deal_id) = doc.deal_id {
                println!("  Deal ID: {}", deal_id);
            }
            if let Some(user_id) = doc.user_id {
                println!("  User ID: {}", user_id);
            }
            println!("  Uploaded: {}", doc.upload_timestamp);
            println!("  Progress: {:?}", doc.processing_progress);
            println!("  Retry Count: {}", doc.retry_count);
            if let Some(started) = doc.processing_started_at {
                println!("  Processing Started: {}", started);
            }
            if let Some(completed) = doc.processing_completed_at {
                println!("  Processing Completed: {}", completed);
            }
            if let Some(error) = &doc.error_message {
                println!("  ❌ Error: {}", error);
            }
            println!("  Content Preview:");
            let preview = if doc.content.len() > 200 {
                format!("{}...", &doc.content[..200])
            } else {
                doc.content.clone()
            };
            println!("    {}\n", preview);
            println!("{}", "-".repeat(80));
        }
    }

    // Query documents by status
    println!("\n{}", "=".repeat(80));
    println!("📊 DOCUMENTS BY STATUS");
    println!("{}", "=".repeat(80));

    for status in &["uploaded", "queued", "processing", "completed", "failed"] {
        let docs = surreal_service.get_documents_by_status(status, 100).await?;
        println!("  {} {}: {}", status_emoji(status), status, docs.len());
    }

    // Query embeddings count
    println!("\n{}", "=".repeat(80));
    println!("📐 EMBEDDING STATISTICS");
    println!("{}", "=".repeat(80));

    let mut result = db
        .query("SELECT count() FROM document_embeddings GROUP ALL")
        .await?;

    #[derive(serde::Deserialize)]
    struct CountResult {
        count: i64,
    }

    let counts: Vec<CountResult> = result.take(0).unwrap_or_default();
    let total_embeddings = counts.first().map(|c| c.count).unwrap_or(0);

    println!("  Total Embeddings: {}", total_embeddings);

    if !documents.is_empty() {
        println!("\n  Embeddings per document:");
        for doc in documents.iter() {
            let embeddings = surreal_service
                .get_embeddings_by_document(doc.document_id)
                .await?;
            println!("    {}: {} chunks", doc.filename, embeddings.len());
        }
    }

    println!("\n{}", "=".repeat(80));
    println!("✅ Query completed successfully!");
    println!("{}", "=".repeat(80));

    Ok(())
}

fn status_emoji(status: &str) -> &str {
    match status {
        "uploaded" => "📤",
        "queued" => "⏳",
        "processing" => "⚙️",
        "completed" => "✅",
        "failed" => "❌",
        _ => "❓",
    }
}
