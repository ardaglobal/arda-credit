/// Hash a UUID to a 32-byte array for privacy
pub fn hash_uuid(uuid: &uuid::Uuid) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(uuid.as_bytes());
    hasher.finalize().into()
}

/// Convert decimal amount to u64 (cents)
pub fn decimal_to_cents(amount: rust_decimal::Decimal) -> u64 {
    use rust_decimal::prelude::ToPrimitive;
    (amount * rust_decimal::Decimal::from(100))
        .to_u64()
        .unwrap_or(0)
}

/// Convert u64 cents back to decimal
pub fn cents_to_decimal(cents: u64) -> rust_decimal::Decimal {
    rust_decimal::Decimal::from(cents) / rust_decimal::Decimal::from(100)
}
