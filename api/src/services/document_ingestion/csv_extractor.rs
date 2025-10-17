use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use csv::ReaderBuilder;
use std::path::Path;
use tracing::{debug, info, warn};

/// CSV data with metadata
#[derive(Debug, Clone)]
pub struct CsvData {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub row_count: usize,
    pub column_count: usize,
}

/// CSV metadata
#[derive(Debug, Clone)]
pub struct CsvMetadata {
    pub filename: String,
    pub delimiter: u8,
    pub has_headers: bool,
    pub row_count: usize,
    pub column_count: usize,
    pub file_size_bytes: usize,
    pub extraction_time: DateTime<Utc>,
}

/// Result of CSV extraction
#[derive(Debug, Clone)]
pub struct CsvExtractionResult {
    pub data: CsvData,
    pub metadata: CsvMetadata,
    pub extraction_errors: Vec<String>,
}

/// CSV Extraction Service
///
/// Extracts data from CSV files with automatic delimiter detection
pub struct CsvExtractor {
    /// Delimiter to use (auto-detected if None)
    delimiter: Option<u8>,
    /// Whether the CSV has headers
    has_headers: bool,
}

impl CsvExtractor {
    /// Create a new CSV extractor with auto-detection
    pub fn new() -> Self {
        Self {
            delimiter: None,
            has_headers: true,
        }
    }

    /// Create a CSV extractor with explicit delimiter
    pub fn with_delimiter(delimiter: u8) -> Self {
        Self {
            delimiter: Some(delimiter),
            has_headers: true,
        }
    }

    /// Create a CSV extractor without headers
    pub fn without_headers(mut self) -> Self {
        self.has_headers = false;
        self
    }

    /// Extract data and metadata from CSV file
    ///
    /// # Arguments
    /// * `csv_path` - Path to the CSV file
    ///
    /// # Returns
    /// * `CsvExtractionResult` containing data, metadata, and errors
    pub fn extract_from_file(&self, csv_path: impl AsRef<Path>) -> Result<CsvExtractionResult> {
        let path = csv_path.as_ref();

        info!("Extracting CSV file: {}", path.display());

        // Get file size
        let file_size = std::fs::metadata(path)
            .context("Failed to get file metadata")?
            .len() as usize;

        // Check file size limit (50MB as per requirements)
        if file_size > 50 * 1024 * 1024 {
            anyhow::bail!("File size exceeds 50MB limit: {} bytes", file_size);
        }

        // Read file content for delimiter detection
        let content = std::fs::read_to_string(path)
            .context("Failed to read CSV file - file may be corrupted or in wrong encoding")?;

        self.extract_from_string(&content, path)
    }

    /// Extract data from CSV bytes
    ///
    /// # Arguments
    /// * `csv_bytes` - CSV file content as bytes
    /// * `filename` - Original filename for metadata
    ///
    /// # Returns
    /// * `CsvExtractionResult` containing data, metadata, and errors
    pub fn extract_from_bytes(
        &self,
        csv_bytes: &[u8],
        filename: &str,
    ) -> Result<CsvExtractionResult> {
        info!(
            "Extracting CSV from {} bytes (filename: {})",
            csv_bytes.len(),
            filename
        );

        let file_size = csv_bytes.len();

        // Check file size limit (50MB)
        if file_size > 50 * 1024 * 1024 {
            anyhow::bail!("File size exceeds 50MB limit: {} bytes", file_size);
        }

        let content = String::from_utf8(csv_bytes.to_vec())
            .context("Failed to decode CSV - invalid UTF-8 encoding")?;

        self.extract_from_string(&content, Path::new(filename))
    }

    /// Extract from string content
    fn extract_from_string(&self, content: &str, path: &Path) -> Result<CsvExtractionResult> {
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        // Detect delimiter if not specified
        let delimiter = match self.delimiter {
            Some(d) => d,
            None => self.detect_delimiter(content),
        };

        debug!("Using delimiter: {:?}", delimiter as char);

        // Parse CSV
        let mut reader = ReaderBuilder::new()
            .delimiter(delimiter)
            .has_headers(self.has_headers)
            .flexible(true) // Allow variable column counts
            .from_reader(content.as_bytes());

        let mut errors = Vec::new();

        // Extract headers if present
        let headers = if self.has_headers {
            reader
                .headers()
                .context("Failed to read CSV headers")?
                .iter()
                .map(|s| s.to_string())
                .collect()
        } else {
            Vec::new()
        };

        let column_count = headers.len();

        // Extract rows
        let mut rows = Vec::new();
        for (i, result) in reader.records().enumerate() {
            match result {
                Ok(record) => {
                    let row: Vec<String> = record.iter().map(|s| s.to_string()).collect();
                    rows.push(row);
                }
                Err(e) => {
                    let error_msg = format!("Failed to read row {}: {}", i + 1, e);
                    warn!("{}", error_msg);
                    errors.push(error_msg);
                }
            }
        }

        let row_count = rows.len();

        info!("Extracted {} rows with {} columns", row_count, column_count);

        Ok(CsvExtractionResult {
            data: CsvData {
                headers,
                rows,
                row_count,
                column_count,
            },
            metadata: CsvMetadata {
                filename,
                delimiter,
                has_headers: self.has_headers,
                row_count,
                column_count,
                file_size_bytes: content.len(),
                extraction_time: Utc::now(),
            },
            extraction_errors: errors,
        })
    }

    /// Detect CSV delimiter by analyzing first few lines
    ///
    /// Checks for common delimiters: comma, tab, semicolon, pipe
    fn detect_delimiter(&self, content: &str) -> u8 {
        let delimiters = [b',', b'\t', b';', b'|'];
        let lines: Vec<&str> = content.lines().take(5).collect();

        if lines.is_empty() {
            return b','; // Default to comma
        }

        // Count occurrences of each delimiter
        let mut counts = vec![0; delimiters.len()];
        for line in &lines {
            for (i, &delimiter) in delimiters.iter().enumerate() {
                counts[i] += line.as_bytes().iter().filter(|&&c| c == delimiter).count();
            }
        }

        // Find delimiter with highest average count per line
        let mut max_count = 0;
        let mut best_delimiter = b',';

        for (i, &count) in counts.iter().enumerate() {
            let avg = count / lines.len();
            if avg > max_count {
                max_count = avg;
                best_delimiter = delimiters[i];
            }
        }

        debug!(
            "Detected delimiter: {:?} (counts: {:?})",
            best_delimiter as char, counts
        );

        best_delimiter
    }

    /// Convert CSV to text representation
    ///
    /// Each row becomes a line with tab-separated values
    pub fn to_text(&self, result: &CsvExtractionResult) -> String {
        let mut text = String::new();

        // Add headers if present
        if !result.data.headers.is_empty() {
            text.push_str(&result.data.headers.join("\t"));
            text.push('\n');
        }

        // Add rows
        for row in &result.data.rows {
            text.push_str(&row.join("\t"));
            text.push('\n');
        }

        text
    }

    /// Convert CSV to text with row numbers
    pub fn to_text_with_row_numbers(&self, result: &CsvExtractionResult) -> String {
        let mut text = String::new();

        // Add headers with row number column
        if !result.data.headers.is_empty() {
            text.push_str("Row\t");
            text.push_str(&result.data.headers.join("\t"));
            text.push('\n');
        }

        // Add rows with row numbers
        for (i, row) in result.data.rows.iter().enumerate() {
            text.push_str(&format!("{}\t", i + 1));
            text.push_str(&row.join("\t"));
            text.push('\n');
        }

        text
    }

    /// Get row count without full extraction
    pub fn get_row_count(&self, csv_path: impl AsRef<Path>) -> Result<usize> {
        let content = std::fs::read_to_string(csv_path)?;
        let lines = content.lines().count();

        // Subtract 1 for header if present
        Ok(if self.has_headers {
            lines.saturating_sub(1)
        } else {
            lines
        })
    }
}

impl Default for CsvExtractor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_csv_extractor_creation() {
        let extractor = CsvExtractor::new();
        assert!(extractor.has_headers);
        assert!(extractor.delimiter.is_none());
    }

    #[test]
    fn test_csv_extractor_with_delimiter() {
        let extractor = CsvExtractor::with_delimiter(b';');
        assert_eq!(extractor.delimiter, Some(b';'));
    }

    #[test]
    fn test_delimiter_detection() {
        let extractor = CsvExtractor::new();

        // Test comma-separated
        let csv = "a,b,c\n1,2,3\n4,5,6";
        assert_eq!(extractor.detect_delimiter(csv), b',');

        // Test tab-separated
        let csv = "a\tb\tc\n1\t2\t3\n4\t5\t6";
        assert_eq!(extractor.detect_delimiter(csv), b'\t');

        // Test semicolon-separated
        let csv = "a;b;c\n1;2;3\n4;5;6";
        assert_eq!(extractor.detect_delimiter(csv), b';');

        // Test pipe-separated
        let csv = "a|b|c\n1|2|3\n4|5|6";
        assert_eq!(extractor.detect_delimiter(csv), b'|');
    }

    #[test]
    fn test_extract_from_string() {
        let extractor = CsvExtractor::new();
        let csv = "Name,Age,City\nAlice,30,NYC\nBob,25,LA";

        let result = extractor
            .extract_from_string(csv, Path::new("test.csv"))
            .unwrap();

        assert_eq!(result.data.headers, vec!["Name", "Age", "City"]);
        assert_eq!(result.data.row_count, 2);
        assert_eq!(result.data.column_count, 3);
        assert_eq!(result.data.rows[0], vec!["Alice", "30", "NYC"]);
        assert_eq!(result.data.rows[1], vec!["Bob", "25", "LA"]);
    }

    #[test]
    fn test_to_text() {
        let extractor = CsvExtractor::new();
        let result = CsvExtractionResult {
            data: CsvData {
                headers: vec!["A".to_string(), "B".to_string()],
                rows: vec![vec!["1".to_string(), "2".to_string()]],
                row_count: 1,
                column_count: 2,
            },
            metadata: CsvMetadata {
                filename: "test.csv".to_string(),
                delimiter: b',',
                has_headers: true,
                row_count: 1,
                column_count: 2,
                file_size_bytes: 100,
                extraction_time: Utc::now(),
            },
            extraction_errors: vec![],
        };

        let text = extractor.to_text(&result);
        assert!(text.contains("A\tB"));
        assert!(text.contains("1\t2"));
    }

    #[test]
    fn test_to_text_with_row_numbers() {
        let extractor = CsvExtractor::new();
        let result = CsvExtractionResult {
            data: CsvData {
                headers: vec!["A".to_string(), "B".to_string()],
                rows: vec![
                    vec!["1".to_string(), "2".to_string()],
                    vec!["3".to_string(), "4".to_string()],
                ],
                row_count: 2,
                column_count: 2,
            },
            metadata: CsvMetadata {
                filename: "test.csv".to_string(),
                delimiter: b',',
                has_headers: true,
                row_count: 2,
                column_count: 2,
                file_size_bytes: 100,
                extraction_time: Utc::now(),
            },
            extraction_errors: vec![],
        };

        let text = extractor.to_text_with_row_numbers(&result);
        assert!(text.contains("Row\tA\tB"));
        assert!(text.contains("1\t1\t2"));
        assert!(text.contains("2\t3\t4"));
    }

    // Integration tests require actual CSV files
    #[test]
    #[ignore]
    fn test_extract_sample_csv() {
        let extractor = CsvExtractor::new();
        let result = extractor.extract_from_file("test_data/sample.csv");
        assert!(result.is_ok());

        let extraction = result.unwrap();
        assert!(extraction.data.row_count > 0);
    }
}
