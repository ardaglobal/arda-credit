// Test PDF → LLM Extraction Integration
use anyhow::Result;
use api::services::document_ingestion::{LlmExtractionService, PdfExtractor, TextChunker};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    println!("🔗 Testing PDF → LLM Extraction Integration\n");
    println!("{}", "=".repeat(80));

    // Step 1: Extract PDF
    println!("\n📄 Step 1: Extracting PDF...");
    let extractor = PdfExtractor::new();
    let result = extractor.extract_from_file("test_data/sample.pdf")?;
    println!(
        "✅ Extracted {} characters from {} pages",
        result.text.len(),
        result.metadata.page_count
    );

    // Step 2: Chunk text
    println!("\n✂️  Step 2: Chunking text...");
    let chunker = TextChunker::default_semantic();
    let chunks = chunker.chunk(&result.text)?;
    println!("✅ Created {} chunks", chunks.len());

    // Step 3: Extract structured data via native Rust LLM service
    println!("\n🤖 Step 3: Extracting structured fields via LLM...");
    let llm_service = LlmExtractionService::from_env()?;

    // Check service health
    match llm_service.health_check().await {
        Ok(true) => println!("✅ LLM extraction service is ready"),
        Ok(false) => {
            println!("❌ LLM extraction service is unhealthy");
            return Ok(());
        }
        Err(e) => {
            println!("❌ Cannot initialize LLM service: {}", e);
            println!("\nMake sure you have set one of these environment variables:");
            println!("  OPENAI_API_KEY");
            println!("  ANTHROPIC_API_KEY");
            println!("  OPENROUTER_API_KEY");
            return Ok(());
        }
    }

    // Extract deal data
    println!("\n📊 Extracting deal data...");
    match llm_service.extract_deal(&result.text).await {
        Ok(extracted) => {
            println!("✅ Extraction successful!");
            println!("\n💰 Financial Terms:");
            println!(
                "  - Principal: ${:.2}",
                extracted.financial_terms.principal_amount
            );
            println!("  - Currency: {:?}", extracted.financial_terms.currency);
            println!(
                "  - Interest Rate: {:.2}%",
                extracted.financial_terms.interest_rate * 100.0
            );
            println!(
                "  - Term: {} days",
                extracted.financial_terms.term_length_days
            );

            println!("\n🤝 Parties:");
            println!("  - Originator: {}", extracted.parties.originator_name);
            println!("  - Investor: {}", extracted.parties.investor_name);
            if !extracted.parties.guarantors.is_empty() {
                println!(
                    "  - Guarantors: {}",
                    extracted.parties.guarantors.join(", ")
                );
            }

            println!("\n🔒 Collateral:");
            println!(
                "  - Has Collateral: {}",
                extracted.collateral.has_collateral
            );
            if extracted.collateral.has_collateral {
                println!("  - Types: {:?}", extracted.collateral.collateral_types);
            }

            println!("\n📈 Analysis:");
            println!("  - Deal Type: {:?}", extracted.deal_type);
            println!(
                "  - Confidence Score: {:.1}%",
                extracted.confidence_score * 100.0
            );

            if !extracted.extraction_notes.is_empty() {
                println!("\n📝 Notes:");
                for note in &extracted.extraction_notes {
                    println!("  - {}", note);
                }
            }

            println!("\n{}", "=".repeat(80));
            println!("✅ Integration test successful!");
            println!("\nNext steps:");
            println!("  1. Store this data in PostgreSQL");
            println!("  2. Generate embeddings from chunks");
            println!("  3. Store vectors in Qdrant");
            println!("  4. Build orchestrator to automate this pipeline");
        }
        Err(e) => {
            println!("❌ LLM extraction failed: {}", e);
            println!("\nPossible causes:");
            println!("  - Document format not recognized");
            println!("  - LLM API rate limit or invalid API key");
            println!("  - Network issues");
            println!("  - Insufficient credits on LLM provider");
        }
    }

    Ok(())
}
