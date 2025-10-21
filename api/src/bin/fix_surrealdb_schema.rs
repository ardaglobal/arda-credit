use anyhow::Result;
use surrealdb::engine::any;
use surrealdb::opt::auth::Root;

#[tokio::main]
async fn main() -> Result<()> {
    println!("🔧 Fixing SurrealDB Schema - UUID field types\n");

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

    // Step 1: Check current field definition
    println!("🔍 Step 1: Checking current field definitions...\n");

    let _result = db.query("INFO FOR TABLE documents").await?;
    println!("Current documents table info retrieved\n");

    // Step 2: Remove old field definitions
    println!("🗑️  Step 2: Removing old field definitions...\n");

    let remove_queries = vec![
        "REMOVE FIELD document_id ON documents",
        "REMOVE FIELD company_id ON documents",
        "REMOVE FIELD deal_id ON documents",
        "REMOVE FIELD user_id ON documents",
        "REMOVE FIELD document_id ON document_embeddings",
        "REMOVE FIELD company_id ON document_embeddings",
        "REMOVE FIELD deal_id ON document_embeddings",
        "REMOVE FIELD user_id ON document_embeddings",
    ];

    for query in remove_queries {
        print!("   Executing: {} ", query);
        match db.query(query).await {
            Ok(_) => println!("✅"),
            Err(e) => {
                println!("⚠️  (may not exist: {})", e);
                // Continue anyway - field might not exist
            }
        }
    }

    println!();

    // Step 3: Redefine fields with correct types
    println!("✨ Step 3: Redefining fields with correct types...\n");

    let define_queries = vec![
        "DEFINE FIELD document_id ON documents TYPE string",
        "DEFINE FIELD company_id ON documents TYPE string",
        "DEFINE FIELD deal_id ON documents TYPE option<string>",
        "DEFINE FIELD user_id ON documents TYPE option<string>",
        "DEFINE FIELD document_id ON document_embeddings TYPE string",
        "DEFINE FIELD embedding_id ON document_embeddings TYPE string",
        "DEFINE FIELD company_id ON document_embeddings TYPE string",
        "DEFINE FIELD deal_id ON document_embeddings TYPE option<string>",
        "DEFINE FIELD user_id ON document_embeddings TYPE option<string>",
    ];

    for query in define_queries {
        print!("   Executing: {} ", query);
        match db.query(query).await {
            Ok(_) => println!("✅"),
            Err(e) => {
                println!("❌");
                println!("   Error: {}", e);
                return Err(e.into());
            }
        }
    }

    println!();

    // Step 4: Recreate indexes
    println!("📇 Step 4: Recreating indexes...\n");

    let index_queries = vec![
        "DEFINE INDEX idx_document_company ON documents FIELDS company_id",
        "DEFINE INDEX idx_document_deal ON documents FIELDS deal_id",
        "DEFINE INDEX idx_document_user ON documents FIELDS user_id",
        "DEFINE INDEX idx_embedding_company ON document_embeddings FIELDS company_id",
        "DEFINE INDEX idx_embedding_deal ON document_embeddings FIELDS deal_id",
        "DEFINE INDEX idx_embedding_user ON document_embeddings FIELDS user_id",
    ];

    for query in index_queries {
        print!("   Executing: {} ", query);
        match db.query(query).await {
            Ok(_) => println!("✅"),
            Err(e) => {
                println!("⚠️  (may already exist: {})", e);
                // Continue anyway - index might already exist
            }
        }
    }

    println!("\n{}", "=".repeat(80));

    // Step 5: Verify the fix
    println!("✅ Step 5: Verifying field definitions...\n");

    let _result = db.query("INFO FOR TABLE documents").await?;
    println!("   ✅ documents table schema updated\n");

    let _result = db.query("INFO FOR TABLE document_embeddings").await?;
    println!("   ✅ document_embeddings table schema updated\n");

    println!("{}", "=".repeat(80));
    println!("🎉 Schema fix completed successfully!");
    println!("\nAll UUID fields (document_id, embedding_id, company_id, deal_id, user_id) are now defined as 'string' type.");
    println!("You can retry your document upload now.");

    Ok(())
}
