//! Poseidon2 hash implementation for ZK-friendly Merkle trees
//!
//! This module provides Poseidon2 hashing using the BabyBear field from Plonky3.
//! BabyBear is the native field used by SP1 zkVM, ensuring maximum compatibility
//! and efficiency for zero-knowledge proofs.
//!
//! CRITICAL: All hash functions here MUST produce identical results in both:
//! - Off-chain database processing (db/src/ads/)
//! - zkVM execution (program/src/main.rs via lib/src/zkvm_ads.rs)

use p3_baby_bear::{BabyBear, DiffusionMatrixBabyBear};
use p3_field::{AbstractField, PrimeField64};
use p3_poseidon2::{Poseidon2, Poseidon2ExternalMatrixGeneral};
use p3_symmetric::Permutation;
use rand::{rngs::StdRng, SeedableRng};

/// BabyBear field modulus: 2^31 - 2^27 + 1 = 2013265921
const BABYBEAR_MODULUS: u64 = 2013265921;

/// Poseidon2 permutation width (number of field elements in state)
/// Using width 16 for security and efficiency balance
const POSEIDON_WIDTH: usize = 16;

/// Convert an i64 value to a BabyBear field element
///
/// We use the absolute value to ensure non-negative inputs,
/// as BabyBear field elements are always non-negative.
fn to_field(value: i64) -> BabyBear {
    let unsigned = value.unsigned_abs();
    // Reduce modulo BabyBear field modulus
    let reduced = unsigned % BABYBEAR_MODULUS;
    BabyBear::from_canonical_u64(reduced)
}

/// Convert an optional i64 to a BabyBear field element
fn to_field_opt(value: Option<i64>) -> BabyBear {
    match value {
        Some(v) => to_field(v),
        None => BabyBear::zero(),
    }
}

/// Hash a leaf node (nullifier) using Poseidon2
///
/// Inputs:
/// - `value`: The nullifier value
/// - `next_index`: Optional index of next higher nullifier in the linked list
/// - `next_value`: Value of next higher nullifier (0 = max)
///
/// Returns: 32-byte hash digest
pub fn hash_leaf_poseidon(value: i64, next_index: Option<i64>, next_value: i64) -> [u8; 32] {
    // Convert inputs to field elements
    let value_field = to_field(value);
    let next_index_field = to_field_opt(next_index);
    let next_value_field = to_field(next_value);

    // Create input state for Poseidon
    let mut state = [BabyBear::zero(); POSEIDON_WIDTH];
    state[0] = value_field;
    state[1] = next_index_field;
    state[2] = next_value_field;

    // Apply Poseidon2 permutation with correct generic parameters
    // Poseidon2<F, MdsLight, Diffusion, WIDTH, D>
    // Use new_from_rng_128 for automatic round number calculation
    let mut rng = StdRng::seed_from_u64(0); // Deterministic seed for consistency
    let poseidon = Poseidon2::<
        BabyBear,
        Poseidon2ExternalMatrixGeneral,
        DiffusionMatrixBabyBear,
        POSEIDON_WIDTH,
        7,
    >::new_from_rng_128(
        Poseidon2ExternalMatrixGeneral,
        DiffusionMatrixBabyBear,
        &mut rng,
    );
    poseidon.permute_mut(&mut state);

    // Convert first 8 field elements to 32 bytes (4 bytes each)
    // BabyBear elements are 31-bit values, so we use canonical u32 representation
    let mut result = [0u8; 32];
    for (i, &element) in state[..8].iter().enumerate() {
        let value_u32 = element.as_canonical_u64() as u32;
        result[i * 4..(i + 1) * 4].copy_from_slice(&value_u32.to_le_bytes());
    }

    result
}

/// Hash two internal nodes (parent hash) using Poseidon2
///
/// Inputs:
/// - `left`: Left child hash (32 bytes)
/// - `right`: Right child hash (32 bytes)
///
/// Returns: 32-byte parent hash
pub fn hash_internal_poseidon(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    // Convert 32-byte inputs to field elements
    // We'll use 8 field elements from each (4 bytes each)
    let mut state = [BabyBear::zero(); POSEIDON_WIDTH];

    // Load left hash into first 8 positions
    for i in 0..8 {
        let bytes = [
            left[i * 4],
            left[i * 4 + 1],
            left[i * 4 + 2],
            left[i * 4 + 3],
        ];
        let value = u32::from_le_bytes(bytes) as u64;
        state[i] = BabyBear::from_canonical_u64(value % BABYBEAR_MODULUS);
    }

    // Load right hash into last 8 positions
    for i in 0..8 {
        let bytes = [
            right[i * 4],
            right[i * 4 + 1],
            right[i * 4 + 2],
            right[i * 4 + 3],
        ];
        let value = u32::from_le_bytes(bytes) as u64;
        state[8 + i] = BabyBear::from_canonical_u64(value % BABYBEAR_MODULUS);
    }

    // Apply Poseidon2 permutation with correct generic parameters
    let mut rng = StdRng::seed_from_u64(0); // Deterministic seed for consistency
    let poseidon = Poseidon2::<
        BabyBear,
        Poseidon2ExternalMatrixGeneral,
        DiffusionMatrixBabyBear,
        POSEIDON_WIDTH,
        7,
    >::new_from_rng_128(
        Poseidon2ExternalMatrixGeneral,
        DiffusionMatrixBabyBear,
        &mut rng,
    );
    poseidon.permute_mut(&mut state);

    // Convert first 8 field elements back to 32 bytes
    let mut result = [0u8; 32];
    for (i, &element) in state[..8].iter().enumerate() {
        let value_u32 = element.as_canonical_u64() as u32;
        result[i * 4..(i + 1) * 4].copy_from_slice(&value_u32.to_le_bytes());
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_leaf_deterministic() {
        let hash1 = hash_leaf_poseidon(100, Some(200), 300);
        let hash2 = hash_leaf_poseidon(100, Some(200), 300);
        assert_eq!(hash1, hash2, "Leaf hashing should be deterministic");
    }

    #[test]
    fn test_hash_leaf_different_inputs() {
        let hash1 = hash_leaf_poseidon(100, Some(200), 300);
        let hash2 = hash_leaf_poseidon(101, Some(200), 300);
        assert_ne!(
            hash1, hash2,
            "Different values should produce different hashes"
        );
    }

    #[test]
    fn test_hash_leaf_none_index() {
        let hash1 = hash_leaf_poseidon(100, None, 0);
        let hash2 = hash_leaf_poseidon(100, Some(0), 0);
        // None and Some(0) should produce same result (both map to field zero)
        assert_eq!(hash1, hash2, "None and Some(0) should hash identically");
    }

    #[test]
    fn test_hash_internal_deterministic() {
        let left = [1u8; 32];
        let right = [2u8; 32];
        let hash1 = hash_internal_poseidon(&left, &right);
        let hash2 = hash_internal_poseidon(&left, &right);
        assert_eq!(hash1, hash2, "Internal hashing should be deterministic");
    }

    #[test]
    fn test_hash_internal_commutative_fails() {
        let left = [1u8; 32];
        let right = [2u8; 32];
        let hash1 = hash_internal_poseidon(&left, &right);
        let hash2 = hash_internal_poseidon(&right, &left);
        assert_ne!(
            hash1, hash2,
            "Internal hashing should NOT be commutative (order matters)"
        );
    }

    #[test]
    fn test_output_is_32_bytes() {
        let hash = hash_leaf_poseidon(42, Some(100), 200);
        assert_eq!(hash.len(), 32, "Hash output must be exactly 32 bytes");
    }
}
