/// Test script to debug SurrealDB deserialization
use api::services::document_ingestion::surrealdb_service::SurrealDBService;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    println!("🔄 Connecting to SurrealDB...");
    let db_service = SurrealDBService::from_env().await?;

    println!("🔍 Fetching a single embedding to inspect structure...");

    // Query for one embedding
    let mut result = db_service
        .execute_query("SELECT * FROM document_embeddings LIMIT 1")
        .await?;

    // Get raw value
    use surrealdb::sql::Value;
    let raw_value: Option<Value> = result.take(0)?;

    if let Some(value) = raw_value {
        println!("\n📋 Raw SurrealDB response:");
        println!("{:#?}", value);

        // Try to convert to JSON
        let json = serde_json::to_string_pretty(&value)?;
        println!("\n📋 As JSON:");
        println!("{}", json);
    } else {
        println!("⚠️  No embeddings found in database");
    }

    Ok(())
}
