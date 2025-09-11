//! Unified proof generation and verification module
//!
//! This module provides a shared implementation for Sindri-based proof generation
//! and verification that can be used by both the CLI script and API server.
//! It consolidates the feature-rich logic from the script implementation.

use crate::LoanBatchPublicValues;
use crate::tracing_utils::{
    record_error, record_performance_metrics, record_cost_metrics, record_success,
    sindri_error_context, ErrorCategory, PerformanceMetrics, CostMetrics
};
use alloy_sol_types::SolType;
use serde::{Deserialize, Serialize};
use sindri::integrations::sp1_v5::SP1ProofInfo;
use sindri::{client::SindriClient, JobStatus, ProofInfoResponse, ProofInput};
use sp1_sdk::{HashableKey, SP1Stdin};
use std::convert::TryInto;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use thiserror::Error;
use tracing::{debug, error, info, instrument, warn, Span};

/// Available EVM-compatible proof systems
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "clap", derive(clap::ValueEnum))]
pub enum ProofSystem {
    Plonk,
    #[default]
    Groth16,
}

impl ProofSystem {
    /// Convert to the proving scheme string expected by Sindri
    #[must_use]
    pub const fn to_sindri_scheme(&self) -> &'static str {
        match self {
            Self::Plonk => "plonk",
            Self::Groth16 => "groth16",
        }
    }
}

/// Request for proof generation (batch processing)
#[derive(Debug, Clone)]
pub struct ProofGenerationRequest {
    pub initial_balance: i32,
    pub transactions: Vec<i32>,
    pub proof_system: ProofSystem,
    pub generate_fixtures: bool,
}

/// Alias for backward compatibility
pub type BatchProofGenerationRequest = ProofGenerationRequest;

/// Response from proof generation
#[derive(Debug, Clone)]
pub struct ProofGenerationResponse {
    pub proof_id: String,
    pub status: String,
    pub circuit_name: String,
    pub circuit_tag: String,
    pub verification_command: String,
    pub proof_info: ProofInfoResponse,
}

/// Request for proof verification (loan batch)
#[derive(Debug, Clone)]
pub struct ProofVerificationRequest {
    pub proof_id: String,
    pub expected_transaction_count: u32,
    pub expected_batch_timestamp: u64,
}

/// Response from proof verification (loan batch)
#[derive(Debug, Clone)]
pub struct ProofVerificationResponse {
    pub is_valid: bool,
    pub cryptographic_proof_valid: bool,
    pub batch_details_match_expected: bool,
    pub actual_transaction_count: Option<u32>,
    pub actual_batch_timestamp: Option<u64>,
    pub expected_transaction_count: u32,
    pub expected_batch_timestamp: u64,
    pub verification_message: String,
    pub verification_time_ms: u64,
}

/// Errors that can occur during proof operations
#[derive(Error, Debug)]
pub enum ProofError {
    #[error("Failed to serialize SP1 stdin: {0}")]
    SerializationError(String),

    #[error("Sindri API error: {0}")]
    SindriError(String),

    #[error("Proof generation failed: {0}")]
    ProofGenerationFailed(String),

    #[error("Proof verification failed: {0}")]
    VerificationFailed(String),

    #[error("Failed to decode public values: {0}")]
    PublicValuesDecodeError(String),

    #[error("EVM fixture generation failed: {0}")]
    FixtureGenerationError(String),

    #[error("Environment configuration error: {0}")]
    ConfigError(String),

    #[error("Proof not ready: {0}")]
    ProofNotReady(String),

    #[error("JSON serialization error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// A fixture that can be used to test the verification of SP1 zkVM proofs inside Solidity.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SP1ArithmeticProofFixture {
    pub initial_balance: i32,
    pub final_balance: i32,
    pub vkey: String,
    pub public_values: String,
    pub proof: String,
}

/// Generate a batch proof via Sindri with full feature support
///
/// # Errors
///
/// Returns `ProofError` if:
/// - Sindri API key is not set in environment
/// - Circuit is not found or not deployed
/// - Proof generation fails on Sindri servers
/// - Network communication errors occur
#[allow(clippy::cognitive_complexity)]
#[instrument(skip(request), fields(
    sindri.proof_system = ?request.proof_system,
    sindri.transaction_count = %request.transactions.len(),
    sindri.initial_balance = %request.initial_balance,
    sindri.final_balance = %(request.initial_balance + request.transactions.iter().sum::<i32>()),
    sindri.api_endpoint = "prove_circuit",
    sindri.request_size_bytes = %std::mem::size_of_val(&request),
    layer = "proof_generation"
), level = "info")]
pub async fn generate_batch_proof(
    request: BatchProofGenerationRequest,
) -> Result<ProofGenerationResponse, ProofError> {
    let start_time = Instant::now();
    let final_balance = request.initial_balance + request.transactions.iter().sum::<i32>();

    // Record request details in span
    Span::current().record("sindri.final_balance", final_balance);

    info!(
        "🔐 Generating {} batch proof: {} + {:?} = {} via Sindri",
        request.proof_system.to_sindri_scheme().to_uppercase(),
        request.initial_balance,
        request.transactions,
        final_balance
    );

    // Create SP1 inputs for batch processing and serialize for Sindri
    let serialization_start = Instant::now();
    debug!("📝 Creating SP1 stdin with initial_balance={} and transactions={:?}",
        request.initial_balance, request.transactions);
    
    let mut stdin = SP1Stdin::new();
    stdin.write(&request.initial_balance);
    stdin.write(&request.transactions);

    let stdin_json = serde_json::to_string(&stdin).map_err(|e| {
        let error_context = sindri_error_context(
            "generate_batch_proof",
            "serialization",
            None,
            None,
            None,
        );
        record_error(error_context, &format!("Failed to serialize SP1 stdin: {}", e));
        ProofError::SerializationError(e.to_string())
    })?;
    
    let serialization_time = serialization_start.elapsed().as_millis();
    Span::current().record("sindri.serialization_time_ms", serialization_time);
    Span::current().record("sindri.request_size_bytes", stdin_json.len());

    debug!("✅ SP1 stdin serialized in {}ms, JSON length: {} bytes", 
           serialization_time, stdin_json.len());

    let proof_input = ProofInput::from(stdin_json.clone());

    // Get circuit name with configurable tag from environment
    let circuit_tag = std::env::var("SINDRI_CIRCUIT_TAG").unwrap_or_else(|_| "latest".to_string());
    let circuit_name = format!("arda-credit:{circuit_tag}");
    
    Span::current().record("sindri.circuit", &circuit_name);
    Span::current().record("sindri.circuit_tag", &circuit_tag);

    debug!("📋 Using circuit: {} (tag: {})", circuit_name, circuit_tag);

    // Validate API key availability
    if std::env::var("SINDRI_API_KEY").is_err() {
        let error_context = sindri_error_context(
            "generate_batch_proof",
            "configuration",
            None,
            None,
            None,
        );
        record_error(error_context, "SINDRI_API_KEY not set in environment");
        return Err(ProofError::ConfigError("SINDRI_API_KEY not set".to_string()));
    }

    // Submit proof request to Sindri with timing
    let client = SindriClient::default();
    let api_call_start = Instant::now();
    
    info!("🚀 Submitting proof request to Sindri circuit: {}", circuit_name);
    
    let proof_info = client
        .prove_circuit(&circuit_name, proof_input, None, None, None)
        .await
        .map_err(|e| {
            let api_call_time = api_call_start.elapsed().as_millis();
            let error_context = sindri_error_context(
                "generate_batch_proof",
                "api_call",
                None,
                None,
                None,
            );
            record_error(error_context, &format!("Sindri API call failed after {}ms: {}", api_call_time, e));
            Span::current().record("sindri.api_call_time_ms", api_call_time);
            Span::current().record("sindri.success", false);
            ProofError::SindriError(e.to_string())
        })?;

    let api_call_time = api_call_start.elapsed().as_millis();
    Span::current().record("sindri.api_call_time_ms", api_call_time);
    Span::current().record("sindri.proof_id", &proof_info.proof_id);
    Span::current().record("sindri.status", format!("{:?}", proof_info.status));
    Span::current().record("sindri.success", true);

    info!("✅ Sindri API call successful in {}ms! Proof ID: {}, Status: {:?}",
          api_call_time, proof_info.proof_id, proof_info.status);

    let status = match proof_info.status {
        JobStatus::Ready => "Ready".to_string(),
        JobStatus::Failed => "Failed".to_string(),
        _ => "Pending".to_string(),
    };

    let verification_command = format!(
        "cargo run --release -- --verify --proof-id {} --initial-balance {} --transactions {:?}",
        proof_info.proof_id, request.initial_balance, request.transactions
    );

    // Record final timing and cost metrics
    let total_time = start_time.elapsed().as_millis();
    Span::current().record("sindri.total_time_ms", total_time);
    
    // Estimate cost based on transaction count and proof system
    let estimated_cost_usd = estimate_sindri_cost(
        request.transactions.len(),
        request.proof_system,
        &status
    );
    Span::current().record("sindri.estimated_cost_usd", estimated_cost_usd);

    // Record success and cost metrics
    record_success(
        "generate_batch_proof",
        total_time,
        Some(&format!("Proof ID: {}", proof_info.proof_id)),
    );
    
    record_cost_metrics(CostMetrics {
        operation: "generate_batch_proof".to_string(),
        estimated_cost_usd,
        actual_cost_usd: None, // Will be updated when we get actual billing
        cost_category: "sindri".to_string(),
        resource_usage: Some(serde_json::json!({
            "transaction_count": request.transactions.len(),
            "proof_system": request.proof_system.to_sindri_scheme(),
            "circuit": circuit_name
        })),
    });
    
    record_performance_metrics(PerformanceMetrics {
        operation: "generate_batch_proof".to_string(),
        duration_ms: total_time,
        throughput_ops_per_sec: Some(request.transactions.len() as f64 * 1000.0 / total_time as f64),
        memory_usage_mb: None,
        success: true,
        error_category: None,
    });

    info!(
        "✅ {} batch proof submitted successfully in {}ms - ID: {}, Estimated Cost: ${:.4}",
        request.proof_system.to_sindri_scheme().to_uppercase(),
        total_time,
        proof_info.proof_id,
        estimated_cost_usd
    );

    let response = ProofGenerationResponse {
        proof_id: proof_info.proof_id.clone(),
        status,
        circuit_name,
        circuit_tag,
        verification_command,
        proof_info,
    };

    // Generate EVM fixture if requested
    if request.generate_fixtures {
        let fixture_start = Instant::now();
        if let Err(e) = create_batch_evm_fixture(
            &response.proof_info,
            request.initial_balance,
            &request.transactions,
            final_balance,
            request.proof_system,
        )
        .await
        {
            warn!("⚠️  Failed to generate EVM fixture: {}", e);
            Span::current().record("fixture.generation_failed", true);
        } else {
            let fixture_time = fixture_start.elapsed().as_millis();
            Span::current().record("fixture.generation_time_ms", fixture_time);
            debug!("✅ EVM fixture generated in {}ms", fixture_time);
        }
    }

    Ok(response)
}

/// Estimate Sindri cost based on transaction count, proof system, and status
/// 
/// This provides rough cost estimates for observability purposes.
/// Actual costs may vary based on Sindri's pricing model.
fn estimate_sindri_cost(transaction_count: usize, proof_system: ProofSystem, status: &str) -> f64 {
    // Base cost estimates (these are rough estimates for observability)
    let base_cost = match proof_system {
        ProofSystem::Groth16 => 0.10, // $0.10 base for Groth16
        ProofSystem::Plonk => 0.15,   // $0.15 base for Plonk
    };
    
    // Transaction complexity multiplier
    let complexity_multiplier = 1.0 + (transaction_count as f64 * 0.01);
    
    // Status-based adjustment
    let status_multiplier = match status {
        "Ready" => 1.0,     // Full cost for successful proofs
        "Failed" => 0.1,    // Reduced cost for failed proofs
        _ => 0.5,           // Partial cost for pending proofs
    };
    
    base_cost * complexity_multiplier * status_multiplier
}

/// Generate a proof via Sindri with full feature support
///
/// # Errors
///
/// Returns `ProofError` if:
/// - Sindri API key is not set in environment
/// - Circuit is not found or not deployed
/// - Proof generation fails on Sindri servers
/// - Network communication errors occur
#[allow(clippy::cognitive_complexity)]
pub async fn generate_sindri_proof(
    request: ProofGenerationRequest,
) -> Result<ProofGenerationResponse, ProofError> {
    let final_balance = request.initial_balance + request.transactions.iter().sum::<i32>();

    info!(
        "🔐 Generating {} batch proof: {} + {:?} = {} via Sindri",
        request.proof_system.to_sindri_scheme().to_uppercase(),
        request.initial_balance,
        request.transactions,
        final_balance
    );

    // Create SP1 inputs for batch processing and serialize for Sindri
    let mut stdin = SP1Stdin::new();
    stdin.write(&request.initial_balance);
    stdin.write(&request.transactions);

    let stdin_json =
        serde_json::to_string(&stdin).map_err(|e| ProofError::SerializationError(e.to_string()))?;
    let proof_input = ProofInput::from(stdin_json);

    // Get circuit name with configurable tag from environment
    let circuit_tag = std::env::var("SINDRI_CIRCUIT_TAG").unwrap_or_else(|_| "latest".to_string());
    let circuit_name = format!("arda-credit:{circuit_tag}");

    info!("📋 Using circuit: {} (tag: {})", circuit_name, circuit_tag);

    let client = SindriClient::default();

    let proof_info = client
        .prove_circuit(&circuit_name, proof_input, None, None, None)
        .await
        .map_err(|e| ProofError::SindriError(e.to_string()))?;

    if proof_info.status == JobStatus::Failed {
        return Err(ProofError::ProofGenerationFailed(format!(
            "Sindri proof generation failed: {:?}",
            proof_info.error
        )));
    }

    let status = match proof_info.status {
        JobStatus::Ready => "Ready".to_string(),
        JobStatus::Failed => "Failed".to_string(),
        _ => "Pending".to_string(),
    };

    let verification_command = format!(
        "cargo run --release -- --verify --proof-id {} --initial-balance {} --transactions {:?}",
        proof_info.proof_id, request.initial_balance, request.transactions
    );

    info!(
        "✅ {} proof submitted successfully - ID: {}",
        request.proof_system.to_sindri_scheme().to_uppercase(),
        proof_info.proof_id
    );

    let response = ProofGenerationResponse {
        proof_id: proof_info.proof_id.clone(),
        status,
        circuit_name,
        circuit_tag,
        verification_command,
        proof_info,
    };

    // Generate EVM fixture if requested
    if request.generate_fixtures {
        if let Err(e) = create_evm_fixture(
            &response.proof_info,
            request.initial_balance,
            &request.transactions,
            final_balance,
            request.proof_system,
        )
        .await
        {
            warn!("⚠️  Failed to generate EVM fixture: {}", e);
        }
    }

    Ok(response)
}

/// Verify a proof via Sindri with comprehensive validation
///
/// # Errors
///
/// Returns `ProofError` if:
/// - Proof is not found on Sindri
/// - Proof verification fails
/// - Network communication errors occur
/// - SP1 proof extraction fails
pub async fn verify_sindri_proof(
    request: ProofVerificationRequest,
) -> Result<ProofVerificationResponse, ProofError> {
    let start_time = std::time::Instant::now();

    info!("🔍 Verifying proof ID: {}", request.proof_id);

    let client = SindriClient::default();

    let proof_info = client
        .get_proof(&request.proof_id, None, None, None)
        .await
        .map_err(|e| ProofError::SindriError(e.to_string()))?;

    if proof_info.status != JobStatus::Ready {
        return Ok(ProofVerificationResponse {
            is_valid: false,
            cryptographic_proof_valid: false,
            batch_details_match_expected: false,
            actual_transaction_count: None,
            actual_batch_timestamp: None,
            expected_transaction_count: request.expected_transaction_count,
            expected_batch_timestamp: request.expected_batch_timestamp,
            verification_message: format!("Proof not ready. Status: {:?}", proof_info.status),
            verification_time_ms: start_time
                .elapsed()
                .as_millis()
                .try_into()
                .unwrap_or(u64::MAX),
        });
    }

    // Extract SP1 proof and verification key from Sindri response
    let sp1_proof = proof_info
        .to_sp1_proof_with_public()
        .map_err(|e| ProofError::VerificationFailed(format!("Failed to extract SP1 proof: {e}")))?;

    let sindri_verifying_key = proof_info.get_sp1_verifying_key().map_err(|e| {
        ProofError::VerificationFailed(format!("Failed to extract verification key: {e}"))
    })?;

    // Perform local verification using Sindri's verification key
    let cryptographic_proof_valid = proof_info
        .verify_sp1_proof_locally(&sindri_verifying_key)
        .is_ok();

    if !cryptographic_proof_valid {
        return Ok(ProofVerificationResponse {
            is_valid: false,
            cryptographic_proof_valid: false,
            batch_details_match_expected: false,
            actual_transaction_count: None,
            actual_batch_timestamp: None,
            expected_transaction_count: request.expected_transaction_count,
            expected_batch_timestamp: request.expected_batch_timestamp,
            verification_message: "Cryptographic proof verification failed".to_string(),
            verification_time_ms: start_time
                .elapsed()
                .as_millis()
                .try_into()
                .unwrap_or(u64::MAX),
        });
    }

    // Verification successful - now validate the loan batch details
    let decoded = LoanBatchPublicValues::abi_decode(sp1_proof.public_values.as_slice())
        .map_err(|e| ProofError::PublicValuesDecodeError(e.to_string()))?;

    let actual_transaction_count = decoded.transaction_count;
    let actual_batch_timestamp = decoded.batch_timestamp;
    let batch_details_match_expected = actual_transaction_count
        == request.expected_transaction_count
        && actual_batch_timestamp == request.expected_batch_timestamp;
    let is_valid = cryptographic_proof_valid && batch_details_match_expected;

    let verification_message = if is_valid {
        format!("✅ LOAN BATCH PROCESSING VERIFIED: {actual_transaction_count} transactions processed at timestamp {actual_batch_timestamp} (cryptographically verified)")
    } else {
        format!(
            "❌ Loan batch verification failed: Expected {} txs @ {}, got {} txs @ {}",
            request.expected_transaction_count,
            request.expected_batch_timestamp,
            actual_transaction_count,
            actual_batch_timestamp
        )
    };

    info!("{}", verification_message);

    Ok(ProofVerificationResponse {
        is_valid,
        cryptographic_proof_valid,
        batch_details_match_expected,
        actual_transaction_count: Some(actual_transaction_count),
        actual_batch_timestamp: Some(actual_batch_timestamp),
        expected_transaction_count: request.expected_transaction_count,
        expected_batch_timestamp: request.expected_batch_timestamp,
        verification_message,
        verification_time_ms: start_time
            .elapsed()
            .as_millis()
            .try_into()
            .unwrap_or(u64::MAX),
    })
}

/// Create EVM-compatible fixture from Sindri proof for Solidity testing
#[allow(clippy::cognitive_complexity)]
async fn create_evm_fixture(
    proof_info: &ProofInfoResponse,
    initial_balance: i32,
    _transactions: &[i32],
    final_balance: i32,
    system: ProofSystem,
) -> Result<(), ProofError> {
    const MAX_ATTEMPTS: u32 = 60; // 5 minutes with 5-second intervals

    info!(
        "🔧 Generating EVM fixture for {} proof...",
        system.to_sindri_scheme().to_uppercase()
    );

    // Wait for proof to be ready if it's still processing
    let client = SindriClient::default();
    let mut current_proof = proof_info.clone();

    // Poll until proof is ready (with timeout)
    let mut attempts = 0;

    while current_proof.status != JobStatus::Ready && attempts < MAX_ATTEMPTS {
        if current_proof.status == JobStatus::Failed {
            return Err(ProofError::FixtureGenerationError(format!(
                "Sindri proof generation failed: {:?}",
                current_proof.error
            )));
        }

        info!(
            "⏳ Waiting for proof to be ready... (attempt {}/{})",
            attempts + 1,
            MAX_ATTEMPTS
        );
        tokio::time::sleep(Duration::from_secs(5)).await;

        current_proof = client
            .get_proof(&proof_info.proof_id, None, None, Some(true))
            .await
            .map_err(|e| ProofError::SindriError(e.to_string()))?;
        attempts += 1;
    }

    if current_proof.status != JobStatus::Ready {
        return Err(ProofError::FixtureGenerationError(
            "Timeout waiting for Sindri proof to be ready".to_string(),
        ));
    }

    info!("✅ Sindri proof is ready, extracting EVM-compatible data...");

    // Extract SP1 proof data from Sindri response
    let sp1_proof = current_proof
        .to_sp1_proof_with_public()
        .map_err(|e| ProofError::FixtureGenerationError(e.to_string()))?;
    let verification_key = current_proof
        .get_sp1_verifying_key()
        .map_err(|e| ProofError::FixtureGenerationError(e.to_string()))?;

    // Create the fixture using batch processing format
    let fixture = SP1ArithmeticProofFixture {
        initial_balance,
        final_balance,
        vkey: verification_key.bytes32(),
        public_values: format!("0x{}", hex::encode(sp1_proof.public_values.as_slice())),
        proof: format!("0x{}", hex::encode(sp1_proof.bytes())),
    };

    // Create fixtures directory and save the fixture
    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or_else(|| {
            ProofError::FixtureGenerationError("Failed to get parent directory".to_string())
        })?
        .join("contracts/src/fixtures");

    std::fs::create_dir_all(&fixture_path).map_err(|e| {
        ProofError::FixtureGenerationError(format!("Failed to create fixtures directory: {e}"))
    })?;

    let filename = format!("{}-fixture.json", system.to_sindri_scheme());
    let fixture_file = fixture_path.join(&filename);

    std::fs::write(&fixture_file, serde_json::to_string_pretty(&fixture)?).map_err(|e| {
        ProofError::FixtureGenerationError(format!("Failed to write fixture file: {e}"))
    })?;

    info!("✅ EVM fixture saved to: {}", fixture_file.display());
    info!("🔑 Verification Key: {}", fixture.vkey);
    info!("📊 Public Values: {}", fixture.public_values);
    info!(
        "🔒 Proof Bytes: {}...{}",
        &fixture.proof[..42],
        &fixture.proof[fixture.proof.len() - 6..]
    );

    Ok(())
}

/// Get proof information from Sindri
///
/// # Errors
///
/// Returns `ProofError` if:
/// - Proof is not found on Sindri
/// - Network communication errors occur
#[instrument(skip(proof_id), fields(
    sindri.proof_id = %proof_id,
    sindri.api_endpoint = "get_proof",
    layer = "proof_status_check"
), level = "info")]
pub async fn get_sindri_proof_info(proof_id: &str) -> Result<ProofInfoResponse, ProofError> {
    let start_time = Instant::now();
    let client = SindriClient::default();
    
    debug!("🔍 Fetching proof info for ID: {}", proof_id);
    
    let result = client
        .get_proof(proof_id, None, None, None)
        .await
        .map_err(|e| {
            let api_time = start_time.elapsed().as_millis();
            error!("❌ Failed to get proof info after {}ms: {}", api_time, e);
            Span::current().record("sindri.api_call_time_ms", api_time);
            Span::current().record("error.type", "sindri_api_error");
            Span::current().record("error.message", e.to_string());
            ProofError::SindriError(e.to_string())
        })?;
    
    let api_time = start_time.elapsed().as_millis();
    Span::current().record("sindri.api_call_time_ms", api_time);
    Span::current().record("sindri.status", format!("{:?}", result.status));
    
    debug!("✅ Retrieved proof info in {}ms, status: {:?}", api_time, result.status);
    
    Ok(result)
}

/// Check if a proof is ready on Sindri
///
/// # Errors
///
/// Returns `ProofError` if:
/// - Proof is not found on Sindri
/// - Network communication errors occur
pub async fn is_proof_ready(proof_id: &str) -> Result<bool, ProofError> {
    let proof_info = get_sindri_proof_info(proof_id).await?;
    Ok(proof_info.status == JobStatus::Ready)
}

/// Retrieve actual proof data from Sindri for smart contract submission
///
/// # Errors
///
/// Returns `ProofError` if:
/// - Proof is not found on Sindri
/// - Proof is not ready yet
/// - Network communication errors occur
/// - Proof data is malformed
#[instrument(skip(proof_id), fields(
    sindri.proof_id = %proof_id,
    sindri.api_endpoint = "get_proof_data",
    layer = "proof_data_retrieval"
), level = "info")]
pub async fn get_sindri_proof_data(proof_id: &str) -> Result<SindriProofData, ProofError> {
    let start_time = Instant::now();
    
    info!("📥 Retrieving proof data for ID: {}", proof_id);
    
    let proof_info = get_sindri_proof_info(proof_id).await?;

    // Check if proof is ready
    if proof_info.status != JobStatus::Ready {
        Span::current().record("error.type", "proof_not_ready");
        Span::current().record("sindri.status", format!("{:?}", proof_info.status));
        return Err(ProofError::ProofNotReady(format!(
            "Proof {} is not ready yet. Status: {:?}",
            proof_id, proof_info.status
        )));
    }

    // Extract proof data using Sindri's methods
    let extraction_start = Instant::now();
    
    let sp1_proof = proof_info
        .to_sp1_proof_with_public()
        .map_err(|e| {
            Span::current().record("error.type", "proof_extraction_error");
            Span::current().record("error.message", e.to_string());
            ProofError::SindriError(format!("Failed to extract SP1 proof: {e}"))
        })?;

    let verifying_key = proof_info
        .get_sp1_verifying_key()
        .map_err(|e| {
            Span::current().record("error.type", "vkey_extraction_error");
            Span::current().record("error.message", e.to_string());
            ProofError::SindriError(format!("Failed to extract verification key: {e}"))
        })?;

    // Convert hex string to bytes for verifying key
    let vkey_hex = verifying_key.bytes32();
    let vkey_bytes = hex::decode(vkey_hex.strip_prefix("0x").unwrap_or(&vkey_hex))
        .map_err(|e| {
            Span::current().record("error.type", "vkey_decode_error");
            Span::current().record("error.message", e.to_string());
            ProofError::SindriError(format!("Failed to decode verifying key hex: {e}"))
        })?;

    let extraction_time = extraction_start.elapsed().as_millis();
    let total_time = start_time.elapsed().as_millis();
    
    // Record metrics
    Span::current().record("sindri.extraction_time_ms", extraction_time);
    Span::current().record("sindri.total_time_ms", total_time);
    Span::current().record("sindri.proof_size_bytes", sp1_proof.bytes().len());
    Span::current().record("sindri.public_values_size_bytes", sp1_proof.public_values.len());
    Span::current().record("sindri.verifying_key_size_bytes", vkey_bytes.len());

    info!("✅ Retrieved proof data in {}ms: proof_size={}B, public_values={}B, vkey={}B",
          total_time, sp1_proof.bytes().len(), sp1_proof.public_values.len(), vkey_bytes.len());

    Ok(SindriProofData {
        proof_id: proof_id.to_string(),
        proof_bytes: sp1_proof.bytes(),
        public_values: sp1_proof.public_values.as_slice().to_vec(),
        verifying_key: vkey_bytes,
    })
}

/// Structured proof data retrieved from Sindri
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SindriProofData {
    pub proof_id: String,
    pub proof_bytes: Vec<u8>,
    pub public_values: Vec<u8>,
    pub verifying_key: Vec<u8>,
}

/// Create EVM-compatible fixture from Sindri proof for batch processing
#[allow(clippy::cognitive_complexity)]
async fn create_batch_evm_fixture(
    proof_info: &ProofInfoResponse,
    initial_balance: i32,
    _transactions: &[i32],
    final_balance: i32,
    system: ProofSystem,
) -> Result<(), ProofError> {
    const MAX_ATTEMPTS: u32 = 60; // 5 minutes with 5-second intervals
    info!(
        "🔧 Generating batch EVM fixture for {} proof...",
        system.to_sindri_scheme().to_uppercase()
    );

    // Wait for proof to be ready if it's still processing
    let client = SindriClient::default();
    let mut current_proof = proof_info.clone();

    // Poll until proof is ready (with timeout)
    let mut attempts = 0;

    while current_proof.status != JobStatus::Ready && attempts < MAX_ATTEMPTS {
        if current_proof.status == JobStatus::Failed {
            return Err(ProofError::FixtureGenerationError(format!(
                "Sindri proof generation failed: {:?}",
                current_proof.error
            )));
        }

        info!(
            "⏳ Waiting for proof to be ready... (attempt {}/{})",
            attempts + 1,
            MAX_ATTEMPTS
        );
        tokio::time::sleep(Duration::from_secs(5)).await;

        current_proof = client
            .get_proof(&proof_info.proof_id, None, None, Some(true))
            .await
            .map_err(|e| ProofError::SindriError(e.to_string()))?;
        attempts += 1;
    }

    if current_proof.status != JobStatus::Ready {
        return Err(ProofError::FixtureGenerationError(
            "Timeout waiting for Sindri proof to be ready".to_string(),
        ));
    }

    info!("✅ Sindri proof is ready, extracting EVM-compatible data...");

    // Extract SP1 proof data from Sindri response
    let sp1_proof = current_proof
        .to_sp1_proof_with_public()
        .map_err(|e| ProofError::FixtureGenerationError(e.to_string()))?;
    let verification_key = current_proof
        .get_sp1_verifying_key()
        .map_err(|e| ProofError::FixtureGenerationError(e.to_string()))?;

    // Create the batch fixture using the unified format
    let fixture = SP1ArithmeticProofFixture {
        initial_balance,
        final_balance,
        vkey: verification_key.bytes32(),
        public_values: format!("0x{}", hex::encode(sp1_proof.public_values.as_slice())),
        proof: format!("0x{}", hex::encode(sp1_proof.bytes())),
    };

    // Create fixtures directory and save the fixture
    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or_else(|| {
            ProofError::FixtureGenerationError("Failed to get parent directory".to_string())
        })?
        .join("contracts/src/fixtures");

    std::fs::create_dir_all(&fixture_path).map_err(|e| {
        ProofError::FixtureGenerationError(format!("Failed to create fixtures directory: {e}"))
    })?;

    let filename = format!("batch-{}-fixture.json", system.to_sindri_scheme());
    let fixture_file = fixture_path.join(&filename);

    std::fs::write(&fixture_file, serde_json::to_string_pretty(&fixture)?).map_err(|e| {
        ProofError::FixtureGenerationError(format!("Failed to write fixture file: {e}"))
    })?;

    info!("✅ Batch EVM fixture saved to: {}", fixture_file.display());
    info!("🔑 Verification Key: {}", fixture.vkey);
    info!("📊 Public Values: {}", fixture.public_values);
    info!(
        "🔒 Proof Bytes: {}...{}",
        &fixture.proof[..42],
        &fixture.proof[fixture.proof.len() - 6..]
    );

    Ok(())
}

/// Wait for a proof to be ready with timeout
///
/// # Errors
///
/// Returns `ProofError` if:
/// - Proof times out before becoming ready
/// - Proof fails during generation
/// - Network communication errors occur
pub async fn wait_for_proof_ready(
    proof_id: &str,
    timeout_seconds: u64,
) -> Result<ProofInfoResponse, ProofError> {
    let client = SindriClient::default();
    let mut attempts = 0;
    let max_attempts = timeout_seconds / 5; // Check every 5 seconds

    loop {
        let proof_info = client
            .get_proof(proof_id, None, None, None)
            .await
            .map_err(|e| ProofError::SindriError(e.to_string()))?;

        match proof_info.status {
            JobStatus::Ready => return Ok(proof_info),
            JobStatus::Failed => {
                return Err(ProofError::ProofGenerationFailed(format!(
                    "Proof generation failed: {:?}",
                    proof_info.error
                )));
            }
            _ => {
                if attempts >= max_attempts {
                    return Err(ProofError::ProofGenerationFailed(format!(
                        "Timeout waiting for proof to be ready after {timeout_seconds} seconds"
                    )));
                }
                tokio::time::sleep(Duration::from_secs(5)).await;
                attempts += 1;
            }
        }
    }
}
