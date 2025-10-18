///! Enhanced Semantic Chunking with Document Structure Awareness
///!
///! Provides advanced chunking strategies that respect document semantics:
///! - Section-based chunking (respects headings and structure)
///! - Sentence-boundary awareness
///! - Context preservation across chunks
///! - Financial document-specific patterns
use anyhow::Result;
use regex::Regex;
use tracing::{debug, info};

use super::text_chunker::{ChunkMetadata, TextChunk};

/// Enhanced chunking strategies
#[derive(Debug, Clone)]
pub enum EnhancedChunkingStrategy {
    /// Semantic chunking with section awareness (for structured documents)
    SemanticSections {
        min_chunk_size: usize,
        max_chunk_size: usize,
        overlap: usize,
    },
    /// Hierarchical chunking (parent-child chunks for context)
    Hierarchical {
        parent_size: usize,
        child_size: usize,
        overlap: usize,
    },
    /// Financial document-specific chunking
    FinancialDocument { target_size: usize, overlap: usize },
}

/// Enhanced text chunker with document structure awareness
pub struct EnhancedTextChunker {
    strategy: EnhancedChunkingStrategy,
    sentence_pattern: Regex,
    section_pattern: Regex,
    financial_pattern: Regex,
}

impl EnhancedTextChunker {
    /// Create a new enhanced chunker with specified strategy
    pub fn new(strategy: EnhancedChunkingStrategy) -> Self {
        Self {
            strategy,
            // Sentence boundary detection (simplified without look-around)
            sentence_pattern: Regex::new(
                r"[.!?]\s+[A-Z]|[.!?]\n+|\n\n+"
            ).unwrap(),
            // Section heading detection
            section_pattern: Regex::new(
                r"(?m)^(?:#{1,6}\s+|\d+\.\s+|[IVX]+\.\s+|[A-Z][A-Z\s]+:)"
            ).unwrap(),
            // Financial terms that should stay together
            financial_pattern: Regex::new(
                r"\$[\d,]+\.?\d*|[\d,]+\.?\d*%|\d+\s+(?:basis\s+)?points?|LIBOR|SOFR|principal|interest|maturity|collateral|covenant"
            ).unwrap(),
        }
    }

    /// Default configuration optimized for credit documents
    pub fn default_financial() -> Self {
        Self::new(EnhancedChunkingStrategy::FinancialDocument {
            target_size: 600,
            overlap: 75,
        })
    }

    /// Chunk text with enhanced semantic awareness
    pub fn chunk(&self, text: &str) -> Result<Vec<TextChunk>> {
        info!(
            "Enhanced chunking: {} chars with strategy {:?}",
            text.len(),
            self.strategy
        );

        let chunks = match &self.strategy {
            EnhancedChunkingStrategy::SemanticSections {
                min_chunk_size,
                max_chunk_size,
                overlap,
            } => self.chunk_by_sections(text, *min_chunk_size, *max_chunk_size, *overlap),
            EnhancedChunkingStrategy::Hierarchical {
                parent_size,
                child_size,
                overlap,
            } => self.chunk_hierarchical(text, *parent_size, *child_size, *overlap),
            EnhancedChunkingStrategy::FinancialDocument {
                target_size,
                overlap,
            } => self.chunk_financial_document(text, *target_size, *overlap),
        };

        debug!("Created {} enhanced chunks", chunks.len());
        Ok(chunks)
    }

    /// Chunk by sections while respecting document structure
    fn chunk_by_sections(
        &self,
        text: &str,
        min_size: usize,
        max_size: usize,
        overlap: usize,
    ) -> Vec<TextChunk> {
        let mut chunks = Vec::new();
        let mut chunk_index = 0;

        // Split into sections first
        let sections = self.split_into_sections(text);

        for section in sections {
            // If section is too large, split it further
            if section.text.chars().count() > max_size {
                let sub_chunks = self.split_large_section(&section.text, max_size, overlap);
                for sub_text in sub_chunks {
                    chunks.push(TextChunk {
                        text: sub_text.clone(),
                        char_count: sub_text.chars().count(),
                        start_index: section.start_pos,
                        end_index: section.start_pos + sub_text.len(),
                        chunk_index,
                        metadata: ChunkMetadata {
                            page_number: None,
                            sheet_name: None,
                            start_row: None,
                            end_row: None,
                        },
                    });
                    chunk_index += 1;
                }
            } else if section.text.chars().count() >= min_size {
                // Section is good size, keep as-is
                chunks.push(TextChunk {
                    text: section.text.clone(),
                    char_count: section.text.chars().count(),
                    start_index: section.start_pos,
                    end_index: section.end_pos,
                    chunk_index,
                    metadata: ChunkMetadata::default(),
                });
                chunk_index += 1;
            } else if !chunks.is_empty() {
                // Section too small, merge with previous chunk if possible
                let last_idx = chunks.len() - 1;
                let merged_text = format!("{}\n\n{}", chunks[last_idx].text, section.text);
                chunks[last_idx].text = merged_text.clone();
                chunks[last_idx].char_count = merged_text.chars().count();
                chunks[last_idx].end_index = section.end_pos;
            }
        }

        // Add overlap between chunks
        self.add_overlap_to_chunks(chunks, overlap)
    }

    /// Hierarchical chunking with parent-child relationships
    fn chunk_hierarchical(
        &self,
        text: &str,
        parent_size: usize,
        child_size: usize,
        overlap: usize,
    ) -> Vec<TextChunk> {
        let mut chunks = Vec::new();
        let chars: Vec<char> = text.chars().collect();
        let mut start = 0;
        let mut chunk_index = 0;

        while start < chars.len() {
            // Create parent chunk
            let parent_end = (start + parent_size).min(chars.len());
            let parent_text: String = chars[start..parent_end].iter().collect();

            // Split parent into child chunks
            let mut child_start = 0;
            let child_chars: Vec<char> = parent_text.chars().collect();

            while child_start < child_chars.len() {
                let child_end = (child_start + child_size).min(child_chars.len());
                let child_text: String = child_chars[child_start..child_end].iter().collect();

                chunks.push(TextChunk {
                    text: child_text.clone(),
                    char_count: child_text.chars().count(),
                    start_index: start + child_start,
                    end_index: start + child_end,
                    chunk_index,
                    metadata: ChunkMetadata::default(),
                });

                chunk_index += 1;
                child_start += child_size - overlap;

                if overlap >= child_size {
                    child_start += 1;
                }
            }

            start += parent_size - overlap;
            if overlap >= parent_size {
                start += 1;
            }
        }

        chunks
    }

    /// Financial document-specific chunking
    fn chunk_financial_document(
        &self,
        text: &str,
        target_size: usize,
        overlap: usize,
    ) -> Vec<TextChunk> {
        let mut chunks = Vec::new();
        let mut chunk_index = 0;

        // Split by sentences first to respect semantic boundaries
        let sentences = self.split_into_sentences(text);
        let mut current_chunk = String::new();
        let mut start_pos = 0;
        let mut current_pos = 0;

        for sentence in sentences {
            let sentence_len = sentence.chars().count();

            // Check if adding this sentence would exceed target size
            if current_chunk.chars().count() + sentence_len > target_size
                && !current_chunk.is_empty()
            {
                // Finalize current chunk
                chunks.push(TextChunk {
                    text: current_chunk.clone(),
                    char_count: current_chunk.chars().count(),
                    start_index: start_pos,
                    end_index: current_pos,
                    chunk_index,
                    metadata: ChunkMetadata::default(),
                });

                chunk_index += 1;

                // Start new chunk with overlap
                let overlap_text = self.get_last_n_chars(&current_chunk, overlap);
                current_chunk = overlap_text;
                start_pos = current_pos.saturating_sub(overlap);
            }

            // Add sentence to current chunk
            if !current_chunk.is_empty() && !current_chunk.ends_with(' ') {
                current_chunk.push(' ');
            }
            current_chunk.push_str(&sentence);
            current_pos += sentence_len;
        }

        // Add final chunk
        if !current_chunk.is_empty() {
            chunks.push(TextChunk {
                text: current_chunk.clone(),
                char_count: current_chunk.chars().count(),
                start_index: start_pos,
                end_index: current_pos,
                chunk_index,
                metadata: ChunkMetadata::default(),
            });
        }

        chunks
    }

    /// Split text into sections based on headings and structure
    fn split_into_sections(&self, text: &str) -> Vec<Section> {
        let mut sections = Vec::new();
        let matches: Vec<_> = self.section_pattern.find_iter(text).collect();

        if matches.is_empty() {
            // No sections found, treat whole text as one section
            return vec![Section {
                text: text.to_string(),
                start_pos: 0,
                end_pos: text.len(),
                heading: None,
            }];
        }

        // Add content before first heading
        if matches[0].start() > 0 {
            sections.push(Section {
                text: text[..matches[0].start()].to_string(),
                start_pos: 0,
                end_pos: matches[0].start(),
                heading: None,
            });
        }

        // Process each section
        for i in 0..matches.len() {
            let start = matches[i].start();
            let end = if i + 1 < matches.len() {
                matches[i + 1].start()
            } else {
                text.len()
            };

            sections.push(Section {
                text: text[start..end].to_string(),
                start_pos: start,
                end_pos: end,
                heading: Some(matches[i].as_str().to_string()),
            });
        }

        sections
    }

    /// Split text into sentences
    fn split_into_sentences(&self, text: &str) -> Vec<String> {
        self.sentence_pattern
            .split(text)
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.trim().to_string())
            .collect()
    }

    /// Split large section into smaller chunks
    fn split_large_section(&self, text: &str, max_size: usize, overlap: usize) -> Vec<String> {
        let sentences = self.split_into_sentences(text);
        let mut chunks = Vec::new();
        let mut current_chunk = String::new();

        for sentence in sentences {
            if current_chunk.chars().count() + sentence.chars().count() > max_size
                && !current_chunk.is_empty()
            {
                chunks.push(current_chunk.clone());

                // Start new chunk with overlap
                current_chunk = self.get_last_n_chars(&current_chunk, overlap);
            }

            if !current_chunk.is_empty() && !current_chunk.ends_with(' ') {
                current_chunk.push(' ');
            }
            current_chunk.push_str(&sentence);
        }

        if !current_chunk.is_empty() {
            chunks.push(current_chunk);
        }

        chunks
    }

    /// Get last N characters from text
    fn get_last_n_chars(&self, text: &str, n: usize) -> String {
        let chars: Vec<char> = text.chars().collect();
        if chars.len() <= n {
            text.to_string()
        } else {
            chars[chars.len() - n..].iter().collect()
        }
    }

    /// Add overlap between consecutive chunks
    fn add_overlap_to_chunks(&self, mut chunks: Vec<TextChunk>, overlap: usize) -> Vec<TextChunk> {
        if overlap == 0 || chunks.len() < 2 {
            return chunks;
        }

        for i in 1..chunks.len() {
            let prev_text = &chunks[i - 1].text;
            let overlap_text = self.get_last_n_chars(prev_text, overlap);

            // Prepend overlap to current chunk
            chunks[i].text = format!("{} {}", overlap_text, chunks[i].text);
            chunks[i].char_count = chunks[i].text.chars().count();
        }

        chunks
    }
}

/// Section structure for document parsing
#[derive(Debug, Clone)]
struct Section {
    text: String,
    start_pos: usize,
    end_pos: usize,
    heading: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_financial_chunking() {
        let chunker = EnhancedTextChunker::default_financial();
        let text = "The principal amount is $1,000,000. The interest rate is 5.5% per annum. \
                     The maturity date is December 31, 2025. This is a senior secured loan.";

        let chunks = chunker.chunk(text).unwrap();
        assert!(!chunks.is_empty());
        assert!(chunks[0].text.contains("$1,000,000"));
    }

    #[test]
    fn test_section_splitting() {
        let chunker = EnhancedTextChunker::new(EnhancedChunkingStrategy::SemanticSections {
            min_chunk_size: 50,
            max_chunk_size: 200,
            overlap: 20,
        });

        let text =
            "# Introduction\nThis is the introduction section with enough content to exceed the minimum chunk size requirement.\n\n# Section 1\nThis is section one content with sufficient length to be treated as a separate chunk.";
        let chunks = chunker.chunk(text).unwrap();
        assert!(chunks.len() >= 2);
    }
}
