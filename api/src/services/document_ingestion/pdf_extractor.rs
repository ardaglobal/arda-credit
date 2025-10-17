use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use lopdf::Document;
use std::path::Path;
use tracing::{debug, info, warn};

/// PDF Metadata extracted from document
#[derive(Debug, Clone)]
pub struct PdfMetadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub subject: Option<String>,
    pub creator: Option<String>,
    pub producer: Option<String>,
    pub creation_date: Option<DateTime<Utc>>,
    pub modification_date: Option<DateTime<Utc>>,
    pub page_count: usize,
    pub pdf_version: String,
    pub is_encrypted: bool,
    pub file_size_bytes: usize,
}

/// Result of PDF extraction
#[derive(Debug, Clone)]
pub struct PdfExtractionResult {
    pub text: String,
    pub metadata: PdfMetadata,
    pub text_by_page: Vec<String>,
    pub extraction_errors: Vec<String>,
}

/// PDF Extraction Service
///
/// Extracts text and metadata from PDF documents
pub struct PdfExtractor;

impl PdfExtractor {
    /// Create a new PDF extractor
    pub fn new() -> Self {
        Self
    }

    /// Extract text and metadata from PDF file
    ///
    /// # Arguments
    /// * `pdf_path` - Path to the PDF file
    ///
    /// # Returns
    /// * `PdfExtractionResult` containing text, metadata, and per-page text
    pub fn extract_from_file(&self, pdf_path: impl AsRef<Path>) -> Result<PdfExtractionResult> {
        let path = pdf_path.as_ref();

        info!("Extracting PDF: {}", path.display());

        // Get file size
        let file_size = std::fs::metadata(path)
            .context("Failed to get file metadata")?
            .len() as usize;

        // Check file size limit (50MB as per requirements)
        if file_size > 50 * 1024 * 1024 {
            anyhow::bail!("File size exceeds 50MB limit: {} bytes", file_size);
        }

        // Load PDF document
        let doc = Document::load(path)
            .context("Failed to load PDF document - file may be corrupted or encrypted")?;

        // Extract metadata
        let metadata = self.extract_metadata(&doc, file_size)?;

        debug!(
            "PDF metadata: {} pages, version {}",
            metadata.page_count, metadata.pdf_version
        );

        // Extract text
        let (text, text_by_page, errors) = self.extract_text(&doc)?;

        if !errors.is_empty() {
            warn!("PDF extraction had {} errors", errors.len());
        }

        info!(
            "Extracted {} characters from {} pages",
            text.len(),
            text_by_page.len()
        );

        Ok(PdfExtractionResult {
            text,
            metadata,
            text_by_page,
            extraction_errors: errors,
        })
    }

    /// Extract text and metadata from PDF bytes
    ///
    /// # Arguments
    /// * `pdf_bytes` - PDF file content as bytes
    ///
    /// # Returns
    /// * `PdfExtractionResult` containing text, metadata, and per-page text
    pub fn extract_from_bytes(&self, pdf_bytes: &[u8]) -> Result<PdfExtractionResult> {
        info!("Extracting PDF from {} bytes", pdf_bytes.len());

        let file_size = pdf_bytes.len();

        // Check file size limit (50MB)
        if file_size > 50 * 1024 * 1024 {
            anyhow::bail!("File size exceeds 50MB limit: {} bytes", file_size);
        }

        // Load PDF from bytes
        let doc = Document::load_mem(pdf_bytes)
            .context("Failed to load PDF from memory - file may be corrupted or encrypted")?;

        // Extract metadata
        let metadata = self.extract_metadata(&doc, file_size)?;

        // Extract text
        let (text, text_by_page, errors) = self.extract_text(&doc)?;

        if !errors.is_empty() {
            warn!("PDF extraction had {} errors", errors.len());
        }

        Ok(PdfExtractionResult {
            text,
            metadata,
            text_by_page,
            extraction_errors: errors,
        })
    }

    /// Extract metadata from PDF document
    fn extract_metadata(&self, doc: &Document, file_size: usize) -> Result<PdfMetadata> {
        let page_count = doc.get_pages().len();
        let pdf_version = doc.version.clone();
        let is_encrypted = doc.is_encrypted();

        // Extract metadata dictionary
        let trailer = &doc.trailer;
        let mut metadata = PdfMetadata {
            title: None,
            author: None,
            subject: None,
            creator: None,
            producer: None,
            creation_date: None,
            modification_date: None,
            page_count,
            pdf_version,
            is_encrypted,
            file_size_bytes: file_size,
        };

        // Try to get Info dictionary
        if let Ok(info_ref) = trailer.get(b"Info") {
            if let Ok(info_obj) = doc.get_object(info_ref.as_reference().unwrap()) {
                if let Ok(info_dict) = info_obj.as_dict() {
                    // Extract string metadata fields
                    metadata.title = self.extract_string_field(info_dict, b"Title");
                    metadata.author = self.extract_string_field(info_dict, b"Author");
                    metadata.subject = self.extract_string_field(info_dict, b"Subject");
                    metadata.creator = self.extract_string_field(info_dict, b"Creator");
                    metadata.producer = self.extract_string_field(info_dict, b"Producer");

                    // Extract date fields
                    metadata.creation_date = self.extract_date_field(info_dict, b"CreationDate");
                    metadata.modification_date = self.extract_date_field(info_dict, b"ModDate");
                }
            }
        }

        Ok(metadata)
    }

    /// Extract text from all pages
    fn extract_text(&self, doc: &Document) -> Result<(String, Vec<String>, Vec<String>)> {
        let pages = doc.get_pages();
        let mut all_text = String::new();
        let mut text_by_page = Vec::new();
        let mut errors = Vec::new();

        for (page_num, page_id) in pages.iter() {
            match self.extract_page_text(doc, *page_id) {
                Ok(page_text) => {
                    let cleaned_text = self.clean_text(&page_text);

                    if !all_text.is_empty() && !cleaned_text.is_empty() {
                        all_text.push('\n');
                        all_text.push('\n');
                    }
                    all_text.push_str(&cleaned_text);
                    text_by_page.push(cleaned_text);
                }
                Err(e) => {
                    let error_msg = format!("Failed to extract text from page {}: {}", page_num, e);
                    warn!("{}", error_msg);
                    errors.push(error_msg);
                    text_by_page.push(String::new());
                }
            }
        }

        Ok((all_text, text_by_page, errors))
    }

    /// Extract text from a single page
    fn extract_page_text(&self, doc: &Document, page_id: (u32, u16)) -> Result<String> {
        // Use pdf-extract for actual text extraction
        // lopdf is mainly for structure parsing
        let text = doc
            .extract_text(&[page_id.0])
            .context("Failed to extract text from page")?;

        Ok(text)
    }

    /// Clean extracted text
    fn clean_text(&self, text: &str) -> String {
        text.lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Extract string field from dictionary
    fn extract_string_field(&self, dict: &lopdf::Dictionary, key: &[u8]) -> Option<String> {
        dict.get(key).ok().and_then(|obj| {
            if let lopdf::Object::String(bytes, _) = obj {
                Some(String::from_utf8_lossy(bytes).to_string())
            } else {
                None
            }
        })
    }

    /// Extract date field from dictionary
    fn extract_date_field(&self, dict: &lopdf::Dictionary, key: &[u8]) -> Option<DateTime<Utc>> {
        let date_str = self.extract_string_field(dict, key)?;

        // Parse PDF date format: D:YYYYMMDDHHmmSSOHH'mm'
        // Example: D:20240115120000+00'00'
        if let Some(date_part) = date_str.strip_prefix("D:") {
            // Simplified parsing - full PDF date parsing is complex
            let year = date_part.get(0..4)?.parse().ok()?;
            let month = date_part.get(4..6)?.parse().ok()?;
            let day = date_part.get(6..8)?.parse().ok()?;

            chrono::NaiveDate::from_ymd_opt(year, month, day)?
                .and_hms_opt(0, 0, 0)
                .map(|dt| DateTime::from_naive_utc_and_offset(dt, Utc))
        } else {
            None
        }
    }

    /// Get page count without full extraction
    pub fn get_page_count(&self, pdf_path: impl AsRef<Path>) -> Result<usize> {
        let doc = Document::load(pdf_path)?;
        Ok(doc.get_pages().len())
    }

    /// Check if PDF is encrypted
    pub fn is_encrypted(&self, pdf_path: impl AsRef<Path>) -> Result<bool> {
        let doc = Document::load(pdf_path)?;
        Ok(doc.is_encrypted())
    }
}

impl Default for PdfExtractor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pdf_extractor_creation() {
        let extractor = PdfExtractor::new();
        assert!(std::mem::size_of_val(&extractor) >= 0);
    }

    #[test]
    fn test_text_cleaning() {
        let extractor = PdfExtractor::new();
        let dirty_text = "  Line 1  \n  \n  Line 2  \n\n\nLine 3  ";
        let clean = extractor.clean_text(dirty_text);
        assert_eq!(clean, "Line 1\nLine 2\nLine 3");
    }

    // Integration tests require actual PDF files
    #[test]
    #[ignore]
    fn test_extract_sample_pdf() {
        let extractor = PdfExtractor::new();
        let result = extractor.extract_from_file("test_data/sample.pdf");
        assert!(result.is_ok());

        let extraction = result.unwrap();
        assert!(!extraction.text.is_empty());
        assert!(extraction.metadata.page_count > 0);
    }
}
