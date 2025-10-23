/// Test script to debug vector search queries
use api::services::document_ingestion::surrealdb_service::SurrealDBService;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    println!("🔄 Connecting to SurrealDB...");
    let db_service = SurrealDBService::from_env().await?;

    // Check if embeddings exist
    println!("\n📊 Checking for embeddings...");
    let mut result = db_service
        .execute_query("SELECT * FROM document_embeddings LIMIT 5")
        .await?;

    use surrealdb::sql::Value;
    let embeddings: Vec<Value> = result.take(0)?;
    println!("Found {} embeddings", embeddings.len());

    if let Some(first) = embeddings.first() {
        println!("First embedding sample: {:#?}", first);
    }

    // Get a sample embedding
    println!("\n📋 Fetching sample embedding...");
    let mut result = db_service
        .execute_query("SELECT meta::id(id) as id, document_id, company_id, deal_id, chunk_index FROM document_embeddings LIMIT 1")
        .await?;

    let sample: Option<Value> = result.take(0)?;
    println!("Sample embedding: {:#?}", sample);

    // Test the problematic query with string conversion
    println!("\n🔍 Testing search query with string() conversion...");
    let test_vector = vec![0.0f32; 4096]; // Dummy vector

    let query = r#"
        SELECT
         string::split(meta::id(id), ':')[1] AS embedding_id,
         string(document_id) AS document_id,
         string(company_id) AS company_id,
         string(deal_id) AS deal_id,
         chunk_index
         FROM document_embeddings
         LIMIT 1
    "#;

    let mut result = db_service.execute_query(query).await;

    match result {
        Ok(ref mut res) => {
            let data: Option<Value> = res.take(0)?;
            println!("✅ Query succeeded: {:#?}", data);
        }
        Err(e) => {
            println!("❌ Query failed: {:#}", e);
        }
    }

    // Check for null chunk_text
    println!("\n🔍 Checking for null chunk_text values...");
    let query = r#"
        SELECT count() FROM document_embeddings WHERE chunk_text = NONE
    "#;

    let mut result = db_service.execute_query(query).await;

    match result {
        Ok(ref mut res) => {
            let count: Option<Value> = res.take(0)?;
            println!("❌ Found null chunk_text records: {:#?}", count);
        }
        Err(e) => {
            println!("❌ Query failed: {:#}", e);
        }
    }

    println!("\n✅ Test completed - fixed RawSearchResult to handle null chunk_text values");

    Ok(())
}
