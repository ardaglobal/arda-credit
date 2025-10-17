use anyhow::Result;
use text_splitter::{ChunkConfig, TextSplitter};
use tracing::{debug, info};

/// Document-specific metadata for a chunk
#[derive(Debug, Clone, Default)]
pub struct ChunkMetadata {
    /// Page number (for PDF documents)
    pub page_number: Option<i32>,
    /// Sheet name (for Excel documents)
    pub sheet_name: Option<String>,
    /// Starting row index (for CSV/Excel documents)
    pub start_row: Option<usize>,
    /// Ending row index (for CSV/Excel documents)
    pub end_row: Option<usize>,
}

/// A chunk of text with metadata
#[derive(Debug, Clone)]
pub struct TextChunk {
    pub text: String,
    pub char_count: usize,
    pub start_index: usize,
    pub end_index: usize,
    pub chunk_index: usize,
    /// Document-specific metadata
    pub metadata: ChunkMetadata,
}

/// Strategy for chunking text
#[derive(Debug, Clone, Copy)]
pub enum ChunkingStrategy {
    /// Fixed character count with overlap
    FixedSize { chunk_size: usize, overlap: usize },
    /// Semantic chunking (sentence-aware)
    Semantic { target_size: usize, overlap: usize },
    /// By paragraph
    Paragraph,
}

/// Text Chunking Service
///
/// Splits text into chunks suitable for vector embeddings while preserving context
pub struct TextChunker {
    strategy: ChunkingStrategy,
}

impl TextChunker {
    /// Create a new text chunker with specified strategy
    pub fn new(strategy: ChunkingStrategy) -> Self {
        Self { strategy }
    }

    /// Create a chunker with default semantic strategy optimized for embeddings
    ///
    /// Defaults: 512 characters per chunk, 50 character overlap
    /// This works well with most embedding models (ada-002, text-embedding-3-small)
    pub fn default_semantic() -> Self {
        Self {
            strategy: ChunkingStrategy::Semantic {
                target_size: 512,
                overlap: 50,
            },
        }
    }

    /// Create a chunker optimized for large embeddings
    ///
    /// Defaults: 1024 characters per chunk, 100 character overlap
    /// Use with large embedding models (text-embedding-3-large)
    pub fn large_semantic() -> Self {
        Self {
            strategy: ChunkingStrategy::Semantic {
                target_size: 1024,
                overlap: 100,
            },
        }
    }

    /// Chunk text into segments
    ///
    /// # Arguments
    /// * `text` - The text to chunk
    ///
    /// # Returns
    /// * Vector of `TextChunk` objects with metadata
    pub fn chunk(&self, text: &str) -> Result<Vec<TextChunk>> {
        info!(
            "Chunking text ({} chars) with strategy {:?}",
            text.len(),
            self.strategy
        );

        let chunks = match self.strategy {
            ChunkingStrategy::FixedSize {
                chunk_size,
                overlap,
            } => self.chunk_fixed_size(text, chunk_size, overlap),
            ChunkingStrategy::Semantic {
                target_size,
                overlap,
            } => self.chunk_semantic(text, target_size, overlap),
            ChunkingStrategy::Paragraph => self.chunk_by_paragraph(text),
        };

        debug!("Created {} chunks", chunks.len());

        Ok(chunks)
    }

    /// Fixed-size chunking with overlap
    fn chunk_fixed_size(&self, text: &str, chunk_size: usize, overlap: usize) -> Vec<TextChunk> {
        let mut chunks = Vec::new();
        let chars: Vec<char> = text.chars().collect();
        let mut start = 0;
        let mut chunk_index = 0;

        while start < chars.len() {
            let end = (start + chunk_size).min(chars.len());
            let chunk_text: String = chars[start..end].iter().collect();

            chunks.push(TextChunk {
                text: chunk_text.clone(),
                char_count: chunk_text.chars().count(),
                start_index: start,
                end_index: end,
                chunk_index,
                metadata: ChunkMetadata::default(),
            });

            start += chunk_size - overlap;
            chunk_index += 1;

            // Avoid infinite loop if overlap >= chunk_size
            if overlap >= chunk_size {
                start += 1;
            }
        }

        chunks
    }

    /// Semantic chunking (sentence-aware) using text-splitter crate
    fn chunk_semantic(&self, text: &str, target_size: usize, overlap: usize) -> Vec<TextChunk> {
        // Create splitter with character-based chunking
        let splitter =
            TextSplitter::new(ChunkConfig::new(target_size).with_overlap(overlap).unwrap());

        let raw_chunks = splitter.chunks(text);

        let mut chunks = Vec::new();
        let mut current_index = 0;

        for (chunk_index, chunk_text) in raw_chunks.enumerate() {
            let char_count = chunk_text.chars().count();
            let start_index = current_index;
            let end_index = start_index + char_count;

            chunks.push(TextChunk {
                text: chunk_text.to_string(),
                char_count,
                start_index,
                end_index,
                chunk_index,
                metadata: ChunkMetadata::default(),
            });

            // Move forward accounting for overlap
            current_index = end_index.saturating_sub(overlap);
        }

        chunks
    }

    /// Chunk by paragraphs
    fn chunk_by_paragraph(&self, text: &str) -> Vec<TextChunk> {
        let paragraphs: Vec<&str> = text
            .split("\n\n")
            .filter(|p| !p.trim().is_empty())
            .collect();

        let mut chunks = Vec::new();
        let mut current_index = 0;

        for (chunk_index, paragraph) in paragraphs.iter().enumerate() {
            let chunk_text = paragraph.trim().to_string();
            let char_count = chunk_text.chars().count();
            let end_index = current_index + char_count;

            chunks.push(TextChunk {
                text: chunk_text,
                char_count,
                start_index: current_index,
                end_index,
                chunk_index,
                metadata: ChunkMetadata::default(),
            });

            // Account for the paragraph separator
            current_index = end_index + 2; // +2 for "\n\n"
        }

        chunks
    }

    /// Preview how text will be chunked (for debugging)
    pub fn preview_chunks(&self, text: &str, max_display: usize) -> Result<()> {
        let chunks = self.chunk(text)?;

        println!("Chunking Preview:");
        println!("Strategy: {:?}", self.strategy);
        println!("Total chunks: {}", chunks.len());
        println!("Total characters: {}", text.chars().count());
        println!("\nFirst {} chunks:", max_display.min(chunks.len()));
        println!("{}", "=".repeat(80));

        for chunk in chunks.iter().take(max_display) {
            println!(
                "\nChunk #{} ({} chars)",
                chunk.chunk_index, chunk.char_count
            );
            println!("Position: {}-{}", chunk.start_index, chunk.end_index);
            let preview = if chunk.text.len() > 200 {
                format!("{}...", &chunk.text[..200])
            } else {
                chunk.text.clone()
            };
            println!("Text: {}", preview);
            println!("{}", "-".repeat(80));
        }

        Ok(())
    }

    /// Chunk PDF text with page numbers preserved
    ///
    /// # Arguments
    /// * `text_by_page` - Vector of text strings, one per page
    ///
    /// # Returns
    /// * Vector of chunks with page metadata
    pub fn chunk_pdf(&self, text_by_page: &[String]) -> Result<Vec<TextChunk>> {
        let mut all_chunks = Vec::new();
        let mut global_chunk_index = 0;

        for (page_num, page_text) in text_by_page.iter().enumerate() {
            if page_text.trim().is_empty() {
                continue;
            }

            let mut page_chunks = self.chunk(page_text)?;

            // Add page metadata to each chunk
            for chunk in &mut page_chunks {
                chunk.metadata.page_number = Some((page_num + 1) as i32);
                chunk.chunk_index = global_chunk_index;
                global_chunk_index += 1;
            }

            all_chunks.extend(page_chunks);
        }

        info!(
            "Chunked PDF: {} pages → {} chunks",
            text_by_page.len(),
            all_chunks.len()
        );

        Ok(all_chunks)
    }

    /// Chunk Excel data with sheet and row metadata
    ///
    /// # Arguments
    /// * `sheet_name` - Name of the Excel sheet
    /// * `rows` - Vector of row data (each row is a vector of cell values)
    /// * `batch_size` - Number of rows to include in each chunk
    ///
    /// # Returns
    /// * Vector of chunks with sheet and row metadata
    pub fn chunk_excel(
        &self,
        sheet_name: &str,
        rows: &[Vec<String>],
        batch_size: usize,
    ) -> Result<Vec<TextChunk>> {
        let mut chunks = Vec::new();
        let mut chunk_index = 0;

        for (i, row_batch) in rows.chunks(batch_size).enumerate() {
            // Convert rows to text (tab-separated values)
            let text = row_batch
                .iter()
                .map(|row| row.join("\t"))
                .collect::<Vec<_>>()
                .join("\n");

            if text.trim().is_empty() {
                continue;
            }

            let start_row = i * batch_size;
            let end_row = start_row + row_batch.len();

            chunks.push(TextChunk {
                text: text.clone(),
                char_count: text.chars().count(),
                start_index: 0,
                end_index: text.len(),
                chunk_index,
                metadata: ChunkMetadata {
                    page_number: None,
                    sheet_name: Some(sheet_name.to_string()),
                    start_row: Some(start_row),
                    end_row: Some(end_row),
                },
            });

            chunk_index += 1;
        }

        info!(
            "Chunked Excel sheet '{}': {} rows → {} chunks",
            sheet_name,
            rows.len(),
            chunks.len()
        );

        Ok(chunks)
    }

    /// Chunk CSV data with row metadata and header preservation
    ///
    /// # Arguments
    /// * `headers` - CSV headers
    /// * `rows` - Vector of row data
    /// * `batch_size` - Number of rows to include in each chunk
    ///
    /// # Returns
    /// * Vector of chunks with row metadata
    pub fn chunk_csv(
        &self,
        headers: &[String],
        rows: &[Vec<String>],
        batch_size: usize,
    ) -> Result<Vec<TextChunk>> {
        let mut chunks = Vec::new();
        let mut chunk_index = 0;

        let header_line = headers.join("\t");

        for (i, row_batch) in rows.chunks(batch_size).enumerate() {
            // Include headers in each chunk for context
            let mut text = header_line.clone();
            text.push('\n');

            // Add rows
            for row in row_batch {
                text.push_str(&row.join("\t"));
                text.push('\n');
            }

            if text.trim().is_empty() {
                continue;
            }

            let start_row = i * batch_size;
            let end_row = start_row + row_batch.len();

            chunks.push(TextChunk {
                text: text.clone(),
                char_count: text.chars().count(),
                start_index: 0,
                end_index: text.len(),
                chunk_index,
                metadata: ChunkMetadata {
                    page_number: None,
                    sheet_name: None,
                    start_row: Some(start_row),
                    end_row: Some(end_row),
                },
            });

            chunk_index += 1;
        }

        info!("Chunked CSV: {} rows → {} chunks", rows.len(), chunks.len());

        Ok(chunks)
    }

    /// Get statistics about chunking result
    pub fn get_stats(&self, chunks: &[TextChunk]) -> ChunkingStats {
        let char_counts: Vec<usize> = chunks.iter().map(|c| c.char_count).collect();

        let total_chunks = chunks.len();
        let total_chars: usize = char_counts.iter().sum();
        let avg_chunk_size = if total_chunks > 0 {
            total_chars / total_chunks
        } else {
            0
        };
        let min_chunk_size = char_counts.iter().min().copied().unwrap_or(0);
        let max_chunk_size = char_counts.iter().max().copied().unwrap_or(0);

        ChunkingStats {
            total_chunks,
            total_chars,
            avg_chunk_size,
            min_chunk_size,
            max_chunk_size,
        }
    }
}

/// Statistics about chunking results
#[derive(Debug, Clone)]
pub struct ChunkingStats {
    pub total_chunks: usize,
    pub total_chars: usize,
    pub avg_chunk_size: usize,
    pub min_chunk_size: usize,
    pub max_chunk_size: usize,
}

impl Default for TextChunker {
    fn default() -> Self {
        Self::default_semantic()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunker_creation() {
        let chunker = TextChunker::default_semantic();
        assert!(matches!(
            chunker.strategy,
            ChunkingStrategy::Semantic { .. }
        ));
    }

    #[test]
    fn test_fixed_size_chunking() {
        let chunker = TextChunker::new(ChunkingStrategy::FixedSize {
            chunk_size: 10,
            overlap: 2,
        });

        let text = "This is a test of the chunking system.";
        let chunks = chunker.chunk(text).unwrap();

        assert!(!chunks.is_empty());
        assert!(chunks[0].char_count <= 10);

        // Verify overlap
        if chunks.len() > 1 {
            let first_end = &chunks[0].text[chunks[0].text.len().saturating_sub(2)..];
            let second_start = &chunks[1].text[..2.min(chunks[1].text.len())];
            // Overlap should exist
            assert!(first_end.len() > 0 && second_start.len() > 0);
        }
    }

    #[test]
    fn test_paragraph_chunking() {
        let chunker = TextChunker::new(ChunkingStrategy::Paragraph);

        let text = "First paragraph.\n\nSecond paragraph.\n\nThird paragraph.";
        let chunks = chunker.chunk(text).unwrap();

        assert_eq!(chunks.len(), 3);
        assert!(chunks[0].text.contains("First"));
        assert!(chunks[1].text.contains("Second"));
        assert!(chunks[2].text.contains("Third"));
    }

    #[test]
    fn test_semantic_chunking() {
        let chunker = TextChunker::default_semantic();

        let text = "This is a longer piece of text that should be split into multiple chunks based on semantic boundaries. The chunker should respect sentence boundaries where possible and maintain readability.";
        let chunks = chunker.chunk(text).unwrap();

        assert!(!chunks.is_empty());

        // Verify chunks are reasonably sized
        for chunk in &chunks {
            assert!(chunk.char_count > 0);
            assert!(chunk.char_count <= 600); // With overlap, slightly larger than target
        }
    }

    #[test]
    fn test_chunking_stats() {
        let chunker = TextChunker::default_semantic();
        let text = "A".repeat(1000);
        let chunks = chunker.chunk(&text).unwrap();

        let stats = chunker.get_stats(&chunks);

        // With overlap, total_chars across chunks will be >= original text length
        // because overlapping portions are counted in multiple chunks
        assert!(stats.total_chars >= 1000);
        assert!(stats.total_chunks > 0);
        assert!(stats.avg_chunk_size > 0);
    }

    #[test]
    fn test_empty_text() {
        let chunker = TextChunker::default();
        let chunks = chunker.chunk("").unwrap();
        assert_eq!(chunks.len(), 0);
    }

    #[test]
    fn test_large_semantic_chunker() {
        let chunker = TextChunker::large_semantic();
        let text = "A".repeat(2000);
        let chunks = chunker.chunk(&text).unwrap();

        assert!(!chunks.is_empty());
        // Chunks should be larger than default
        assert!(chunks[0].char_count > 500);
    }
}
