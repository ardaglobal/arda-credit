use anyhow::Result;
use surrealdb::engine::any;
use surrealdb::opt::auth::Root;

#[tokio::main]
async fn main() -> Result<()> {
    println!("🚀 Initializing SurrealDB Schema\n");

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
    println!("📝 Executing schema definition...\n");

    // Load the schema file
    let schema = include_str!("../services/document_ingestion/surrealdb_schema.surql");

    // Split by semicolon and execute each statement
    let statements: Vec<&str> = schema
        .split(';')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty() && !s.starts_with("--"))
        .collect();

    println!("Found {} schema statements to execute\n", statements.len());

    for (i, statement) in statements.iter().enumerate() {
        let preview = if statement.len() > 60 {
            format!("{}...", &statement[..60])
        } else {
            statement.to_string()
        };

        print!("[{}/{}] Executing: {} ", i + 1, statements.len(), preview);

        match db.query(*statement).await {
            Ok(_) => {
                println!("✅");
            }
            Err(e) => {
                println!("❌");
                println!("   Error: {}", e);
                println!("   Statement: {}", statement);
                return Err(e.into());
            }
        }
    }

    println!("\n{}", "=".repeat(80));
    println!("✅ Schema initialization completed successfully!\n");

    // Verify tables were created
    println!("🔍 Verifying tables...\n");

    let tables_to_check = vec!["documents", "document_embeddings"];

    for table in tables_to_check {
        match db.query(format!("INFO FOR TABLE {}", table)).await {
            Ok(_) => println!("   ✅ Table '{}' exists", table),
            Err(e) => {
                println!("   ❌ Table '{}' NOT found: {}", table, e);
                return Err(e.into());
            }
        }
    }

    println!("\n{}", "=".repeat(80));
    println!("🎉 All done! Your SurrealDB schema is ready.");
    println!("\nYou can now:");
    println!("  • Run 'make server' to start the API server");
    println!("  • Upload documents via your frontend");
    println!("  • Query documents with 'cargo run --bin query_surrealdb'");

    Ok(())
}
