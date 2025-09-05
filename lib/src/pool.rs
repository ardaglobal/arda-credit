//! Pool state structures for zkVM execution
//!
//! This module defines the state structures and operations for liquidity pools
//! within the zero-knowledge virtual machine context.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Pool status enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum PoolStatus {
    Pending = 1,
    Active = 2,
    Paused = 3,
    Closed = 4,
}

/// Pool configuration for validation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PoolConfig {
    pub max_ltv_ratio: u64,     // basis points (8000 = 80%)
    pub max_interest_rate: u64, // basis points (1200 = 12%)
    pub repayment_period_days: u32,
}

impl PoolConfig {
    /// Validate pool configuration parameters
    pub fn validate(&self) -> Result<(), PoolValidationError> {
        // LTV ratio should be between 1% and 95% (100-9500 basis points)
        if self.max_ltv_ratio < 100 || self.max_ltv_ratio > 9500 {
            return Err(PoolValidationError::InvalidLtvRatio);
        }

        // Interest rate should be between 0.1% and 50% (10-5000 basis points)
        if self.max_interest_rate < 10 || self.max_interest_rate > 5000 {
            return Err(PoolValidationError::InvalidInterestRate);
        }

        // Repayment period should be between 7 days and 5 years
        if self.repayment_period_days < 7 || self.repayment_period_days > 1825 {
            return Err(PoolValidationError::InvalidRepaymentPeriod);
        }

        Ok(())
    }
}

/// Pool state for zkVM execution
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pool {
    pub pool_id: [u8; 32],
    pub investor_id: [u8; 32],
    pub max_ltv_ratio: u64,     // basis points (8000 = 80%)
    pub max_interest_rate: u64, // basis points (1200 = 12%)
    pub repayment_period_days: u32,
    pub total_deposits: u64,      // in wei equivalent
    pub available_liquidity: u64, // in wei equivalent
    pub utilized_liquidity: u64,  // in wei equivalent
    pub status: PoolStatus,
    pub created_at: u64, // Unix timestamp
}

impl Pool {
    /// Create a new pool from transaction data
    pub fn new(
        pool_id: [u8; 32],
        investor_id: [u8; 32],
        config: PoolConfig,
        created_at: u64,
    ) -> Result<Self, PoolValidationError> {
        // Validate configuration
        config.validate()?;

        Ok(Self {
            pool_id,
            investor_id,
            max_ltv_ratio: config.max_ltv_ratio,
            max_interest_rate: config.max_interest_rate,
            repayment_period_days: config.repayment_period_days,
            total_deposits: 0,
            available_liquidity: 0,
            utilized_liquidity: 0,
            status: PoolStatus::Pending,
            created_at,
        })
    }

    /// Calculate utilization rate in basis points
    #[must_use]
    pub fn utilization_rate_bp(&self) -> u64 {
        if self.total_deposits == 0 {
            0
        } else {
            (self.utilized_liquidity * 10000) / self.total_deposits
        }
    }

    /// Check if pool can accept new loans
    #[must_use]
    pub fn can_accept_loans(&self) -> bool {
        matches!(self.status, PoolStatus::Active) && self.available_liquidity > 0
    }

    /// Add liquidity to the pool
    pub fn add_liquidity(&mut self, amount: u64) -> Result<(), PoolOperationError> {
        if !matches!(self.status, PoolStatus::Active | PoolStatus::Pending) {
            return Err(PoolOperationError::PoolNotActive);
        }

        self.total_deposits = self
            .total_deposits
            .checked_add(amount)
            .ok_or(PoolOperationError::ArithmeticOverflow)?;

        self.available_liquidity = self
            .available_liquidity
            .checked_add(amount)
            .ok_or(PoolOperationError::ArithmeticOverflow)?;

        Ok(())
    }

    /// Utilize liquidity for a loan
    pub fn utilize_liquidity(&mut self, amount: u64) -> Result<(), PoolOperationError> {
        if !self.can_accept_loans() {
            return Err(PoolOperationError::InsufficientLiquidity);
        }

        if amount > self.available_liquidity {
            return Err(PoolOperationError::InsufficientLiquidity);
        }

        self.available_liquidity -= amount;
        self.utilized_liquidity = self
            .utilized_liquidity
            .checked_add(amount)
            .ok_or(PoolOperationError::ArithmeticOverflow)?;

        Ok(())
    }

    /// Return liquidity from loan repayment
    pub fn return_liquidity(&mut self, amount: u64) -> Result<(), PoolOperationError> {
        if amount > self.utilized_liquidity {
            return Err(PoolOperationError::InvalidAmount);
        }

        self.utilized_liquidity -= amount;
        self.available_liquidity = self
            .available_liquidity
            .checked_add(amount)
            .ok_or(PoolOperationError::ArithmeticOverflow)?;

        Ok(())
    }
}

/// Pool state management
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PoolState {
    pub pools: HashMap<[u8; 32], Pool>,
    pub investor_pools: HashMap<[u8; 32], Vec<[u8; 32]>>, // investor_id -> pool_ids
}

impl PoolState {
    /// Create new empty pool state
    #[must_use]
    pub fn new() -> Self {
        Self {
            pools: HashMap::new(),
            investor_pools: HashMap::new(),
        }
    }

    /// Add a new pool
    pub fn add_pool(&mut self, pool: Pool) -> Result<(), PoolOperationError> {
        let pool_id = pool.pool_id;
        let investor_id = pool.investor_id;

        // Check if pool already exists
        if self.pools.contains_key(&pool_id) {
            return Err(PoolOperationError::PoolAlreadyExists);
        }

        // Add pool to main collection
        self.pools.insert(pool_id, pool);

        // Add to investor's pool list
        self.investor_pools
            .entry(investor_id)
            .or_default()
            .push(pool_id);

        Ok(())
    }

    /// Get pool by ID
    #[must_use]
    pub fn get_pool(&self, pool_id: &[u8; 32]) -> Option<&Pool> {
        self.pools.get(pool_id)
    }

    /// Get mutable pool reference
    pub fn get_pool_mut(&mut self, pool_id: &[u8; 32]) -> Option<&mut Pool> {
        self.pools.get_mut(pool_id)
    }

    /// Get all pools for an investor
    #[must_use]
    pub fn get_investor_pools(&self, investor_id: &[u8; 32]) -> Vec<&Pool> {
        self.investor_pools
            .get(investor_id)
            .map(|pool_ids| {
                pool_ids
                    .iter()
                    .filter_map(|pool_id| self.pools.get(pool_id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get total number of pools
    #[must_use]
    pub fn total_pools(&self) -> usize {
        self.pools.len()
    }

    /// Get active pools count
    #[must_use]
    pub fn active_pools(&self) -> usize {
        self.pools
            .values()
            .filter(|pool| matches!(pool.status, PoolStatus::Active))
            .count()
    }

    /// Compute state hash for merkle tree
    #[must_use]
    pub fn compute_hash(&self) -> [u8; 32] {
        use sha2::{Digest, Sha256};

        let mut hasher = Sha256::new();

        // Sort pools by ID for deterministic hashing
        let mut pool_ids: Vec<_> = self.pools.keys().collect();
        pool_ids.sort();

        for pool_id in pool_ids {
            let pool = &self.pools[pool_id];
            hasher.update(pool_id);
            hasher.update(&pool.investor_id);
            hasher.update(&pool.max_ltv_ratio.to_le_bytes());
            hasher.update(&pool.max_interest_rate.to_le_bytes());
            hasher.update(&pool.repayment_period_days.to_le_bytes());
            hasher.update(&pool.total_deposits.to_le_bytes());
            hasher.update(&pool.available_liquidity.to_le_bytes());
            hasher.update(&pool.utilized_liquidity.to_le_bytes());
            hasher.update(&[pool.status as u8]);
            hasher.update(&pool.created_at.to_le_bytes());
        }

        hasher.finalize().into()
    }
}

impl Default for PoolState {
    fn default() -> Self {
        Self::new()
    }
}

/// Pool validation errors
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PoolValidationError {
    InvalidLtvRatio,
    InvalidInterestRate,
    InvalidRepaymentPeriod,
}

/// Pool operation errors
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PoolOperationError {
    PoolNotFound,
    PoolAlreadyExists,
    PoolNotActive,
    InsufficientLiquidity,
    InvalidAmount,
    ArithmeticOverflow,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_config_validation() {
        let valid_config = PoolConfig {
            max_ltv_ratio: 8000,     // 80%
            max_interest_rate: 1200, // 12%
            repayment_period_days: 365,
        };
        assert!(valid_config.validate().is_ok());

        let invalid_ltv_config = PoolConfig {
            max_ltv_ratio: 9600, // 96% - too high
            max_interest_rate: 1200,
            repayment_period_days: 365,
        };
        assert_eq!(
            invalid_ltv_config.validate(),
            Err(PoolValidationError::InvalidLtvRatio)
        );
    }

    #[test]
    fn test_pool_creation() {
        let pool_id = [1u8; 32];
        let investor_id = [2u8; 32];
        let config = PoolConfig {
            max_ltv_ratio: 8000,
            max_interest_rate: 1200,
            repayment_period_days: 365,
        };

        let pool = Pool::new(pool_id, investor_id, config, 1000).unwrap();
        assert_eq!(pool.pool_id, pool_id);
        assert_eq!(pool.investor_id, investor_id);
        assert_eq!(pool.status, PoolStatus::Pending);
    }

    #[test]
    fn test_pool_state_operations() {
        let mut state = PoolState::new();

        let pool_id = [1u8; 32];
        let investor_id = [2u8; 32];
        let config = PoolConfig {
            max_ltv_ratio: 8000,
            max_interest_rate: 1200,
            repayment_period_days: 365,
        };

        let pool = Pool::new(pool_id, investor_id, config, 1000).unwrap();
        assert!(state.add_pool(pool).is_ok());

        assert!(state.get_pool(&pool_id).is_some());
        assert_eq!(state.total_pools(), 1);

        let investor_pools = state.get_investor_pools(&investor_id);
        assert_eq!(investor_pools.len(), 1);
    }
}
