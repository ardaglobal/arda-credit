use anyhow::{Context, Result};
use calamine::{open_workbook_auto, Reader};
use chrono::{DateTime, Utc};
use std::path::Path;
use tracing::{debug, info, warn};

/// Excel sheet data with metadata
#[derive(Debug, Clone)]
pub struct ExcelSheet {
    pub name: String,
    pub rows: Vec<Vec<String>>,
    pub row_count: usize,
    pub column_count: usize,
}

/// Excel metadata extracted from workbook
#[derive(Debug, Clone)]
pub struct ExcelMetadata {
    pub filename: String,
    pub sheet_names: Vec<String>,
    pub total_sheets: usize,
    pub total_rows: usize,
    pub file_size_bytes: usize,
    pub extraction_time: DateTime<Utc>,
}

/// Result of Excel extraction
#[derive(Debug, Clone)]
pub struct ExcelExtractionResult {
    pub sheets: Vec<ExcelSheet>,
    pub metadata: ExcelMetadata,
    pub extraction_errors: Vec<String>,
}

/// Excel Extraction Service
///
/// Extracts data from Excel files (XLS, XLSX, XLSM, ODS)
pub struct ExcelExtractor;

impl ExcelExtractor {
    /// Create a new Excel extractor
    pub fn new() -> Self {
        Self
    }

    /// Extract data and metadata from Excel file
    ///
    /// # Arguments
    /// * `excel_path` - Path to the Excel file
    ///
    /// # Returns
    /// * `ExcelExtractionResult` containing sheets, metadata, and errors
    pub fn extract_from_file(&self, excel_path: impl AsRef<Path>) -> Result<ExcelExtractionResult> {
        let path = excel_path.as_ref();

        info!("Extracting Excel file: {}", path.display());

        // Get file size
        let file_size = std::fs::metadata(path)
            .context("Failed to get file metadata")?
            .len() as usize;

        // Check file size limit (50MB as per requirements)
        if file_size > 50 * 1024 * 1024 {
            anyhow::bail!("File size exceeds 50MB limit: {} bytes", file_size);
        }

        // Open workbook - auto-detects format (XLS, XLSX, etc.)
        let mut workbook = open_workbook_auto(path)
            .context("Failed to open Excel file - file may be corrupted or unsupported")?;

        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let sheet_names = workbook.sheet_names().to_vec();
        let total_sheets = sheet_names.len();

        debug!("Found {} sheets in workbook", total_sheets);

        // Extract all sheets
        let mut sheets = Vec::new();
        let mut errors = Vec::new();
        let mut total_rows = 0;

        for sheet_name in &sheet_names {
            match self.extract_sheet(&mut workbook, sheet_name) {
                Ok(sheet) => {
                    total_rows += sheet.row_count;
                    sheets.push(sheet);
                }
                Err(e) => {
                    let error_msg = format!("Failed to extract sheet '{}': {}", sheet_name, e);
                    warn!("{}", error_msg);
                    errors.push(error_msg);
                }
            }
        }

        info!(
            "Extracted {} sheets with {} total rows",
            sheets.len(),
            total_rows
        );

        Ok(ExcelExtractionResult {
            sheets,
            metadata: ExcelMetadata {
                filename,
                sheet_names,
                total_sheets,
                total_rows,
                file_size_bytes: file_size,
                extraction_time: Utc::now(),
            },
            extraction_errors: errors,
        })
    }

    /// Extract a single sheet from workbook
    fn extract_sheet(
        &self,
        workbook: &mut calamine::Sheets<std::io::BufReader<std::fs::File>>,
        sheet_name: &str,
    ) -> Result<ExcelSheet> {
        let range = workbook
            .worksheet_range(sheet_name)
            .context(format!("Sheet '{}' not found", sheet_name))?;

        let mut rows = Vec::new();
        let mut max_columns = 0;

        // Convert cells to strings
        for row in range.rows() {
            let string_row: Vec<String> = row
                .iter()
                .map(|cell| {
                    // Convert each cell to string
                    match cell {
                        calamine::Data::Empty => String::new(),
                        calamine::Data::String(s) => s.clone(),
                        calamine::Data::Float(f) => {
                            // Format floats nicely
                            if f.fract() == 0.0 {
                                format!("{:.0}", f) // No decimal for integers
                            } else {
                                f.to_string()
                            }
                        }
                        calamine::Data::Int(i) => i.to_string(),
                        calamine::Data::Bool(b) => b.to_string(),
                        calamine::Data::DateTime(dt) => dt.to_string(),
                        calamine::Data::DateTimeIso(dt) => dt.clone(),
                        calamine::Data::DurationIso(d) => d.clone(),
                        calamine::Data::Error(e) => format!("ERROR: {:?}", e),
                    }
                })
                .collect();

            if string_row.iter().any(|s| !s.is_empty()) {
                // Skip completely empty rows
                max_columns = max_columns.max(string_row.len());
                rows.push(string_row);
            }
        }

        let row_count = rows.len();

        Ok(ExcelSheet {
            name: sheet_name.to_string(),
            rows,
            row_count,
            column_count: max_columns,
        })
    }

    /// Extract data from Excel bytes (in-memory)
    ///
    /// # Arguments
    /// * `excel_bytes` - Excel file content as bytes
    /// * `filename` - Original filename (used for format detection)
    ///
    /// # Returns
    /// * `ExcelExtractionResult` containing sheets, metadata, and errors
    pub fn extract_from_bytes(
        &self,
        excel_bytes: &[u8],
        filename: &str,
    ) -> Result<ExcelExtractionResult> {
        info!(
            "Extracting Excel from {} bytes (filename: {})",
            excel_bytes.len(),
            filename
        );

        let file_size = excel_bytes.len();

        // Check file size limit (50MB)
        if file_size > 50 * 1024 * 1024 {
            anyhow::bail!("File size exceeds 50MB limit: {} bytes", file_size);
        }

        // Write to temp file (calamine requires file path)
        let temp_dir = std::env::temp_dir();
        let temp_path = temp_dir.join(format!("temp_excel_{}", filename));

        std::fs::write(&temp_path, excel_bytes).context("Failed to write temporary file")?;

        // Extract from temp file
        let result = self.extract_from_file(&temp_path);

        // Clean up temp file
        let _ = std::fs::remove_file(&temp_path);

        result
    }

    /// Convert Excel sheet to text representation
    ///
    /// Each row becomes a line with tab-separated values
    pub fn sheet_to_text(&self, sheet: &ExcelSheet) -> String {
        let mut text = format!("Sheet: {}\n", sheet.name);

        for row in &sheet.rows {
            let line = row.join("\t");
            text.push_str(&line);
            text.push('\n');
        }

        text
    }

    /// Convert entire workbook to text
    pub fn to_text(&self, result: &ExcelExtractionResult) -> String {
        let mut text = String::new();

        for (i, sheet) in result.sheets.iter().enumerate() {
            if i > 0 {
                text.push_str("\n\n");
            }
            text.push_str(&self.sheet_to_text(sheet));
        }

        text
    }

    /// Get sheet names without full extraction
    pub fn get_sheet_names(&self, excel_path: impl AsRef<Path>) -> Result<Vec<String>> {
        let workbook = open_workbook_auto(excel_path)?;
        Ok(workbook.sheet_names().to_vec())
    }
}

impl Default for ExcelExtractor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_excel_extractor_creation() {
        let extractor = ExcelExtractor::new();
        assert!(std::mem::size_of_val(&extractor) >= 0);
    }

    #[test]
    fn test_sheet_to_text() {
        let extractor = ExcelExtractor::new();
        let sheet = ExcelSheet {
            name: "TestSheet".to_string(),
            rows: vec![
                vec!["A".to_string(), "B".to_string(), "C".to_string()],
                vec!["1".to_string(), "2".to_string(), "3".to_string()],
            ],
            row_count: 2,
            column_count: 3,
        };

        let text = extractor.sheet_to_text(&sheet);
        assert!(text.contains("TestSheet"));
        assert!(text.contains("A\tB\tC"));
        assert!(text.contains("1\t2\t3"));
    }

    // Integration tests require actual Excel files
    #[test]
    #[ignore]
    fn test_extract_sample_excel() {
        let extractor = ExcelExtractor::new();
        let result = extractor.extract_from_file("test_data/sample.xlsx");
        assert!(result.is_ok());

        let extraction = result.unwrap();
        assert!(!extraction.sheets.is_empty());
        assert!(extraction.metadata.total_sheets > 0);
    }
}
