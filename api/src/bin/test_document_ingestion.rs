//! Test document ingestion pipeline
//!
//! This binary demonstrates the full document ingestion and search pipeline:
//! 1. Connect to SurrealDB (cloud or local)
//! 2. Ingest PDFs from local data folder
//! 3. Run semantic search queries
//!
//! Usage:
//! ```bash
//! cargo run --bin test_document_ingestion -- ingest ./data/documents
//! cargo run --bin test_document_ingestion -- search "What are the loan terms?"
//! cargo run --bin test_document_ingestion -- health
//! ```

use anyhow::{Context, Result};
use std::path::PathBuf;
use tracing::{error, info, warn};
use uuid::Uuid;

use api::services::document_ingestion::{
    EmbeddingService, PdfExtractor, SurrealDBService, TextChunker,
};
use api::services::document_ingestion_orchestrator::{
    DocumentIngestionOrchestrator, DocumentIngestionRequest,
};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    // Parse command line arguments
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        print_usage();
        return Ok(());
    }

    let command = &args[1];

    match command.as_str() {
        "ingest" => {
            if args.len() < 3 {
                eprintln!("Usage: {} ingest <directory>", args[0]);
                return Ok(());
            }
            let directory = PathBuf::from(&args[2]);
            ingest_documents(directory).await?;
        }
        "search" => {
            if args.len() < 3 {
                eprintln!("Usage: {} search <query>", args[0]);
                return Ok(());
            }
            let query = args[2..].join(" ");
            search_documents(&query).await?;
        }
        "search-by-company" => {
            if args.len() < 4 {
                eprintln!("Usage: {} search-by-company <company_id> <query>", args[0]);
                return Ok(());
            }
            let company_id = Uuid::parse_str(&args[2]).context("Invalid company_id UUID")?;
            let query = args[3..].join(" ");
            search_by_company(company_id, &query).await?;
        }
        "health" => {
            check_health().await?;
        }
        "info" => {
            show_collection_info().await?;
        }
        _ => {
            eprintln!("Unknown command: {}", command);
            print_usage();
        }
    }

    Ok(())
}

fn print_usage() {
    println!(
        r#"
Document Ingestion Test Tool

USAGE:
    test_document_ingestion <COMMAND> [OPTIONS]

COMMANDS:
    ingest <directory>              Ingest all PDFs from directory
    search <query>                  Search across all documents
    search-by-company <id> <query>  Search within a specific company's documents
    health                          Check service health
    info                            Show collection information

EXAMPLES:
    # Ingest all PDFs from data directory
    cargo run --bin test_document_ingestion -- ingest ./data/documents

    # Search across all documents
    cargo run --bin test_document_ingestion -- search "What are the interest rates?"

    # Search within a specific company
    cargo run --bin test_document_ingestion -- search-by-company 123e4567-e89b-12d3-a456-426614174000 "payment terms"

    # Check if services are healthy
    cargo run --bin test_document_ingestion -- health

    # Show collection statistics
    cargo run --bin test_document_ingestion -- info

ENVIRONMENT VARIABLES:
    SURREALDB_URL           SurrealDB URL (wss://cloud.surrealdb.com or ws://localhost:8000/rpc)
    SURREALDB_USERNAME      SurrealDB username (default: root)
    SURREALDB_PASSWORD      SurrealDB password (required)
    SURREALDB_NAMESPACE     SurrealDB namespace (default: arda)
    SURREALDB_DATABASE      SurrealDB database (default: credit)
    OPENAI_API_KEY          OpenAI API key for embeddings (required)
    EMBEDDING_DIMENSIONS    Embedding dimensions (default: 1536)
"#
    );
}

async fn ingest_documents(directory: PathBuf) -> Result<()> {
    info!("🚀 Starting document ingestion from {:?}", directory);

    // Initialize services
    info!("Initializing services...");
    let pool = sqlx::PgPool::connect(&std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgresql://postgres:password@127.0.0.1:5432/arda_credit_db".to_string()
    }))
    .await
    .context("Failed to connect to database")?;

    let orchestrator = DocumentIngestionOrchestrator::from_env(pool)
        .await
        .context("Failed to initialize document ingestion orchestrator")?;

    // Find all PDFs in the directory
    info!("Scanning for PDF files...");
    let mut pdf_files = Vec::new();
    scan_directory(&directory, &mut pdf_files)?;

    if pdf_files.is_empty() {
        warn!("No PDF files found in {:?}", directory);
        return Ok(());
    }

    info!("Found {} PDF files to ingest", pdf_files.len());

    // Ingest each PDF
    for (i, pdf_path) in pdf_files.iter().enumerate() {
        info!(
            "\n📄 Processing file {}/{}: {:?}",
            i + 1,
            pdf_files.len(),
            pdf_path.file_name().unwrap_or_default()
        );

        // Generate IDs for the document
        let document_id = Uuid::new_v4();
        let company_id = Uuid::new_v4(); // In production, this would come from user context

        let request = DocumentIngestionRequest {
            document_id,
            company_id,
            deal_id: None,
            user_id: None, // In production, this would come from user context
            document_type: "credit_agreement".to_string(),
            file_path: pdf_path.to_string_lossy().to_string(),
        };

        match orchestrator.ingest_document(request).await {
            Ok(result) => {
                info!(
                    "✅ Successfully ingested document: {} chunks, took {}ms",
                    result.chunks_processed, result.processing_time_ms
                );
                info!("   Document ID: {}", document_id);
                info!("   Company ID: {}", company_id);
            }
            Err(e) => {
                error!("❌ Failed to ingest {:?}: {}", pdf_path, e);
            }
        }
    }

    info!("\n🎉 Document ingestion complete!");
    Ok(())
}

async fn search_documents(query: &str) -> Result<()> {
    info!("🔍 Searching for: '{}'", query);

    // Initialize services
    let embedding_service =
        EmbeddingService::from_env().context("Failed to initialize embedding service")?;
    let surrealdb_service = SurrealDBService::from_env()
        .await
        .context("Failed to initialize SurrealDB service")?;

    // Generate embedding for query
    info!("Generating query embedding...");
    let query_embedding = embedding_service
        .embed(query)
        .await
        .context("Failed to generate query embedding")?;

    // Search in SurrealDB
    info!("Searching in SurrealDB...");
    let results = surrealdb_service
        .search(query_embedding, 5)
        .await
        .context("Failed to search in SurrealDB")?;

    // Display results
    info!("\n📊 Found {} results:\n", results.len());

    for (i, result) in results.iter().enumerate() {
        println!("{}. Score: {:.4}", i + 1, result.score);
        println!("   Document ID: {}", result.document_id);
        println!("   Chunk {}", result.chunk_index);
        println!(
            "   Text: {}...",
            result.chunk_text.chars().take(150).collect::<String>()
        );
        println!();
    }

    Ok(())
}

async fn search_by_company(company_id: Uuid, query: &str) -> Result<()> {
    info!("🔍 Searching for: '{}' (company: {})", query, company_id);

    // Initialize services
    let embedding_service =
        EmbeddingService::from_env().context("Failed to initialize embedding service")?;
    let surrealdb_service = SurrealDBService::from_env()
        .await
        .context("Failed to initialize SurrealDB service")?;

    // Generate embedding for query
    info!("Generating query embedding...");
    let query_embedding = embedding_service
        .embed(query)
        .await
        .context("Failed to generate query embedding")?;

    // Search in SurrealDB with company filter
    info!("Searching in SurrealDB for company {}...", company_id);
    let results = surrealdb_service
        .search_by_company(Some(company_id), query_embedding, 5)
        .await
        .context("Failed to search in SurrealDB")?;

    // Display results
    info!(
        "\n📊 Found {} results for company {}:\n",
        results.len(),
        company_id
    );

    for (i, result) in results.iter().enumerate() {
        println!("{}. Score: {:.4}", i + 1, result.score);
        println!("   Document ID: {}", result.document_id);
        println!("   Chunk {}", result.chunk_index);
        println!(
            "   Text: {}...",
            result.chunk_text.chars().take(150).collect::<String>()
        );
        println!();
    }

    Ok(())
}

async fn check_health() -> Result<()> {
    info!("🏥 Checking service health...\n");

    // Check SurrealDB
    info!("Checking SurrealDB connection...");
    let surrealdb_service = SurrealDBService::from_env().await?;
    let surrealdb_healthy = surrealdb_service.health_check().await?;
    if surrealdb_healthy {
        info!("✅ SurrealDB: HEALTHY");
    } else {
        error!("❌ SurrealDB: UNHEALTHY");
    }

    // Check Embedding service
    info!("Checking OpenAI embedding service...");
    let embedding_service = EmbeddingService::from_env()?;
    let embedding_healthy = embedding_service.health_check().await?;
    if embedding_healthy {
        info!("✅ Embedding service: HEALTHY");
    } else {
        error!("❌ Embedding service: UNHEALTHY");
    }

    // Check PDF extractor
    info!("Checking PDF extractor...");
    let _pdf_extractor = PdfExtractor::new();
    info!("✅ PDF extractor: READY");

    // Check text chunker
    info!("Checking text chunker...");
    let _text_chunker = TextChunker::new(
        api::services::document_ingestion::ChunkingStrategy::Semantic {
            target_size: 512,
            overlap: 50,
        },
    );
    info!("✅ Text chunker: READY");

    info!("\n🎉 All services are healthy!");
    Ok(())
}

async fn show_collection_info() -> Result<()> {
    info!("📊 Fetching SurrealDB information...\n");

    let surrealdb_service = SurrealDBService::from_env().await?;

    // Test basic connection
    if surrealdb_service.health_check().await? {
        println!("SurrealDB Information:");
        println!("======================");
        println!("Status: Connected");
        println!("Database: Ready for document ingestion");
    } else {
        println!("Status: Disconnected or unhealthy");
    }

    Ok(())
}

/// Recursively scan directory for PDF files
fn scan_directory(path: &PathBuf, pdf_files: &mut Vec<PathBuf>) -> Result<()> {
    if !path.exists() {
        anyhow::bail!("Directory does not exist: {:?}", path);
    }

    if path.is_file() {
        if path.extension().and_then(|s| s.to_str()) == Some("pdf") {
            pdf_files.push(path.clone());
        }
        return Ok(());
    }

    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let entry_path = entry.path();

        if entry_path.is_dir() {
            scan_directory(&entry_path, pdf_files)?;
        } else if entry_path.extension().and_then(|s| s.to_str()) == Some("pdf") {
            pdf_files.push(entry_path);
        }
    }

    Ok(())
}
