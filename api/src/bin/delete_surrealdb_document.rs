use anyhow::Result;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<()> {
    // Install default crypto provider for rustls
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .ok();

    // Initialize tracing for better logging
    tracing_subscriber::fmt::init();

    println!("🌐 Connecting to SurrealDB...\n");

    // Get document ID from command line args
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: cargo run --bin delete_surrealdb_document <document-id>");
        eprintln!("Example: cargo run --bin delete_surrealdb_document ff75b1c9-3223-4643-a9b7-f6e0ae74c9dd");
        std::process::exit(1);
    }

    let document_id = Uuid::parse_str(&args[1])?;
    println!("🗑️  Deleting document: {}\n", document_id);

    // Connect to SurrealDB using environment variables
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

    println!("✅ Connected to SurrealDB!\n");

    // First, check if the document exists
    let check_query = "SELECT * FROM documents WHERE document_id = $document_id";
    let mut result = db
        .query(check_query)
        .bind(("document_id", document_id.to_string()))
        .await?;

    #[derive(serde::Deserialize, Debug)]
    struct DocumentCheck {
        document_id: String,
        filename: String,
        status: String,
    }

    let docs: Vec<DocumentCheck> = result.take(0)?;

    if docs.is_empty() {
        println!("❌ Document {} not found in SurrealDB", document_id);
        return Ok(());
    }

    println!("📄 Found document:");
    for doc in &docs {
        println!("  ID: {}", doc.document_id);
        println!("  Filename: {}", doc.filename);
        println!("  Status: {}\n", doc.status);
    }

    // Delete the document
    let delete_query = "DELETE FROM documents WHERE document_id = $document_id";
    db.query(delete_query)
        .bind(("document_id", document_id.to_string()))
        .await?;

    println!("✅ Document {} deleted successfully!", document_id);

    // Also delete any associated embeddings
    let delete_embeddings_query =
        "DELETE FROM document_embeddings WHERE document_id = $document_id";
    let _emb_result = db
        .query(delete_embeddings_query)
        .bind(("document_id", document_id.to_string()))
        .await?;

    // Try to count deleted embeddings
    println!("🧹 Cleaned up associated embeddings");

    println!("\n{}", "=".repeat(80));
    println!("✅ Cleanup completed successfully!");
    println!("{}", "=".repeat(80));

    Ok(())
}
