//! Integration tests for balance tracking with loan transaction processing

use lib::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transactions::MetadataArray;
    use crate::user::{KycStatus, UserType};

    fn create_test_user_registration(user_id: [u8; 32], user_type: u8) -> LoanTransaction {
        LoanTransaction {
            transaction_id: [1u8; 32],
            user_id,
            loan_id: [0u8; 32],
            transaction_type: TransactionType::UserRegistration,
            amount: 0,
            created_at: 1640995200,
            metadata: MetadataArray::from_bytes(&[user_type]),
        }
    }

    fn create_kyc_verification(user_id: [u8; 32], approved: bool) -> LoanTransaction {
        LoanTransaction {
            transaction_id: [2u8; 32],
            user_id,
            loan_id: [0u8; 32],
            transaction_type: TransactionType::KycVerification,
            amount: 0,
            created_at: 1640995300,
            metadata: MetadataArray::from_bytes(&[if approved { 1 } else { 0 }]),
        }
    }

    fn create_loan_origination(
        loan_id: [u8; 32],
        borrower_id: [u8; 32],
        lender_id: [u8; 32],
        amount: u64,
    ) -> LoanTransaction {
        let mut metadata = vec![0u8; 64];
        metadata[0..32].copy_from_slice(&borrower_id);
        metadata[32..64].copy_from_slice(&lender_id);

        LoanTransaction {
            transaction_id: [3u8; 32],
            user_id: borrower_id,
            loan_id,
            transaction_type: TransactionType::LoanOrigination,
            amount,
            created_at: 1640995400,
            metadata: MetadataArray::from_bytes(&metadata),
        }
    }

    fn create_loan_payment(
        payment_id: [u8; 32],
        loan_id: [u8; 32],
        payer_id: [u8; 32],
        amount: u64,
        timestamp: u64,
    ) -> LoanTransaction {
        LoanTransaction {
            transaction_id: payment_id,
            user_id: payer_id,
            loan_id,
            transaction_type: TransactionType::LoanPayment,
            amount,
            created_at: timestamp,
            metadata: MetadataArray::empty(),
        }
    }

    #[test]
    fn test_loan_origination_creates_balance_updates() {
        let mut state = GlobalLoanState::new();

        let borrower_id = [1u8; 32];
        let lender_id = [2u8; 32];
        let loan_id = [10u8; 32];

        // Setup KYC'd users
        let borrower_reg = create_test_user_registration(borrower_id, 1); // Investor
        let lender_reg = create_test_user_registration(lender_id, 2); // Lender
        let borrower_kyc = create_kyc_verification(borrower_id, true);
        let lender_kyc = create_kyc_verification(lender_id, true);

        state.process_transaction(&borrower_reg).unwrap();
        state.process_transaction(&lender_reg).unwrap();
        state.process_transaction(&borrower_kyc).unwrap();
        state.process_transaction(&lender_kyc).unwrap();

        // Initial balance state should be empty
        assert_eq!(state.get_user_balance(&borrower_id), 0);
        assert_eq!(state.get_user_balance(&lender_id), 0);
        assert_eq!(state.balance_state.updates.len(), 0);

        // Process loan origination
        let loan_origination = create_loan_origination(loan_id, borrower_id, lender_id, 50000);
        state.process_transaction(&loan_origination).unwrap();

        // Verify loan was created
        let loan = state.get_loan(&loan_id).unwrap();
        assert_eq!(loan.principal_amount, 50000);
        assert_eq!(loan.borrower_id, borrower_id);
        assert_eq!(loan.lender_id, lender_id);

        // Verify balance updates were created
        assert_eq!(state.balance_state.updates.len(), 2);

        // Find borrower credit update
        let borrower_updates: Vec<_> = state.balance_state.updates
            .iter()
            .filter(|u| u.user_id == borrower_id)
            .collect();
        assert_eq!(borrower_updates.len(), 1);
        
        let borrower_update = borrower_updates[0];
        assert_eq!(borrower_update.update_type, BalanceUpdateType::CreditFromOrigination);
        assert_eq!(borrower_update.amount, 50000);
        assert_eq!(borrower_update.previous_balance, 0);
        assert_eq!(borrower_update.new_balance, 50000);
        assert_eq!(borrower_update.loan_id, Some(loan_id));

        // Find lender debit update
        let lender_updates: Vec<_> = state.balance_state.updates
            .iter()
            .filter(|u| u.user_id == lender_id)
            .collect();
        assert_eq!(lender_updates.len(), 1);
        
        let lender_update = lender_updates[0];
        assert_eq!(lender_update.update_type, BalanceUpdateType::DebitFromOrigination);
        assert_eq!(lender_update.amount, 50000);
        assert_eq!(lender_update.previous_balance, 0);
        assert_eq!(lender_update.new_balance, 0); // Can't go negative, saturates at 0
        assert_eq!(lender_update.loan_id, Some(loan_id));

        // Verify current balances
        assert_eq!(state.get_user_balance(&borrower_id), 50000);
        assert_eq!(state.get_user_balance(&lender_id), 0);
    }

    #[test]
    fn test_loan_payment_creates_balance_updates() {
        let mut state = GlobalLoanState::new();

        let borrower_id = [1u8; 32];
        let lender_id = [2u8; 32];
        let loan_id = [10u8; 32];

        // Setup users and originate loan
        let borrower_reg = create_test_user_registration(borrower_id, 1);
        let lender_reg = create_test_user_registration(lender_id, 2);
        let borrower_kyc = create_kyc_verification(borrower_id, true);
        let lender_kyc = create_kyc_verification(lender_id, true);
        let loan_origination = create_loan_origination(loan_id, borrower_id, lender_id, 50000);

        state.process_transaction(&borrower_reg).unwrap();
        state.process_transaction(&lender_reg).unwrap();
        state.process_transaction(&borrower_kyc).unwrap();
        state.process_transaction(&lender_kyc).unwrap();
        state.process_transaction(&loan_origination).unwrap();

        // Clear balance updates from origination for cleaner test
        let initial_update_count = state.balance_state.updates.len();
        assert_eq!(initial_update_count, 2); // Origination creates 2 updates

        // Process loan payment
        let payment = create_loan_payment([4u8; 32], loan_id, borrower_id, 25000, 1640995500);
        state.process_transaction(&payment).unwrap();

        // Verify loan payment was processed
        let loan = state.get_loan(&loan_id).unwrap();
        assert_eq!(loan.outstanding_balance, 25000); // 50000 - 25000

        // Verify balance update was created for the payment
        assert_eq!(state.balance_state.updates.len(), 3); // 2 origination + 1 payment
        
        let payment_update = &state.balance_state.updates[2]; // Latest update
        assert_eq!(payment_update.user_id, lender_id); // Lender receives the payment
        assert_eq!(payment_update.update_type, BalanceUpdateType::CreditFromPayment);
        assert_eq!(payment_update.amount, 25000);
        assert_eq!(payment_update.previous_balance, 0); // Lender balance before payment
        assert_eq!(payment_update.new_balance, 25000);
        assert_eq!(payment_update.loan_id, Some(loan_id));
        assert_eq!(payment_update.triggering_transaction_id, Some([4u8; 32]));

        // Verify updated balances
        assert_eq!(state.get_user_balance(&borrower_id), 50000); // Unchanged from origination
        assert_eq!(state.get_user_balance(&lender_id), 25000); // Received payment
    }

    #[test]
    fn test_balance_history_tracking() {
        let mut state = GlobalLoanState::new();

        let borrower_id = [1u8; 32];
        let lender_id = [2u8; 32];
        let loan_id = [10u8; 32];

        // Setup and process full loan lifecycle
        let transactions = vec![
            create_test_user_registration(borrower_id, 1),
            create_test_user_registration(lender_id, 2),
            create_kyc_verification(borrower_id, true),
            create_kyc_verification(lender_id, true),
            create_loan_origination(loan_id, borrower_id, lender_id, 100000),
            create_loan_payment([5u8; 32], loan_id, borrower_id, 25000, 1640995500),
            create_loan_payment([6u8; 32], loan_id, borrower_id, 35000, 1640995600),
            create_loan_payment([7u8; 32], loan_id, borrower_id, 40000, 1640995700), // Final payment
        ];

        for tx in transactions {
            state.process_transaction(&tx).unwrap();
        }

        // Verify complete balance history for lender
        let lender_history = state.get_user_balance_history(&lender_id, None);
        assert_eq!(lender_history.len(), 4); // 1 debit from origination + 3 credits from payments

        // Check balance progression for lender
        let mut expected_balances = vec![
            (BalanceUpdateType::DebitFromOrigination, 100000, 0, 0), // Lends out money
            (BalanceUpdateType::CreditFromPayment, 25000, 0, 25000), // First payment
            (BalanceUpdateType::CreditFromPayment, 35000, 25000, 60000), // Second payment  
            (BalanceUpdateType::CreditFromPayment, 40000, 60000, 100000), // Final payment
        ];

        // Sort history by timestamp for comparison
        let mut sorted_history = lender_history.clone();
        sorted_history.sort_by_key(|u| u.created_at);

        for (i, update) in sorted_history.iter().enumerate() {
            let (expected_type, expected_amount, expected_prev, expected_new) = expected_balances[i];
            assert_eq!(update.update_type, expected_type);
            assert_eq!(update.amount, expected_amount);
            assert_eq!(update.previous_balance, expected_prev);
            assert_eq!(update.new_balance, expected_new);
            assert_eq!(update.loan_id, Some(loan_id));
        }

        // Verify final balance
        assert_eq!(state.get_user_balance(&lender_id), 100000); // Fully repaid
    }

    #[test]
    fn test_balance_nullifier_generation() {
        let mut state = GlobalLoanState::new();

        let borrower_id = [1u8; 32];
        let lender_id = [2u8; 32];
        let loan_id = [10u8; 32];

        // Setup and create balance update
        let borrower_reg = create_test_user_registration(borrower_id, 1);
        let lender_reg = create_test_user_registration(lender_id, 2);
        let borrower_kyc = create_kyc_verification(borrower_id, true);
        let lender_kyc = create_kyc_verification(lender_id, true);
        let loan_origination = create_loan_origination(loan_id, borrower_id, lender_id, 50000);

        state.process_transaction(&borrower_reg).unwrap();
        state.process_transaction(&lender_reg).unwrap();
        state.process_transaction(&borrower_kyc).unwrap();
        state.process_transaction(&lender_kyc).unwrap();
        state.process_transaction(&loan_origination).unwrap();

        // Get balance updates and test nullifier generation
        let balance_updates = &state.balance_state.updates;
        assert_eq!(balance_updates.len(), 2);

        for update in balance_updates {
            let nullifier1 = update.to_nullifier();
            let nullifier2 = update.to_nullifier();
            
            // Same update should produce same nullifier
            assert_eq!(nullifier1, nullifier2);
            
            // Nullifiers should not be all zeros
            assert_ne!(nullifier1, [0u8; 32]);
        }

        // Different updates should produce different nullifiers
        let nullifier1 = balance_updates[0].to_nullifier();
        let nullifier2 = balance_updates[1].to_nullifier();
        assert_ne!(nullifier1, nullifier2);
    }

    #[test]
    fn test_balance_inclusion_proof() {
        use lib::BalanceUpdate;

        // Create a test balance update
        let balance_update = BalanceUpdate::new(
            [1u8; 32], // update_id
            [2u8; 32], // user_id
            Some([3u8; 32]), // loan_id
            Some([4u8; 32]), // triggering_transaction_id
            BalanceUpdateType::CreditFromPayment,
            50000, // amount
            100000, // previous_balance
            1640995200, // timestamp
            Some("Test balance update".to_string()),
        ).unwrap();

        // Create mock Merkle path
        let merkle_path = vec![[5u8; 32], [6u8; 32], [7u8; 32]];
        
        // Calculate expected root for the proof
        use sha2::{Digest, Sha256};
        let mut current_hash = balance_update.to_nullifier();
        let mut leaf_index = 0u64;

        for sibling in &merkle_path {
            let mut hasher = Sha256::new();
            if leaf_index % 2 == 0 {
                hasher.update(&current_hash);
                hasher.update(sibling);
            } else {
                hasher.update(sibling);
                hasher.update(&current_hash);
            }
            current_hash = hasher.finalize().into();
            leaf_index /= 2;
        }

        let merkle_root = current_hash;

        // Create inclusion proof
        let proof = BalanceInclusionProof::new(
            balance_update,
            merkle_path,
            merkle_root,
            [8u8; 32], // batch_id
            0, // leaf_index
        );

        // Verify the proof
        assert!(proof.verify());

        // Test proof description
        let description = proof.description();
        assert!(description.contains("Balance update proof"));
        assert!(description.contains("credit_from_payment"));
        assert!(description.contains("50000"));
    }

    #[test]
    fn test_multiple_loans_balance_tracking() {
        let mut state = GlobalLoanState::new();

        let borrower1_id = [1u8; 32];
        let borrower2_id = [2u8; 32];
        let lender_id = [3u8; 32];
        let loan1_id = [10u8; 32];
        let loan2_id = [11u8; 32];

        // Setup users
        let users = vec![
            create_test_user_registration(borrower1_id, 1),
            create_test_user_registration(borrower2_id, 1),
            create_test_user_registration(lender_id, 2),
            create_kyc_verification(borrower1_id, true),
            create_kyc_verification(borrower2_id, true),
            create_kyc_verification(lender_id, true),
        ];

        for tx in users {
            state.process_transaction(&tx).unwrap();
        }

        // Originate two loans
        let loan1_origination = create_loan_origination(loan1_id, borrower1_id, lender_id, 50000);
        let loan2_origination = create_loan_origination(loan2_id, borrower2_id, lender_id, 75000);
        
        state.process_transaction(&loan1_origination).unwrap();
        state.process_transaction(&loan2_origination).unwrap();

        // Verify balance updates for multiple loans
        assert_eq!(state.balance_state.updates.len(), 4); // 2 loans × 2 updates each

        // Check borrower balances
        assert_eq!(state.get_user_balance(&borrower1_id), 50000);
        assert_eq!(state.get_user_balance(&borrower2_id), 75000);
        assert_eq!(state.get_user_balance(&lender_id), 0); // Debits don't reduce below 0

        // Check loan-specific balance updates
        let loan1_updates = state.get_loan_balance_updates(&loan1_id);
        let loan2_updates = state.get_loan_balance_updates(&loan2_id);
        
        assert_eq!(loan1_updates.len(), 2); // Credit to borrower, debit from lender
        assert_eq!(loan2_updates.len(), 2); // Credit to borrower, debit from lender

        // Verify loan-specific amounts
        let loan1_borrower_update = loan1_updates
            .iter()
            .find(|u| u.user_id == borrower1_id && u.update_type == BalanceUpdateType::CreditFromOrigination)
            .unwrap();
        assert_eq!(loan1_borrower_update.amount, 50000);

        let loan2_borrower_update = loan2_updates
            .iter()
            .find(|u| u.user_id == borrower2_id && u.update_type == BalanceUpdateType::CreditFromOrigination)
            .unwrap();
        assert_eq!(loan2_borrower_update.amount, 75000);

        // Make payments on both loans
        let payment1 = create_loan_payment([20u8; 32], loan1_id, borrower1_id, 25000, 1640995500);
        let payment2 = create_loan_payment([21u8; 32], loan2_id, borrower2_id, 35000, 1640995600);
        
        state.process_transaction(&payment1).unwrap();
        state.process_transaction(&payment2).unwrap();

        // Verify payment balance updates
        assert_eq!(state.balance_state.updates.len(), 6); // 4 origination + 2 payment
        
        // Final lender balance should reflect both payments
        assert_eq!(state.get_user_balance(&lender_id), 60000); // 25000 + 35000
    }

    #[test]
    fn test_balance_state_consistency() {
        let mut state = GlobalLoanState::new();

        let borrower_id = [1u8; 32];
        let lender_id = [2u8; 32];
        let loan_id = [10u8; 32];

        // Process full loan lifecycle
        let transactions = vec![
            create_test_user_registration(borrower_id, 1),
            create_test_user_registration(lender_id, 2),
            create_kyc_verification(borrower_id, true),
            create_kyc_verification(lender_id, true),
            create_loan_origination(loan_id, borrower_id, lender_id, 100000),
            create_loan_payment([5u8; 32], loan_id, borrower_id, 100000, 1640995500), // Full payment
        ];

        for tx in transactions {
            state.process_transaction(&tx).unwrap();
        }

        // Verify balance state consistency
        let borrower_balance = state.get_user_balance(&borrower_id);
        let lender_balance = state.get_user_balance(&lender_id);
        let total_user_balance = state.compute_total_user_balance();

        // Total user balance should equal sum of individual balances
        assert_eq!(total_user_balance, borrower_balance + lender_balance);

        // Verify history consistency
        let borrower_history = state.get_user_balance_history(&borrower_id, None);
        let lender_history = state.get_user_balance_history(&lender_id, None);

        // Each user should have latest balance matching their history
        if let Some(latest_borrower) = borrower_history.first() {
            assert_eq!(latest_borrower.new_balance, borrower_balance);
        }
        if let Some(latest_lender) = lender_history.first() {
            assert_eq!(latest_lender.new_balance, lender_balance);
        }

        // Verify loan is fully paid off
        let loan = state.get_loan(&loan_id).unwrap();
        assert_eq!(loan.outstanding_balance, 0);
        assert_eq!(loan.loan_status, LoanStatus::PaidOff);

        // Lender should have received full repayment
        assert_eq!(lender_balance, 100000);
    }
}