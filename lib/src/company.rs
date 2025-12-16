//! Company management logic for zkVM execution
//!
//! This module contains company-related types and state transition logic.
//! Companies are the primary entities that participate in deals (as originators or investors).
//! Users act on behalf of companies, but companies hold the balances and participate in deals.

use crate::transactions::{LoanTransaction, TransactionType};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// Errors that can occur during company operations
#[derive(Error, Debug, Clone)]
pub enum CompanyError {
    #[error("Company already exists: {company_id:?}")]
    CompanyAlreadyExists { company_id: [u8; 32] },

    #[error("Company not found: {company_id:?}")]
    CompanyNotFound { company_id: [u8; 32] },

    #[error("Invalid company data")]
    InvalidCompanyData,

    #[error("Invalid transaction type for company operation")]
    InvalidTransactionType,
}

/// Company status enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum CompanyStatus {
    Active = 0,
    Suspended = 1,
    Inactive = 2,
}

impl CompanyStatus {
    /// Create CompanyStatus from u8
    #[must_use]
    pub const fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::Active,
            1 => Self::Suspended,
            2 => Self::Inactive,
            _ => Self::Active, // Default to Active for unknown values
        }
    }
}

/// Company entity for zkVM state
///
/// Represents a company that can participate in deals as originator or investor.
/// Companies have balances and can transact with other companies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Company {
    /// Hashed company UUID (32 bytes)
    pub company_id: [u8; 32],

    /// When company was created (Unix timestamp)
    pub created_at: u64,

    /// Company status (Active, Suspended, Inactive)
    pub status: CompanyStatus,
}

impl Company {
    /// Create a new company
    #[must_use]
    pub const fn new(company_id: [u8; 32], created_at: u64, status: CompanyStatus) -> Self {
        Self {
            company_id,
            created_at,
            status,
        }
    }

    /// Check if company is active
    #[must_use]
    pub const fn is_active(&self) -> bool {
        matches!(self.status, CompanyStatus::Active)
    }

    /// Update company status
    pub fn update_status(&mut self, status: CompanyStatus) {
        self.status = status;
    }
}

/// Company state management for zkVM
///
/// Manages all companies in the system and provides operations for
/// company registration, updates, and lookups.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyState {
    /// All companies in the system (company_id → Company)
    pub companies: HashMap<[u8; 32], Company>,
}

impl CompanyState {
    /// Create a new empty company state
    #[must_use]
    pub fn new() -> Self {
        Self {
            companies: HashMap::new(),
        }
    }

    /// Register a new company
    ///
    /// # Errors
    /// Returns `CompanyError::CompanyAlreadyExists` if company already exists
    pub fn register_company(
        &mut self,
        company_id: [u8; 32],
        created_at: u64,
        status: CompanyStatus,
    ) -> Result<(), CompanyError> {
        if self.companies.contains_key(&company_id) {
            return Err(CompanyError::CompanyAlreadyExists { company_id });
        }

        let company = Company::new(company_id, created_at, status);
        self.companies.insert(company_id, company);
        Ok(())
    }

    /// Process company registration transaction
    ///
    /// Extracts company data from transaction and registers the company
    ///
    /// # Errors
    /// Returns `CompanyError` if registration fails
    pub fn process_company_registration(
        &mut self,
        transaction: &LoanTransaction,
    ) -> Result<(), CompanyError> {
        if transaction.transaction_type != TransactionType::CompanyRegistration {
            return Err(CompanyError::InvalidTransactionType);
        }

        // Company ID comes from originator_company_id field
        let company_id = transaction.originator_company_id;

        // Status comes from first byte of metadata (0=Active, 1=Suspended, 2=Inactive)
        let metadata = transaction.metadata.as_bytes();
        let status = CompanyStatus::from_u8(metadata[0]);

        self.register_company(company_id, transaction.created_at, status)
    }

    /// Process company update transaction
    ///
    /// # Errors
    /// Returns `CompanyError` if company not found or update fails
    pub fn process_company_update(
        &mut self,
        transaction: &LoanTransaction,
    ) -> Result<(), CompanyError> {
        if transaction.transaction_type != TransactionType::CompanyUpdate {
            return Err(CompanyError::InvalidTransactionType);
        }

        let company_id = transaction.originator_company_id;

        let company = self
            .companies
            .get_mut(&company_id)
            .ok_or(CompanyError::CompanyNotFound { company_id })?;

        // Status comes from first byte of metadata
        let metadata = transaction.metadata.as_bytes();
        let new_status = CompanyStatus::from_u8(metadata[0]);
        company.update_status(new_status);

        Ok(())
    }

    /// Get a company by ID
    #[must_use]
    pub fn get_company(&self, company_id: &[u8; 32]) -> Option<&Company> {
        self.companies.get(company_id)
    }

    /// Check if a company exists
    #[must_use]
    pub fn company_exists(&self, company_id: &[u8; 32]) -> bool {
        self.companies.contains_key(company_id)
    }

    /// Get total number of companies
    #[must_use]
    pub fn total_companies(&self) -> usize {
        self.companies.len()
    }
}

impl Default for CompanyState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transactions::MetadataArray;

    #[test]
    fn test_company_creation() {
        let company_id = [1u8; 32];
        let company = Company::new(company_id, 1000, CompanyStatus::Active);

        assert_eq!(company.company_id, company_id);
        assert_eq!(company.created_at, 1000);
        assert_eq!(company.status, CompanyStatus::Active);
        assert!(company.is_active());
    }

    #[test]
    fn test_company_status_update() {
        let company_id = [1u8; 32];
        let mut company = Company::new(company_id, 1000, CompanyStatus::Active);

        company.update_status(CompanyStatus::Suspended);
        assert_eq!(company.status, CompanyStatus::Suspended);
        assert!(!company.is_active());
    }

    #[test]
    fn test_company_registration() {
        let mut state = CompanyState::new();
        let company_id = [1u8; 32];

        let result = state.register_company(company_id, 1000, CompanyStatus::Active);
        assert!(result.is_ok());
        assert_eq!(state.total_companies(), 1);
        assert!(state.company_exists(&company_id));

        // Try to register same company again
        let result = state.register_company(company_id, 1000, CompanyStatus::Active);
        assert!(matches!(
            result,
            Err(CompanyError::CompanyAlreadyExists { .. })
        ));
    }

    #[test]
    fn test_process_company_registration_transaction() {
        let mut state = CompanyState::new();
        let company_id = [1u8; 32];

        let mut metadata = MetadataArray::empty();
        metadata.as_mut_bytes()[0] = CompanyStatus::Active as u8;

        let transaction = LoanTransaction {
            transaction_id: [10u8; 32],
            user_id: [2u8; 32],
            originator_company_id: company_id,
            investor_company_id: [0u8; 32],
            deal_id: [0u8; 32],
            transaction_type: TransactionType::CompanyRegistration,
            amount: 0,
            created_at: 1000,
            metadata,
        };

        let result = state.process_company_registration(&transaction);
        assert!(result.is_ok());

        let company = state.get_company(&company_id).unwrap();
        assert_eq!(company.company_id, company_id);
        assert_eq!(company.created_at, 1000);
        assert_eq!(company.status, CompanyStatus::Active);
    }

    #[test]
    fn test_process_company_update_transaction() {
        let mut state = CompanyState::new();
        let company_id = [1u8; 32];

        // Register company first
        state
            .register_company(company_id, 1000, CompanyStatus::Active)
            .unwrap();

        // Update to suspended
        let mut metadata = MetadataArray::empty();
        metadata.as_mut_bytes()[0] = CompanyStatus::Suspended as u8;

        let transaction = LoanTransaction {
            transaction_id: [11u8; 32],
            user_id: [2u8; 32],
            originator_company_id: company_id,
            investor_company_id: [0u8; 32],
            deal_id: [0u8; 32],
            transaction_type: TransactionType::CompanyUpdate,
            amount: 0,
            created_at: 2000,
            metadata,
        };

        let result = state.process_company_update(&transaction);
        assert!(result.is_ok());

        let company = state.get_company(&company_id).unwrap();
        assert_eq!(company.status, CompanyStatus::Suspended);
    }

    #[test]
    fn test_company_update_nonexistent() {
        let mut state = CompanyState::new();
        let company_id = [1u8; 32];

        let mut metadata = MetadataArray::empty();
        metadata.as_mut_bytes()[0] = CompanyStatus::Suspended as u8;

        let transaction = LoanTransaction {
            transaction_id: [11u8; 32],
            user_id: [2u8; 32],
            originator_company_id: company_id,
            investor_company_id: [0u8; 32],
            deal_id: [0u8; 32],
            transaction_type: TransactionType::CompanyUpdate,
            amount: 0,
            created_at: 2000,
            metadata,
        };

        let result = state.process_company_update(&transaction);
        assert!(matches!(result, Err(CompanyError::CompanyNotFound { .. })));
    }
}
