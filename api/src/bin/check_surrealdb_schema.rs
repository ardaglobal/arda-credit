use anyhow::Result;
use surrealdb::engine::any;
use surrealdb::opt::auth::Root;

#[tokio::main]
async fn main() -> Result<()> {
    println!("🔍 Checking SurrealDB Schema\n");

    // Get connection details from environment
    let url =
        std::env::var("SURREALDB_URL").unwrap_or_else(|_| "wss://cloud.surrealdb.com".to_string());
    let username = std::env::var("SURREALDB_USERNAME").unwrap_or_else(|_| "root".to_string());
    let password = std::env::var("SURREALDB_PASSWORD")
        .expect("SURREALDB_PASSWORD environment variable is required");
    let namespace = std::env::var("SURREALDB_NAMESPACE").unwrap_or_else(|_| "arda".to_string());
    let database = std::env::var("SURREALDB_DATABASE").unwrap_or_else(|_| "credit".to_string());

    println!("📡 Connecting to: {}", url);
    println!("📂 Namespace: {}", namespace);
    println!("💾 Database: {}\n", database);

    // Install crypto provider
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    // Connect
    let db = any::connect(&url).await?;
    db.use_ns(&namespace).use_db(&database).await?;
    db.signin(Root {
        username: &username,
        password: &password,
    })
    .await?;

    println!("✅ Connected successfully!\n");
    println!("{}", "=".repeat(80));

    // Check if documents table exists
    println!("📊 Checking for 'documents' table...");
    let mut result = db.query("INFO FOR TABLE documents").await;
    match result {
        Ok(_) => {
            println!("✅ 'documents' table exists");

            // Get document count
            let mut result = db.query("SELECT count() FROM documents GROUP ALL").await?;
            #[derive(serde::Deserialize)]
            struct CountResult {
                count: i64,
            }
            let counts: Vec<CountResult> = result.take(0).unwrap_or_default();
            let count = counts.first().map(|c| c.count).unwrap_or(0);
            println!("   📄 Total documents: {}", count);
        }
        Err(e) => {
            println!("❌ 'documents' table does NOT exist");
            println!("   Error: {}", e);
        }
    }

    println!();

    // Check if document_embeddings table exists
    println!("📊 Checking for 'document_embeddings' table...");
    let mut result = db.query("INFO FOR TABLE document_embeddings").await;
    match result {
        Ok(_) => {
            println!("✅ 'document_embeddings' table exists");

            // Get embedding count
            let mut result = db
                .query("SELECT count() FROM document_embeddings GROUP ALL")
                .await?;
            #[derive(serde::Deserialize)]
            struct CountResult {
                count: i64,
            }
            let counts: Vec<CountResult> = result.take(0).unwrap_or_default();
            let count = counts.first().map(|c| c.count).unwrap_or(0);
            println!("   📐 Total embeddings: {}", count);
        }
        Err(e) => {
            println!("❌ 'document_embeddings' table does NOT exist");
            println!("   Error: {}", e);
        }
    }

    println!("\n{}", "=".repeat(80));

    // List all tables in database
    println!("\n📋 Listing all tables in database '{}':", database);
    let result = db.query("INFO FOR DB").await;

    match result {
        Ok(_) => {
            println!("   ✅ Database info retrieved");
            println!("\n💡 To see detailed table info, check individual tables above");
        }
        Err(e) => {
            println!("   ⚠️  Could not retrieve database info: {}", e);
        }
    }

    println!("\n{}", "=".repeat(80));
    println!("\n🔧 To manually initialize schema, run:");
    println!("   cargo run --bin init_surrealdb_schema");
    println!("\nOr ensure 'make server' completes successfully (it auto-initializes)");

    Ok(())
}
