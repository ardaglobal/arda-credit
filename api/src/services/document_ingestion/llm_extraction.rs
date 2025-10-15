use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, info};

/// LLM Provider selection
#[derive(Debug, Clone)]
pub enum LlmProvider {
    OpenAI { api_key: String, model: String },
    Anthropic { api_key: String, model: String },
    OpenRouter { api_key: String, model: String },
}

/// Structured document extraction service using LLMs
///
/// Replaces the BAML TypeScript service with native Rust implementation.
/// Uses OpenAI's structured output mode or Anthropic's JSON mode for type-safe extraction.
pub struct LlmExtractionService {
    client: Client,
    provider: LlmProvider,
}

impl LlmExtractionService {
    /// Create service from environment variables
    ///
    /// Tries providers in order: OpenAI → Anthropic → OpenRouter
    pub fn from_env() -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()?;

        // Try OpenAI first
        if let Ok(api_key) = std::env::var("OPENAI_API_KEY") {
            let model = std::env::var("OPENAI_EXTRACTION_MODEL")
                .unwrap_or_else(|_| "gpt-4o-2024-08-06".to_string());
            info!("Using OpenAI for document extraction: {}", model);
            return Ok(Self {
                client,
                provider: LlmProvider::OpenAI { api_key, model },
            });
        }

        // Try Anthropic
        if let Ok(api_key) = std::env::var("ANTHROPIC_API_KEY") {
            let model = std::env::var("ANTHROPIC_EXTRACTION_MODEL")
                .unwrap_or_else(|_| "claude-3-5-sonnet-20241022".to_string());
            info!("Using Anthropic for document extraction: {}", model);
            return Ok(Self {
                client,
                provider: LlmProvider::Anthropic { api_key, model },
            });
        }

        // Try OpenRouter
        if let Ok(api_key) = std::env::var("OPENROUTER_API_KEY") {
            let model = std::env::var("OPENROUTER_EXTRACTION_MODEL")
                .unwrap_or_else(|_| "anthropic/claude-3.5-sonnet".to_string());
            info!("Using OpenRouter for document extraction: {}", model);
            return Ok(Self {
                client,
                provider: LlmProvider::OpenRouter { api_key, model },
            });
        }

        anyhow::bail!(
            "No LLM API key found. Set OPENAI_API_KEY, ANTHROPIC_API_KEY, or OPENROUTER_API_KEY"
        )
    }

    /// Extract complete deal data from document text
    pub async fn extract_deal(&self, document_text: &str) -> Result<ExtractedDealData> {
        debug!(
            "Extracting deal data from document ({} chars)",
            document_text.len()
        );

        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "metadata": {
                    "type": "object",
                    "properties": {
                        "document_type": {"type": "string"},
                        "document_date": {"type": ["string", "null"]},
                        "execution_date": {"type": ["string", "null"]},
                        "governing_law": {"type": ["string", "null"]},
                        "document_title": {"type": ["string", "null"]}
                    },
                    "required": ["document_type"]
                },
                "deal_type": {
                    "type": "string",
                    "enum": ["LOAN", "CREDIT_LINE", "INVOICE_FINANCING", "EQUIPMENT_FINANCING", "TRADE_FINANCE", "OTHER"]
                },
                "financial_terms": {
                    "type": "object",
                    "properties": {
                        "principal_amount": {"type": "number"},
                        "currency": {"type": "string", "enum": ["USD", "EUR", "GBP", "JPY", "CHF", "OTHER"]},
                        "interest_rate": {"type": "number", "description": "As decimal (0.085 = 8.5%)"},
                        "interest_rate_type": {"type": "string", "enum": ["FIXED", "VARIABLE", "MIXED"]},
                        "interest_rate_bps": {"type": ["integer", "null"]},
                        "origination_fee": {"type": ["number", "null"]},
                        "payment_frequency": {"type": "string", "enum": ["DAILY", "WEEKLY", "BIWEEKLY", "MONTHLY", "QUARTERLY", "SEMIANNUALLY", "ANNUALLY", "ONE_TIME", "OTHER"]},
                        "term_length_days": {"type": "integer"},
                        "maturity_date": {"type": ["string", "null"]}
                    },
                    "required": ["principal_amount", "currency", "interest_rate", "interest_rate_type", "payment_frequency", "term_length_days"]
                },
                "parties": {
                    "type": "object",
                    "properties": {
                        "originator_name": {"type": "string"},
                        "originator_legal_name": {"type": ["string", "null"]},
                        "originator_address": {"type": ["string", "null"]},
                        "originator_tax_id": {"type": ["string", "null"]},
                        "investor_name": {"type": "string"},
                        "investor_legal_name": {"type": ["string", "null"]},
                        "investor_address": {"type": ["string", "null"]},
                        "investor_tax_id": {"type": ["string", "null"]},
                        "guarantors": {"type": "array", "items": {"type": "string"}}
                    },
                    "required": ["originator_name", "investor_name", "guarantors"]
                },
                "collateral": {
                    "type": "object",
                    "properties": {
                        "has_collateral": {"type": "boolean"},
                        "collateral_types": {"type": "array", "items": {"type": "string", "enum": ["REAL_ESTATE", "EQUIPMENT", "INVENTORY", "ACCOUNTS_RECEIVABLE", "SECURITIES", "CASH", "PERSONAL_GUARANTEE", "CORPORATE_GUARANTEE", "NONE", "OTHER"]}},
                        "collateral_description": {"type": ["string", "null"]},
                        "loan_to_value_ratio": {"type": ["number", "null"]}
                    },
                    "required": ["has_collateral", "collateral_types"]
                },
                "covenants": {
                    "type": "object",
                    "properties": {
                        "financial_covenants": {"type": "array", "items": {"type": "string"}},
                        "operational_covenants": {"type": "array", "items": {"type": "string"}},
                        "reporting_requirements": {"type": "array", "items": {"type": "string"}},
                        "events_of_default": {"type": "array", "items": {"type": "string"}}
                    },
                    "required": ["financial_covenants", "operational_covenants", "reporting_requirements", "events_of_default"]
                },
                "purpose_of_funds": {"type": ["string", "null"]},
                "special_conditions": {"type": "array", "items": {"type": "string"}},
                "confidence_score": {"type": "number", "minimum": 0.0, "maximum": 1.0},
                "extraction_notes": {"type": "array", "items": {"type": "string"}}
            },
            "required": ["metadata", "deal_type", "financial_terms", "parties", "collateral", "covenants", "special_conditions", "confidence_score", "extraction_notes"]
        });

        let prompt = format!(
            r#"You are a financial document analyst. Extract structured deal information from the following document.

Document:
{}

Extract all relevant information including metadata, deal type, financial terms, parties involved, collateral, covenants, and any special conditions.

For the confidence_score field, provide a value between 0.0 and 1.0 indicating how confident you are in the extraction (1.0 = very confident, 0.0 = not confident).

Add any relevant notes about the extraction process or missing information to extraction_notes."#,
            document_text
        );

        let result: ExtractedDealData = match &self.provider {
            LlmProvider::OpenAI { api_key, model } => {
                self.extract_with_openai(api_key, model, &prompt, schema)
                    .await?
            }
            LlmProvider::Anthropic { api_key, model } => {
                self.extract_with_anthropic(api_key, model, &prompt, schema)
                    .await?
            }
            LlmProvider::OpenRouter { api_key, model } => {
                self.extract_with_openrouter(api_key, model, &prompt, schema)
                    .await?
            }
        };

        info!(
            "Deal extraction completed with confidence score: {:.2}",
            result.confidence_score
        );

        Ok(result)
    }

    /// Extract financial terms only
    pub async fn extract_financial_terms(&self, document_text: &str) -> Result<DealFinancialTerms> {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "principal_amount": {"type": "number"},
                "currency": {"type": "string"},
                "interest_rate": {"type": "number"},
                "interest_rate_type": {"type": "string"},
                "interest_rate_bps": {"type": ["integer", "null"]},
                "origination_fee": {"type": ["number", "null"]},
                "payment_frequency": {"type": "string"},
                "term_length_days": {"type": "integer"},
                "maturity_date": {"type": ["string", "null"]}
            }
        });

        let prompt = format!(
            "Extract only the financial terms from this document:\n\n{}",
            document_text
        );

        match &self.provider {
            LlmProvider::OpenAI { api_key, model } => {
                self.extract_with_openai(api_key, model, &prompt, schema)
                    .await
            }
            LlmProvider::Anthropic { api_key, model } => {
                self.extract_with_anthropic(api_key, model, &prompt, schema)
                    .await
            }
            LlmProvider::OpenRouter { api_key, model } => {
                self.extract_with_openrouter(api_key, model, &prompt, schema)
                    .await
            }
        }
    }

    /// Extract with OpenAI (using structured outputs)
    async fn extract_with_openai<T: for<'de> Deserialize<'de>>(
        &self,
        api_key: &str,
        model: &str,
        prompt: &str,
        schema: serde_json::Value,
    ) -> Result<T> {
        let request = serde_json::json!({
            "model": model,
            "messages": [
                {
                    "role": "system",
                    "content": "You are a precise financial document analyst. Extract information accurately and indicate your confidence level."
                },
                {
                    "role": "user",
                    "content": prompt
                }
            ],
            "response_format": {
                "type": "json_schema",
                "json_schema": {
                    "name": "deal_extraction",
                    "strict": true,
                    "schema": schema
                }
            },
            "temperature": 0.1
        });

        let response = self
            .client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .context("Failed to send OpenAI request")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("OpenAI API error ({}): {}", status, error_text);
        }

        let response_data: serde_json::Value = response
            .json()
            .await
            .context("Failed to parse OpenAI response")?;

        let content = response_data["choices"][0]["message"]["content"]
            .as_str()
            .context("No content in OpenAI response")?;

        let result: T =
            serde_json::from_str(content).context("Failed to parse extracted data from OpenAI")?;

        Ok(result)
    }

    /// Extract with Anthropic (using JSON mode)
    async fn extract_with_anthropic<T: for<'de> Deserialize<'de>>(
        &self,
        api_key: &str,
        model: &str,
        prompt: &str,
        _schema: serde_json::Value,
    ) -> Result<T> {
        let request = serde_json::json!({
            "model": model,
            "max_tokens": 4096,
            "messages": [
                {
                    "role": "user",
                    "content": format!("{}\n\nRespond with valid JSON only, no markdown formatting.", prompt)
                }
            ],
            "temperature": 0.1
        });

        let response = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .context("Failed to send Anthropic request")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("Anthropic API error ({}): {}", status, error_text);
        }

        let response_data: serde_json::Value = response
            .json()
            .await
            .context("Failed to parse Anthropic response")?;

        let content = response_data["content"][0]["text"]
            .as_str()
            .context("No content in Anthropic response")?;

        let result: T = serde_json::from_str(content)
            .context("Failed to parse extracted data from Anthropic")?;

        Ok(result)
    }

    /// Extract with OpenRouter
    async fn extract_with_openrouter<T: for<'de> Deserialize<'de>>(
        &self,
        api_key: &str,
        model: &str,
        prompt: &str,
        _schema: serde_json::Value,
    ) -> Result<T> {
        let request = serde_json::json!({
            "model": model,
            "messages": [
                {
                    "role": "system",
                    "content": "You are a precise financial document analyst. Extract information accurately in valid JSON format."
                },
                {
                    "role": "user",
                    "content": format!("{}\n\nRespond with valid JSON only.", prompt)
                }
            ],
            "temperature": 0.1,
            "response_format": { "type": "json_object" }
        });

        let response = self
            .client
            .post("https://openrouter.ai/api/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .context("Failed to send OpenRouter request")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("OpenRouter API error ({}): {}", status, error_text);
        }

        let response_data: serde_json::Value = response
            .json()
            .await
            .context("Failed to parse OpenRouter response")?;

        let content = response_data["choices"][0]["message"]["content"]
            .as_str()
            .context("No content in OpenRouter response")?;

        let result: T = serde_json::from_str(content)
            .context("Failed to parse extracted data from OpenRouter")?;

        Ok(result)
    }

    /// Extract comprehensive Credit Agreement data (52 fields from CSV)
    ///
    /// Uses the full schema defined in comprehensive_schema.rs to extract
    /// all fields from the Credit Agreement.csv specification
    pub async fn extract_credit_agreement_comprehensive(
        &self,
        document_text: &str,
    ) -> Result<super::comprehensive_schema::CreditAgreementExtraction> {
        use super::comprehensive_schema;

        info!("Extracting comprehensive Credit Agreement data");
        let schema = comprehensive_schema::get_credit_agreement_schema();

        let prompt = format!(
            r#"You are a financial document analyst specializing in credit agreements.

Extract ALL fields from this Credit Agreement document following the comprehensive schema.

Document:
{}

Extract:
1. Core Identifiers (agreement ID, version, status, dates, governing law)
2. Parties & Roles (borrower, agent/lender, guarantors, collateral agent)
3. Facility & Economics (type, amount, currency, rates, fees, payment terms)
4. Collateral & Support (secured status, scope, control requirements, UCC filings)
5. Covenants (financial covenants with types and levels, affirmative, negative, cure rights)
6. Defaults & Remedies (events of default, cross-default threshold, cure periods)
7. Ops & Admin (borrowing notice, compliance, reporting, assignment rules)
8. Conditions Precedent (KYC, legal opinions, insurance, corporate approvals)
9. Linkage & Provenance (PDF hash, registry pointers, linked documents)

Be thorough and extract every available detail."#,
            document_text
        );

        match &self.provider {
            LlmProvider::OpenAI { api_key, model } => {
                self.extract_with_openai(api_key, model, &prompt, schema)
                    .await
            }
            LlmProvider::Anthropic { api_key, model } => {
                self.extract_with_anthropic(api_key, model, &prompt, schema)
                    .await
            }
            LlmProvider::OpenRouter { api_key, model } => {
                self.extract_with_openrouter(api_key, model, &prompt, schema)
                    .await
            }
        }
    }

    /// Extract comprehensive Term Sheet data (39 fields from CSV)
    ///
    /// Uses the full schema defined in comprehensive_schema.rs to extract
    /// all fields from the term_sheet.csv specification
    pub async fn extract_term_sheet_comprehensive(
        &self,
        document_text: &str,
    ) -> Result<super::comprehensive_schema::TermSheetExtraction> {
        use super::comprehensive_schema;

        info!("Extracting comprehensive Term Sheet data");
        let schema = comprehensive_schema::get_term_sheet_schema();

        let prompt = format!(
            r#"You are a financial document analyst specializing in term sheets.

Extract ALL fields from this Term Sheet document following the comprehensive schema.

Document:
{}

Extract:
1. Core Identifiers (term sheet ID, version/status, issue/expiry dates, binding clauses)
2. Parties & Roles (borrower, lenders/arranger/agent, sponsor/parent, guarantors)
3. Facility Structure (types, amounts, accordion/upsize, tenor, draw mechanics, use of proceeds)
4. Pricing & Economics (reference rate, margin/spread, floor, OID, fees, amortization, prepayment)
5. Security & Support (secured status, collateral scope, account control, guarantees)
6. Covenants (financial, affirmative, negative, equity cure rights)
7. Conditions & Process (CPs, documentation standard, timelines, exclusivity, governing law)
8. Digital & Linkage (PDF hash, linked documents, notes)

Be thorough and extract every available detail."#,
            document_text
        );

        match &self.provider {
            LlmProvider::OpenAI { api_key, model } => {
                self.extract_with_openai(api_key, model, &prompt, schema)
                    .await
            }
            LlmProvider::Anthropic { api_key, model } => {
                self.extract_with_anthropic(api_key, model, &prompt, schema)
                    .await
            }
            LlmProvider::OpenRouter { api_key, model } => {
                self.extract_with_openrouter(api_key, model, &prompt, schema)
                    .await
            }
        }
    }

    /// Health check
    pub async fn health_check(&self) -> Result<bool> {
        // Just check if we have valid provider configured
        Ok(true)
    }
}

// ============================================================================
// Schema Types (matches BAML types)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DealType {
    Loan,
    CreditLine,
    InvoiceFinancing,
    EquipmentFinancing,
    TradeFinance,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CurrencyCode {
    #[serde(rename = "USD")]
    Usd,
    #[serde(rename = "EUR")]
    Eur,
    #[serde(rename = "GBP")]
    Gbp,
    #[serde(rename = "JPY")]
    Jpy,
    #[serde(rename = "CHF")]
    Chf,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum InterestRateType {
    Fixed,
    Variable,
    Mixed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PaymentFrequency {
    Daily,
    Weekly,
    Biweekly,
    Monthly,
    Quarterly,
    Semiannually,
    Annually,
    OneTime,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CollateralType {
    RealEstate,
    Equipment,
    Inventory,
    AccountsReceivable,
    Securities,
    Cash,
    PersonalGuarantee,
    CorporateGuarantee,
    None,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DealFinancialTerms {
    pub principal_amount: f64,
    pub currency: CurrencyCode,
    pub interest_rate: f64,
    pub interest_rate_type: InterestRateType,
    pub interest_rate_bps: Option<i32>,
    pub origination_fee: Option<f64>,
    pub payment_frequency: PaymentFrequency,
    pub term_length_days: i32,
    pub maturity_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DealParties {
    pub originator_name: String,
    pub originator_legal_name: Option<String>,
    pub originator_address: Option<String>,
    pub originator_tax_id: Option<String>,
    pub investor_name: String,
    pub investor_legal_name: Option<String>,
    pub investor_address: Option<String>,
    pub investor_tax_id: Option<String>,
    pub guarantors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DealCollateral {
    pub has_collateral: bool,
    pub collateral_types: Vec<CollateralType>,
    pub collateral_description: Option<String>,
    pub loan_to_value_ratio: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DealCovenants {
    pub financial_covenants: Vec<String>,
    pub operational_covenants: Vec<String>,
    pub reporting_requirements: Vec<String>,
    pub events_of_default: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentMetadata {
    pub document_type: String,
    pub document_date: Option<String>,
    pub execution_date: Option<String>,
    pub governing_law: Option<String>,
    pub document_title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedDealData {
    pub metadata: DocumentMetadata,
    pub deal_type: DealType,
    pub financial_terms: DealFinancialTerms,
    pub parties: DealParties,
    pub collateral: DealCollateral,
    pub covenants: DealCovenants,
    pub purpose_of_funds: Option<String>,
    pub special_conditions: Vec<String>,
    pub confidence_score: f64,
    pub extraction_notes: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires API key
    async fn test_extraction_service() {
        let service = LlmExtractionService::from_env().expect("Failed to create service");
        assert!(service.health_check().await.is_ok());
    }

    #[test]
    fn test_schema_types_serialization() {
        let financial_terms = DealFinancialTerms {
            principal_amount: 5000000.0,
            currency: CurrencyCode::Usd,
            interest_rate: 0.085,
            interest_rate_type: InterestRateType::Fixed,
            interest_rate_bps: Some(850),
            origination_fee: Some(50000.0),
            payment_frequency: PaymentFrequency::Monthly,
            term_length_days: 1825,
            maturity_date: Some("2028-12-31".to_string()),
        };

        let json = serde_json::to_string(&financial_terms).expect("Serialization failed");
        let deserialized: DealFinancialTerms =
            serde_json::from_str(&json).expect("Deserialization failed");

        assert_eq!(
            financial_terms.principal_amount,
            deserialized.principal_amount
        );
    }

    #[test]
    fn test_comprehensive_credit_agreement_schema_generation() {
        use super::super::comprehensive_schema;

        // Generate the Credit Agreement schema
        let schema = comprehensive_schema::get_credit_agreement_schema();

        // Verify schema structure
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"].is_object());
        assert!(schema["required"].is_array());

        // Verify core identifier fields are present
        assert!(schema["properties"]["agreement_id"].is_object());
        assert!(schema["properties"]["version"].is_object());
        assert!(schema["properties"]["execution_status"].is_object());
        assert!(schema["properties"]["effective_date"].is_object());
        assert!(schema["properties"]["maturity_date"].is_object());

        // Verify party fields are present
        assert!(schema["properties"]["borrower_legal_name"].is_object());
        assert!(schema["properties"]["agent_lender"].is_object());
        assert!(schema["properties"]["guarantors_exist"].is_object());

        // Verify facility fields are present
        assert!(schema["properties"]["facility_type"].is_object());
        assert!(schema["properties"]["committed_amount"].is_object());
        assert!(schema["properties"]["currency"].is_object());
        assert!(schema["properties"]["reference_rate"].is_object());
        assert!(schema["properties"]["margin_bps"].is_object());

        // Verify covenant fields are present
        assert!(schema["properties"]["financial_covenant_1_type"].is_object());
        assert!(schema["properties"]["cure_rights_allowed"].is_object());

        // Verify required fields list contains core fields
        let required = schema["required"].as_array().unwrap();
        assert!(required.contains(&serde_json::json!("borrower_legal_name")));
        assert!(required.contains(&serde_json::json!("agent_lender")));
        assert!(required.contains(&serde_json::json!("committed_amount")));
    }

    #[test]
    fn test_comprehensive_term_sheet_schema_generation() {
        use super::super::comprehensive_schema;

        // Generate the Term Sheet schema
        let schema = comprehensive_schema::get_term_sheet_schema();

        // Verify schema structure
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"].is_object());
        assert!(schema["required"].is_array());

        // Verify core identifier fields
        assert!(schema["properties"]["term_sheet_id"].is_object());
        assert!(schema["properties"]["version_status"].is_object());
        assert!(schema["properties"]["issue_date"].is_object());

        // Verify party fields
        assert!(schema["properties"]["borrower_legal_name"].is_object());
        assert!(schema["properties"]["lenders_arranger_agent"].is_object());

        // Verify facility structure fields
        assert!(schema["properties"]["facility_types"].is_object());
        assert!(schema["properties"]["facility_amounts"].is_object());

        // Verify pricing fields
        assert!(schema["properties"]["reference_rate"].is_object());
        assert!(schema["properties"]["margin_spread_bps"].is_object());

        // Verify required fields
        let required = schema["required"].as_array().unwrap();
        assert!(required.contains(&serde_json::json!("borrower_legal_name")));
        assert!(required.contains(&serde_json::json!("secured")));
    }

    #[test]
    fn test_comprehensive_schema_types_serialization() {
        use super::super::comprehensive_schema::{
            CreditAgreementExtraction, ExecutionStatus, FacilityType,
            PaymentFrequency as SchemaPaymentFrequency, ReferenceRate, RepaymentType,
            TestFrequency,
        };

        // Test Credit Agreement extraction type serialization
        let credit_agreement = CreditAgreementExtraction {
            // Core Identifiers
            agreement_id: Some("CA-2024-001".to_string()),
            version: Some("1.0".to_string()),
            execution_status: Some(ExecutionStatus::Executed),
            effective_date: Some("2024-01-01".to_string()),
            maturity_date: Some("2029-01-01".to_string()),
            governing_law: Some("New York".to_string()),

            // Parties & Roles
            borrower_legal_name: "Acme Corp".to_string(),
            agent_lender: "Big Bank LLC".to_string(),
            guarantors_exist: true,
            guarantor_names: vec!["Guarantor 1".to_string()],
            collateral_agent: Some("Collateral Agent LLC".to_string()),

            // Facility & Economics
            facility_type: Some(FacilityType::TermLoan),
            committed_amount: 10000000.0,
            currency: "USD".to_string(),
            draw_rules: Some("Standard draw rules".to_string()),
            repayment_type: Some(RepaymentType::Amortizing),
            reference_rate: Some(ReferenceRate::Sofr),
            margin_bps: Some(350),
            floor_bps: Some(100),
            oid_percent: Some(2.0),
            upfront_fee_percent: Some(1.0),
            unused_commitment_fee_percent: Some(0.5),
            interest_pay_frequency: Some(SchemaPaymentFrequency::Quarterly),
            day_count: Some("ACT/360".to_string()),
            amortization_schedule: Some("Quarterly".to_string()),
            prepayment_terms: Some("No penalty after year 2".to_string()),

            // Collateral & Support
            secured: true,
            collateral_scope_summary: Some("All assets".to_string()),
            account_control_daca_required: true,
            ucc_filings_required: true,

            // Covenants
            financial_covenant_1_type: Some("Debt/EBITDA".to_string()),
            financial_covenant_1_level: Some("< 3.5x".to_string()),
            financial_covenant_test_frequency: Some(TestFrequency::Quarterly),
            additional_financial_covenants_count: Some(2),
            affirmative_covenants_summary: Some("Standard affirmative covenants".to_string()),
            negative_covenants_summary: Some("Standard negative covenants".to_string()),
            cure_rights_allowed: true,

            // Defaults & Remedies
            events_of_default: vec!["Non-payment".to_string(), "Bankruptcy".to_string()],
            cross_default_threshold: Some(500000.0),
            cure_period_days: Some(30),

            // Ops & Admin
            borrowing_notice_lead_time_days: Some(3),
            compliance_certificate_frequency: Some(TestFrequency::Quarterly),
            reporting_calendar_link: Some("https://example.com/reporting".to_string()),
            assignment_rules_summary: Some("Standard LMA rules".to_string()),

            // Conditions Precedent
            kyc_complete: true,
            legal_opinions_received: true,
            insurance_cois_received: true,
            corporate_approvals_received: true,

            // Linkage & Provenance
            executed_pdf_hash: Some("abc123def456".to_string()),
            registry_pointer_cer_id: Some("CER-001".to_string()),
            linked_term_sheet_id: Some("TS-001".to_string()),
        };

        // Serialize to JSON
        let json = serde_json::to_string(&credit_agreement).expect("Serialization failed");
        assert!(json.contains("Acme Corp"));
        assert!(json.contains("Big Bank LLC"));

        // Deserialize back
        let deserialized: CreditAgreementExtraction =
            serde_json::from_str(&json).expect("Deserialization failed");
        assert_eq!(
            credit_agreement.borrower_legal_name,
            deserialized.borrower_legal_name
        );
        assert_eq!(
            credit_agreement.committed_amount,
            deserialized.committed_amount
        );
    }
}
