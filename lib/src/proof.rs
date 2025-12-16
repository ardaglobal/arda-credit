//! Unified proof generation and verification module
//!
//! This module provides a shared implementation for Sindri-based proof generation
//! and verification that can be used by both the CLI script and API server.
//! It consolidates the feature-rich logic from the script implementation.

use crate::LoanBatchPublicValues;
use alloy_sol_types::SolType;
use serde::{Deserialize, Serialize};
use sindri::integrations::sp1_v5::SP1ProofInfo;
use sindri::{client::SindriClient, JobStatus, ProofInfoResponse, ProofInput};
use sp1_sdk::{HashableKey, SP1Stdin};
use std::convert::TryInto;
use std::path::PathBuf;
use std::time::Duration;
use thiserror::Error;
use tracing::{error, info, warn};

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
    pub initial_state: crate::GlobalLoanState,
    pub ads_state: crate::ZkVMAds,
    pub batch: crate::DealBatch,
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
    /// Only populated for Sindri proofs, None for local proofs
    pub proof_info: Option<ProofInfoResponse>,
    /// Complete proof data for database storage (self-sufficient architecture)
    /// Contains actual proof bytes, public values, and verifying key
    pub proof_data: Option<SindriProofData>,
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

/// Generate a batch proof using the appropriate prover based on SERVER_ENVIRONMENT
///
/// This function automatically selects between:
/// - Local SP1 prover (for local/staging environments)  
/// - Sindri service (for sandbox/production environments)
///
/// # Errors
///
/// Returns `ProofError` if proof generation fails
pub async fn generate_batch_proof(
    request: BatchProofGenerationRequest,
) -> Result<ProofGenerationResponse, ProofError> {
    // Determine which prover to use based on SERVER_ENVIRONMENT
    let server_env = std::env::var("SERVER_ENVIRONMENT")
        .unwrap_or_else(|_| "local".to_string())
        .to_lowercase();

    match server_env.as_str() {
        "local" | "staging" => {
            info!("🏠 Using local SP1 prover for {} environment", server_env);
            generate_batch_proof_local(request).await
        }
        "sandbox" | "production" => {
            info!("🌐 Using Sindri service for {} environment", server_env);
            generate_batch_proof_sindri(request).await
        }
        _ => {
            warn!(
                "⚠️ Unknown SERVER_ENVIRONMENT '{}', defaulting to local SP1 prover",
                server_env
            );
            generate_batch_proof_local(request).await
        }
    }
}

/// Generate a batch proof using local SP1 prover (for local/staging)
///
/// This function:
/// 1. Creates SP1 stdin with ZkVMBatchInput (full state structures)
/// 2. Uses local ProverClient to generate Core proofs
/// 3. Verifies the proof locally
/// 4. Marks the proof as "Ready" immediately (no async proof generation)
///
/// Note: Local proofs skip Sindri entirely for fast development iteration.
/// The proof is generated and verified immediately, then marked as "Ready".
///
/// # Errors
///
/// Returns `ProofError` if:
/// - SP1 prover setup fails
/// - Proof generation fails
/// - Proof verification fails
async fn generate_batch_proof_local(
    request: BatchProofGenerationRequest,
) -> Result<ProofGenerationResponse, ProofError> {
    use sp1_sdk::{ProverClient, SP1Stdin};

    let transaction_count = request.batch.transactions.len();

    info!(
        "🔐 Generating local SP1 Core proof for batch with {} transactions",
        transaction_count
    );

    // Create prover client
    let client = ProverClient::from_env();

    // Create ZkVMBatchInput structure
    let zkvm_input = crate::ZkVMBatchInput {
        initial_loan_state: request.initial_state,
        initial_ads_state: request.ads_state,
        batch: request.batch,
    };

    // Create inputs
    let mut stdin = SP1Stdin::new();
    stdin.write(&zkvm_input);

    // Get the ELF from the program build
    // Note: Use relative path from lib crate to build directory
    const DEAL_PROGRAM_ELF: &[u8] = include_bytes!("../../build/program");

    // Generate Core proof (fast for local development)
    let (pk, vk) = client.setup(DEAL_PROGRAM_ELF);
    let proof = client
        .prove(&pk, &stdin)
        .core() // Use Core proof mode for speed
        .run()
        .map_err(|e| ProofError::ProofGenerationFailed(e.to_string()))?;

    info!("✅ Local Core proof generated successfully!");

    // Verify the proof
    info!("🔍 Verifying local proof...");
    client
        .verify(&proof, &vk)
        .map_err(|e| ProofError::VerificationFailed(e.to_string()))?;

    info!("✅ Local proof verification passed!");

    // ✅ NEW: Extract proof data for database storage (self-sufficient architecture)
    // NOTE: Core proofs cannot be serialized for on-chain use (only Plonk/Groth16 can).
    // For local development, we use empty proof bytes since Core proofs won't be posted on-chain.
    let proof_bytes = vec![]; // Empty for Core proofs (not meant for on-chain verification)
    let public_values = proof.public_values.as_slice().to_vec();

    // Extract verifying key bytes
    let vkey_hash = vk.hash_u32();
    // Convert [u32; 8] to bytes
    let mut vkey_bytes = Vec::new();
    for word in vkey_hash.iter() {
        vkey_bytes.extend_from_slice(&word.to_be_bytes());
    }

    info!(
        "💾 Extracted proof data: Core proof (0 KB on-chain bytes), {} KB public values, {} bytes vkey",
        public_values.len() / 1024,
        vkey_bytes.len()
    );

    // Create a pseudo proof ID for local proofs
    let proof_id = format!("local-{}", chrono::Utc::now().timestamp());

    // For local proofs, we don't need ProofInfoResponse (it's a Sindri-specific type).
    // We only need the proof data for database storage.
    Ok(ProofGenerationResponse {
        proof_id: proof_id.clone(),
        status: "Ready".to_string(),
        circuit_name: "arda-credit-local".to_string(),
        circuit_tag: "local".to_string(),
        verification_command: format!("Local proof verified (proof_id: {})", proof_id),
        proof_info: None, // No Sindri proof info for local proofs
        // ✅ Include actual proof data for database storage
        proof_data: Some(SindriProofData {
            proof_id: proof_id.clone(),
            proof_bytes,
            public_values,
            verifying_key: vkey_bytes,
        }),
    })
}

/// Generate a batch proof via Sindri service (for sandbox/production)
///
/// # Errors
///
/// Returns `ProofError` if:
/// - Sindri API key is not set in environment
/// - Circuit is not found or not deployed
/// - Proof generation fails on Sindri servers
/// - Network communication errors occur
#[allow(clippy::cognitive_complexity)]
async fn generate_batch_proof_sindri(
    request: BatchProofGenerationRequest,
) -> Result<ProofGenerationResponse, ProofError> {
    let transaction_count = request.batch.transactions.len();

    info!(
        "🔐 Generating {} batch proof via Sindri with {} transactions",
        request.proof_system.to_sindri_scheme().to_uppercase(),
        transaction_count
    );

    // Create ZkVMBatchInput structure
    let zkvm_input = crate::ZkVMBatchInput {
        initial_loan_state: request.initial_state,
        initial_ads_state: request.ads_state,
        batch: request.batch,
    };

    // Create SP1 inputs for batch processing and serialize for Sindri
    info!("📝 Creating SP1 stdin with ZkVMBatchInput");
    let mut stdin = SP1Stdin::new();
    stdin.write(&zkvm_input);
    info!("✅ SP1 stdin created successfully");

    info!("📝 Serializing SP1 stdin to JSON for Sindri");
    let stdin_json = serde_json::to_string(&stdin).map_err(|e| {
        error!("❌ Failed to serialize SP1 stdin: {}", e);
        ProofError::SerializationError(e.to_string())
    })?;
    info!(
        "✅ SP1 stdin serialized, JSON length: {} chars",
        stdin_json.len()
    );

    let proof_input = ProofInput::from(stdin_json.clone());
    info!("✅ ProofInput created from JSON");

    // Get circuit name with configurable tag from environment
    let circuit_tag = std::env::var("SINDRI_CIRCUIT_TAG").unwrap_or_else(|_| "latest".to_string());
    let circuit_name = format!("arda-credit:{circuit_tag}");

    info!("📋 Using circuit: {} (tag: {})", circuit_name, circuit_tag);
    info!("🔑 Checking SINDRI_API_KEY availability...");

    if let Ok(key) = std::env::var("SINDRI_API_KEY") {
        info!("✅ SINDRI_API_KEY is set (length: {} chars)", key.len());
    } else {
        error!("❌ SINDRI_API_KEY is not set in environment!");
        return Err(ProofError::ConfigError(
            "SINDRI_API_KEY not set".to_string(),
        ));
    }

    info!("🌐 Creating Sindri client...");
    let client = SindriClient::default();
    info!("✅ Sindri client created");

    info!(
        "🚀 Submitting proof request to Sindri circuit: {}",
        circuit_name
    );
    info!(
        "📊 Request details: stdin_json preview: {}...",
        &stdin_json[..std::cmp::min(100, stdin_json.len())]
    );

    // Enable benchmark mode to get cycle counts in response
    let benchmark_meta = {
        let mut meta = std::collections::HashMap::new();
        meta.insert("benchmark".to_string(), "true".to_string());
        Some(meta)
    };

    let proof_info = client
        .prove_circuit(&circuit_name, proof_input, benchmark_meta, None, None)
        .await
        .map_err(|e| {
            error!("❌ Sindri API call failed: {}", e);
            ProofError::SindriError(e.to_string())
        })?;

    info!(
        "✅ Sindri API call successful! Proof ID: {}, Status: {:?}",
        proof_info.proof_id, proof_info.status
    );

    // Log cycle counts if available in benchmark mode (check available fields)
    info!("🔍 Available proof info fields: {:?}", proof_info);

    // Note: sp1_info field may not be available in current Sindri client version
    // The benchmark metadata should still enable enhanced monitoring

    let status = match proof_info.status {
        JobStatus::Ready => "Ready".to_string(),
        JobStatus::Failed => "Failed".to_string(),
        _ => "Pending".to_string(),
    };

    let verification_command = format!(
        "cargo run --release -- --verify --proof-id {} (batch with {} transactions)",
        proof_info.proof_id, transaction_count
    );

    info!(
        "✅ {} batch proof submitted successfully - ID: {}",
        request.proof_system.to_sindri_scheme().to_uppercase(),
        proof_info.proof_id
    );

    // ✅ NEW: Wait for proof to be ready and extract proof bytes for database storage
    let proof_data = if proof_info.status == JobStatus::Ready {
        info!("✅ Proof is already ready, extracting data immediately");
        match extract_proof_data_from_sindri(&proof_info) {
            Ok(data) => Some(data),
            Err(e) => {
                warn!("⚠️  Failed to extract proof data from ready proof: {}", e);
                None
            }
        }
    } else {
        info!("⏳ Proof is pending, will poll for completion (timeout: 300s)");
        match wait_for_proof_ready(&proof_info.proof_id, 300).await {
            Ok(ready_proof_info) => {
                info!("✅ Proof is now ready, extracting data");
                match extract_proof_data_from_sindri(&ready_proof_info) {
                    Ok(data) => {
                        info!(
                            "💾 Extracted Sindri proof data: {} KB proof, {} KB public values",
                            data.proof_bytes.len() / 1024,
                            data.public_values.len() / 1024
                        );
                        Some(data)
                    }
                    Err(e) => {
                        warn!("⚠️  Failed to extract proof data: {}", e);
                        None
                    }
                }
            }
            Err(e) => {
                warn!("⚠️  Proof not ready within timeout: {}", e);
                None
            }
        }
    };

    let response = ProofGenerationResponse {
        proof_id: proof_info.proof_id.clone(),
        status,
        circuit_name,
        circuit_tag,
        verification_command,
        proof_info: Some(proof_info), // Wrap in Some for Sindri proofs
        proof_data,                   // ✅ NEW: Include proof data if available
    };

    // Note: EVM fixture generation skipped for new format (would need update)
    if request.generate_fixtures {
        warn!("⚠️  EVM fixture generation not yet implemented for new format");
    }

    Ok(response)
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
    let start_time = std::time::Instant::now();

    let transaction_count = request.batch.transactions.len();

    info!(
        "🔐 Generating {} batch proof via Sindri with {} transactions",
        request.proof_system.to_sindri_scheme().to_uppercase(),
        transaction_count
    );

    // Record Sindri API request
    #[cfg(feature = "sp1")]
    {
        use metrics::{counter, histogram};
        counter!("sindri_api_requests_total", "operation" => "prove_circuit").increment(1);
        histogram!("sindri_proof_transactions_per_batch").record(transaction_count as f64);
    }

    // Create ZkVMBatchInput structure
    let zkvm_input = crate::ZkVMBatchInput {
        initial_loan_state: request.initial_state,
        initial_ads_state: request.ads_state,
        batch: request.batch,
    };

    // Create SP1 inputs for batch processing and serialize for Sindri
    let mut stdin = SP1Stdin::new();
    stdin.write(&zkvm_input);

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
        "cargo run --release -- --verify --proof-id {} (batch with {} transactions)",
        proof_info.proof_id, transaction_count
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
        proof_info: Some(proof_info), // Wrap in Some for Sindri proofs
        proof_data: None,             // Will be populated when proof is retrieved
    };

    // Note: EVM fixture generation skipped for new format (would need update)
    if request.generate_fixtures {
        warn!("⚠️  EVM fixture generation not yet implemented for new format");
    }

    // Record proof generation metrics
    #[cfg(feature = "sp1")]
    {
        use metrics::{counter, histogram};
        let duration = start_time.elapsed();
        histogram!("sindri_proof_generation_duration_seconds").record(duration.as_secs_f64());

        let success = response
            .proof_info
            .as_ref()
            .map(|info| info.status == JobStatus::Ready)
            .unwrap_or(false);
        counter!("sindri_proof_generation_total", 
                "status" => if success { "success" } else { "pending" },
                "proof_system" => request.proof_system.to_sindri_scheme())
        .increment(1);
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
pub async fn get_sindri_proof_info(proof_id: &str) -> Result<ProofInfoResponse, ProofError> {
    let client = SindriClient::default();
    client
        .get_proof(proof_id, None, None, None)
        .await
        .map_err(|e| ProofError::SindriError(e.to_string()))
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

/// Extract proof data from a ProofInfoResponse (helper function)
///
/// # Errors
///
/// Returns `ProofError` if:
/// - Proof extraction fails
/// - Verifying key cannot be decoded
fn extract_proof_data_from_sindri(
    proof_info: &ProofInfoResponse,
) -> Result<SindriProofData, ProofError> {
    // Extract SP1 proof data using Sindri's methods
    let sp1_proof = proof_info
        .to_sp1_proof_with_public()
        .map_err(|e| ProofError::SindriError(format!("Failed to extract SP1 proof: {e}")))?;

    let verifying_key = proof_info
        .get_sp1_verifying_key()
        .map_err(|e| ProofError::SindriError(format!("Failed to extract verification key: {e}")))?;

    // Convert hex string to bytes for verifying key
    let vkey_hex = verifying_key.bytes32();
    let vkey_bytes = hex::decode(vkey_hex.strip_prefix("0x").unwrap_or(&vkey_hex))
        .map_err(|e| ProofError::SindriError(format!("Failed to decode verifying key hex: {e}")))?;

    Ok(SindriProofData {
        proof_id: proof_info.proof_id.clone(),
        proof_bytes: sp1_proof.bytes(),
        public_values: sp1_proof.public_values.as_slice().to_vec(),
        verifying_key: vkey_bytes,
    })
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
pub async fn get_sindri_proof_data(proof_id: &str) -> Result<SindriProofData, ProofError> {
    let proof_info = get_sindri_proof_info(proof_id).await?;

    // Check if proof is ready
    if proof_info.status != JobStatus::Ready {
        return Err(ProofError::ProofNotReady(format!(
            "Proof {} is not ready yet. Status: {:?}",
            proof_id, proof_info.status
        )));
    }

    // Extract proof data using Sindri's methods
    let sp1_proof = proof_info
        .to_sp1_proof_with_public()
        .map_err(|e| ProofError::SindriError(format!("Failed to extract SP1 proof: {e}")))?;

    let verifying_key = proof_info
        .get_sp1_verifying_key()
        .map_err(|e| ProofError::SindriError(format!("Failed to extract verification key: {e}")))?;

    // Convert hex string to bytes for verifying key
    let vkey_hex = verifying_key.bytes32();
    let vkey_bytes = hex::decode(vkey_hex.strip_prefix("0x").unwrap_or(&vkey_hex))
        .map_err(|e| ProofError::SindriError(format!("Failed to decode verifying key hex: {e}")))?;

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
