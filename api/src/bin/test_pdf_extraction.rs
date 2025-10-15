// Test PDF Extraction Service
use anyhow::Result;
use api::services::document_ingestion::{PdfExtractor, TextChunker};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("🔍 Testing PDF Extraction Service\n");
    println!("{}", "=".repeat(80));

    // Test PDF extraction
    let extractor = PdfExtractor::new();
    let pdf_path = "test_data/sample.pdf";

    println!("\n📄 Extracting PDF: {}", pdf_path);

    match extractor.extract_from_file(pdf_path) {
        Ok(result) => {
            println!("✅ PDF Extraction Successful!");
            println!("\n📊 Metadata:");
            println!("  - Title: {:?}", result.metadata.title);
            println!("  - Author: {:?}", result.metadata.author);
            println!("  - Pages: {}", result.metadata.page_count);
            println!("  - PDF Version: {}", result.metadata.pdf_version);
            println!("  - File Size: {} bytes", result.metadata.file_size_bytes);
            println!("  - Encrypted: {}", result.metadata.is_encrypted);

            println!("\n📝 Text Content:");
            println!("  - Total characters: {}", result.text.len());
            println!("  - Pages extracted: {}", result.text_by_page.len());

            if !result.extraction_errors.is_empty() {
                println!("\n⚠️  Extraction Errors:");
                for error in &result.extraction_errors {
                    println!("  - {}", error);
                }
            }

            // Show preview
            println!("\n📖 Text Preview (first 500 chars):");
            println!("{}", "-".repeat(80));
            let preview = if result.text.len() > 500 {
                &result.text[..500]
            } else {
                &result.text
            };
            println!("{}", preview);
            println!("{}", "-".repeat(80));

            // Test text chunking
            println!("\n✂️  Testing Text Chunking");
            let chunker = TextChunker::default_semantic();
            let chunks = chunker.chunk(&result.text)?;

            println!("✅ Chunking Successful!");
            println!("  - Total chunks: {}", chunks.len());

            let stats = chunker.get_stats(&chunks);
            println!("\n📊 Chunking Statistics:");
            println!("  - Total chunks: {}", stats.total_chunks);
            println!("  - Total characters: {}", stats.total_chars);
            println!("  - Average chunk size: {}", stats.avg_chunk_size);
            println!("  - Min chunk size: {}", stats.min_chunk_size);
            println!("  - Max chunk size: {}", stats.max_chunk_size);

            // Show first 3 chunks
            println!("\n📦 First 3 Chunks:");
            for chunk in chunks.iter().take(3) {
                println!(
                    "\n  Chunk #{} ({} chars):",
                    chunk.chunk_index, chunk.char_count
                );
                let chunk_preview = if chunk.text.len() > 200 {
                    format!("{}...", &chunk.text[..200])
                } else {
                    chunk.text.clone()
                };
                println!("  {}", chunk_preview);
            }

            println!("\n{}", "=".repeat(80));
            println!("✅ All tests passed!");
        }
        Err(e) => {
            println!("❌ PDF Extraction Failed: {}", e);
            println!("\nPossible causes:");
            println!("  - PDF file not found at '{}'", pdf_path);
            println!("  - PDF is corrupted or invalid");
            println!("  - PDF is encrypted with password");
            println!("\nTroubleshooting:");
            println!("  1. Check if file exists: ls -la {}", pdf_path);
            println!("  2. Try a different PDF file");
            println!("  3. Check PDF can be opened normally");
            return Err(e);
        }
    }

    Ok(())
}
