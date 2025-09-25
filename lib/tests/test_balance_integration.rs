//! Integration tests for balance tracking with deal transaction processing

use lib::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transactions::MetadataArray;

    fn fund_user_account(
        state: &mut GlobalLoanState,
        user_id: [u8; 32],
        amount: u128,
        timestamp: u64,
    ) {
        let funding = BalanceUpdate::new(
            [99u8; 32], // funding transaction ID
            user_id,
            None,
            None,
            BalanceUpdateType::ManualAdjustment,
            amount,
            0, // previous balance
            timestamp,
            Some("Test funding".to_string()),
        )
        .unwrap();
        state.balance_state.apply_update(funding).unwrap();
    }

    fn create_test_user_registration(user_id: [u8; 32], user_type: u8) -> LoanTransaction {
        LoanTransaction {
            transaction_id: [1u8; 32],
            user_id,
            loan_id: [0u8; 32],
            transaction_type: TransactionType::UserRegistration,
            amount: 0,
            created_at: 1_640_995_200,
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

    fn create_deal_origination(
        deal_id: [u8; 32],
        originator_id: [u8; 32],
        investor_id: [u8; 32],
        amount: u64,
    ) -> LoanTransaction {
        let mut metadata = vec![0u8; 64];
        metadata[0..32].copy_from_slice(&originator_id);
        metadata[32..64].copy_from_slice(&investor_id);

        LoanTransaction {
            transaction_id: [3u8; 32],
            user_id: originator_id,
            loan_id: deal_id, // Keep loan_id field for transaction compatibility
            transaction_type: TransactionType::DealOrigination,
            amount,
            created_at: 1640995400,
            metadata: MetadataArray::from_bytes(&metadata),
        }
    }

    fn create_deal_payment(
        payment_id: [u8; 32],
        deal_id: [u8; 32],
        payer_id: [u8; 32],
        amount: u64,
        timestamp: u64,
    ) -> LoanTransaction {
        LoanTransaction {
            transaction_id: payment_id,
            user_id: payer_id,
            loan_id: deal_id, // Keep loan_id field for transaction compatibility
            transaction_type: TransactionType::DealPayment,
            amount,
            created_at: timestamp,
            metadata: MetadataArray::empty(),
        }
    }

    #[test]
    fn test_deal_origination_creates_balance_updates() {
        let mut state = GlobalLoanState::new();

        let originator_id = [1u8; 32];
        let investor_id = [2u8; 32];
        let deal_id = [10u8; 32];

        // Setup KYC'd users
        let originator_reg = create_test_user_registration(originator_id, 1); // Investor
        let investor_reg = create_test_user_registration(investor_id, 2); // Lender
        let originator_kyc = create_kyc_verification(originator_id, true);
        let investor_kyc = create_kyc_verification(investor_id, true);

        state.process_transaction(&originator_reg).unwrap();
        state.process_transaction(&investor_reg).unwrap();
        state.process_transaction(&originator_kyc).unwrap();
        state.process_transaction(&investor_kyc).unwrap();

        // Initial balance state should be empty (before funding)
        assert_eq!(state.get_user_balance(&originator_id), 0);
        assert_eq!(state.get_user_balance(&investor_id), 0);
        assert_eq!(state.balance_state.updates.len(), 0);

        // Fund investor for deal origination (required for balance validation)
        fund_user_account(&mut state, investor_id, 100_000, 1_640_995_150);

        // Process deal origination
        let deal_origination = create_deal_origination(deal_id, originator_id, investor_id, 50000);
        state.process_transaction(&deal_origination).unwrap();

        // Verify deal was created
        let deal = state.get_loan(&deal_id).unwrap(); // Keep using get_loan for compatibility
        assert_eq!(deal.principal_amount, 50000);
        assert_eq!(deal.borrower_id, originator_id); // Keep borrower_id field for compatibility
        assert_eq!(deal.lender_id, investor_id); // Keep lender_id field for compatibility

        // Verify balance updates were created (funding + 2 origination updates)
        assert_eq!(state.balance_state.updates.len(), 3);

        // Find originator credit update
        let originator_updates: Vec<_> = state
            .balance_state
            .updates
            .iter()
            .filter(|u| u.user_id == originator_id)
            .collect();
        assert_eq!(originator_updates.len(), 1);

        let originator_update = originator_updates[0];
        assert_eq!(
            originator_update.update_type,
            BalanceUpdateType::CreditFromOrigination
        );
        assert_eq!(originator_update.amount, 50000);
        assert_eq!(originator_update.previous_balance, 0);
        assert_eq!(originator_update.new_balance, 50000);
        assert_eq!(originator_update.loan_id, Some(deal_id));

        // Find investor debit update
        let investor_updates: Vec<_> = state
            .balance_state
            .updates
            .iter()
            .filter(|u| u.user_id == investor_id)
            .collect();
        assert_eq!(investor_updates.len(), 2); // Funding + DebitFromOrigination

        // Find the origination debit update (not the funding)
        let investor_debit_update = investor_updates
            .iter()
            .find(|u| u.update_type == BalanceUpdateType::DebitFromOrigination)
            .expect("Should find debit from origination");

        assert_eq!(
            investor_debit_update.update_type,
            BalanceUpdateType::DebitFromOrigination
        );
        assert_eq!(investor_debit_update.amount, 50000);
        assert_eq!(investor_debit_update.previous_balance, 100_000); // After funding
        assert_eq!(investor_debit_update.new_balance, 50000); // 100_000 - 50000
        assert_eq!(investor_debit_update.loan_id, Some(deal_id));

        // Verify current balances
        assert_eq!(state.get_user_balance(&originator_id), 50000);
        assert_eq!(state.get_user_balance(&investor_id), 50000); // 100_000 funding - 50000 lent
    }

    #[test]
    fn test_deal_payment_creates_balance_updates() {
        let mut state = GlobalLoanState::new();

        let originator_id = [1u8; 32];
        let investor_id = [2u8; 32];
        let deal_id = [10u8; 32];

        // Setup users and originate deal
        let originator_reg = create_test_user_registration(originator_id, 1);
        let investor_reg = create_test_user_registration(investor_id, 2);
        let originator_kyc = create_kyc_verification(originator_id, true);
        let investor_kyc = create_kyc_verification(investor_id, true);
        let deal_origination = create_deal_origination(deal_id, originator_id, investor_id, 50000);

        state.process_transaction(&originator_reg).unwrap();
        state.process_transaction(&investor_reg).unwrap();
        state.process_transaction(&originator_kyc).unwrap();
        state.process_transaction(&investor_kyc).unwrap();

        // Fund investor for deal origination
        fund_user_account(&mut state, investor_id, 100_000, 1_640_995_150);

        state.process_transaction(&deal_origination).unwrap();

        // Check balance updates count (funding + origination)
        let initial_update_count = state.balance_state.updates.len();
        assert_eq!(initial_update_count, 3); // 1 funding update + 2 origination updates

        // Process deal payment
        let payment = create_deal_payment([4u8; 32], deal_id, originator_id, 25000, 1_640_995_500);
        state.process_transaction(&payment).unwrap();

        // Verify deal payment was processed
        let deal = state.get_loan(&deal_id).unwrap(); // Keep using get_loan for compatibility
        assert_eq!(deal.outstanding_balance, 25000); // 50000 - 25000

        // Verify balance update was created for the payment
        assert_eq!(state.balance_state.updates.len(), 4); // 1 funding + 2 origination + 1 payment

        let payment_update = &state.balance_state.updates[3]; // Latest update
        assert_eq!(payment_update.user_id, investor_id); // Investor receives the payment
        assert_eq!(
            payment_update.update_type,
            BalanceUpdateType::CreditFromPayment
        );
        assert_eq!(payment_update.amount, 25000);
        assert_eq!(payment_update.previous_balance, 50000); // Investor balance before payment (after funding and lending)
        assert_eq!(payment_update.new_balance, 75000); // 50000 + 25000 payment
        assert_eq!(payment_update.loan_id, Some(deal_id));
        assert_eq!(payment_update.triggering_transaction_id, Some([4u8; 32]));

        // Verify updated balances
        assert_eq!(state.get_user_balance(&originator_id), 50000); // Unchanged from origination
        assert_eq!(state.get_user_balance(&investor_id), 75000); // 50000 remaining + 25000 payment
    }

    #[test]
    fn test_balance_history_tracking() {
        let mut state = GlobalLoanState::new();

        let originator_id = [1u8; 32];
        let investor_id = [2u8; 32];
        let deal_id = [10u8; 32];

        // Setup and process full deal lifecycle
        let setup_transactions = vec![
            create_test_user_registration(originator_id, 1),
            create_test_user_registration(investor_id, 2),
            create_kyc_verification(originator_id, true),
            create_kyc_verification(investor_id, true),
        ];

        for tx in setup_transactions {
            state.process_transaction(&tx).unwrap();
        }

        // Fund investor for deal origination
        fund_user_account(&mut state, investor_id, 200_000, 1_640_995_150);

        let deal_transactions = vec![
            create_deal_origination(deal_id, originator_id, investor_id, 100_000),
            create_deal_payment([5u8; 32], deal_id, originator_id, 25000, 1_640_995_500),
            create_deal_payment([6u8; 32], deal_id, originator_id, 35000, 1_640_995_600),
            create_deal_payment([7u8; 32], deal_id, originator_id, 40000, 1_640_995_700), // Final payment
        ];

        for tx in deal_transactions {
            state.process_transaction(&tx).unwrap();
        }

        // Verify complete balance history for investor
        let investor_history = state.get_user_balance_history(&investor_id, None);
        assert_eq!(investor_history.len(), 5); // 1 funding + 1 debit from origination + 3 credits from payments

        // Check balance progression for investor
        let expected_balances = vec![
            (BalanceUpdateType::ManualAdjustment, 200_000, 0, 200_000), // Initial funding
            (
                BalanceUpdateType::DebitFromOrigination,
                100_000,
                200_000,
                100_000,
            ), // Invests out money
            (
                BalanceUpdateType::CreditFromPayment,
                25000,
                100_000,
                125_000,
            ), // First payment
            (
                BalanceUpdateType::CreditFromPayment,
                35000,
                125_000,
                160_000,
            ), // Second payment
            (
                BalanceUpdateType::CreditFromPayment,
                40000,
                160_000,
                200_000,
            ), // Final payment
        ];

        // Sort history by timestamp for comparison
        let mut sorted_history = investor_history.clone();
        sorted_history.sort_by_key(|u| u.created_at);

        for (i, update) in sorted_history.iter().enumerate() {
            let (expected_type, expected_amount, expected_prev, expected_new) =
                expected_balances[i];
            assert_eq!(update.update_type, expected_type);
            assert_eq!(update.amount, expected_amount);
            assert_eq!(update.previous_balance, expected_prev);
            assert_eq!(update.new_balance, expected_new);

            // Only deal-related updates have loan_id (not funding updates)
            if expected_type != BalanceUpdateType::ManualAdjustment {
                assert_eq!(update.loan_id, Some(deal_id));
            } else {
                assert_eq!(update.loan_id, None);
            }
        }

        // Verify final balance
        assert_eq!(state.get_user_balance(&investor_id), 200_000); // Back to original funded amount
    }

    #[test]
    fn test_balance_nullifier_generation() {
        let mut state = GlobalLoanState::new();

        let originator_id = [1u8; 32];
        let investor_id = [2u8; 32];
        let deal_id = [10u8; 32];

        // Setup and create balance update
        let originator_reg = create_test_user_registration(originator_id, 1);
        let investor_reg = create_test_user_registration(investor_id, 2);
        let originator_kyc = create_kyc_verification(originator_id, true);
        let investor_kyc = create_kyc_verification(investor_id, true);
        let deal_origination = create_deal_origination(deal_id, originator_id, investor_id, 50000);

        state.process_transaction(&originator_reg).unwrap();
        state.process_transaction(&investor_reg).unwrap();
        state.process_transaction(&originator_kyc).unwrap();
        state.process_transaction(&investor_kyc).unwrap();

        // Fund investor for deal origination
        fund_user_account(&mut state, investor_id, 100_000, 1_640_995_150);

        state.process_transaction(&deal_origination).unwrap();

        // Get balance updates and test nullifier generation
        let balance_updates = &state.balance_state.updates;
        assert_eq!(balance_updates.len(), 3); // 1 funding + 2 origination updates

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
            [1u8; 32],       // update_id
            [2u8; 32],       // user_id
            Some([3u8; 32]), // deal_id (keeping loan_id field for compatibility)
            Some([4u8; 32]), // triggering_transaction_id
            BalanceUpdateType::CreditFromPayment,
            50000,         // amount
            100_000,       // previous_balance
            1_640_995_200, // timestamp
            Some("Test balance update".to_string()),
        )
        .unwrap();

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
            0,         // leaf_index
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
    fn test_multiple_deals_balance_tracking() {
        let mut state = GlobalLoanState::new();

        let originator1_id = [1u8; 32];
        let originator2_id = [2u8; 32];
        let investor_id = [3u8; 32];
        let deal1_id = [10u8; 32];
        let deal2_id = [11u8; 32];

        // Setup users
        let users = vec![
            create_test_user_registration(originator1_id, 1),
            create_test_user_registration(originator2_id, 1),
            create_test_user_registration(investor_id, 2),
            create_kyc_verification(originator1_id, true),
            create_kyc_verification(originator2_id, true),
            create_kyc_verification(investor_id, true),
        ];

        for tx in users {
            state.process_transaction(&tx).unwrap();
        }

        // Fund investor for both deal originations
        fund_user_account(&mut state, investor_id, 200_000, 1_640_995_150); // Enough for both deals

        // Originate two deals
        let deal1_origination =
            create_deal_origination(deal1_id, originator1_id, investor_id, 50000);
        let deal2_origination =
            create_deal_origination(deal2_id, originator2_id, investor_id, 75000);

        state.process_transaction(&deal1_origination).unwrap();
        state.process_transaction(&deal2_origination).unwrap();

        // Verify balance updates for multiple deals
        assert_eq!(state.balance_state.updates.len(), 5); // 1 funding + 2 deals × 2 updates each

        // Check originator balances
        assert_eq!(state.get_user_balance(&originator1_id), 50000);
        assert_eq!(state.get_user_balance(&originator2_id), 75000);
        assert_eq!(state.get_user_balance(&investor_id), 75000); // 200_000 funding - 50000 - 75000 deals

        // Check deal-specific balance updates
        let deal1_updates = state.get_loan_balance_updates(&deal1_id); // Keep using get_loan_balance_updates for compatibility
        let deal2_updates = state.get_loan_balance_updates(&deal2_id); // Keep using get_loan_balance_updates for compatibility

        assert_eq!(deal1_updates.len(), 2); // Credit to originator, debit from investor
        assert_eq!(deal2_updates.len(), 2); // Credit to originator, debit from investor

        // Verify deal-specific amounts
        let deal1_originator_update = deal1_updates
            .iter()
            .find(|u| {
                u.user_id == originator1_id
                    && u.update_type == BalanceUpdateType::CreditFromOrigination
            })
            .unwrap();
        assert_eq!(deal1_originator_update.amount, 50000);

        let deal2_originator_update = deal2_updates
            .iter()
            .find(|u| {
                u.user_id == originator2_id
                    && u.update_type == BalanceUpdateType::CreditFromOrigination
            })
            .unwrap();
        assert_eq!(deal2_originator_update.amount, 75000);

        // Make payments on both deals
        let payment1 =
            create_deal_payment([20u8; 32], deal1_id, originator1_id, 25000, 1_640_995_500);
        let payment2 =
            create_deal_payment([21u8; 32], deal2_id, originator2_id, 35000, 1_640_995_600);

        state.process_transaction(&payment1).unwrap();
        state.process_transaction(&payment2).unwrap();

        // Verify payment balance updates
        assert_eq!(state.balance_state.updates.len(), 7); // 1 funding + 4 origination + 2 payment

        // Final investor balance should reflect both payments
        assert_eq!(state.get_user_balance(&investor_id), 135000); // 75000 remaining + 25000 + 35000 payments
    }

    #[test]
    fn test_balance_state_consistency() {
        let mut state = GlobalLoanState::new();

        let originator_id = [1u8; 32];
        let investor_id = [2u8; 32];
        let deal_id = [10u8; 32];

        // Process full deal lifecycle
        let setup_transactions = vec![
            create_test_user_registration(originator_id, 1),
            create_test_user_registration(investor_id, 2),
            create_kyc_verification(originator_id, true),
            create_kyc_verification(investor_id, true),
        ];

        for tx in setup_transactions {
            state.process_transaction(&tx).unwrap();
        }

        // Fund investor for deal origination
        fund_user_account(&mut state, investor_id, 200_000, 1_640_995_150);

        let deal_transactions = vec![
            create_deal_origination(deal_id, originator_id, investor_id, 100_000),
            create_deal_payment([5u8; 32], deal_id, originator_id, 100_000, 1_640_995_500), // Full payment
        ];

        for tx in deal_transactions {
            state.process_transaction(&tx).unwrap();
        }

        // Verify balance state consistency
        let originator_balance = state.get_user_balance(&originator_id);
        let investor_balance = state.get_user_balance(&investor_id);
        let total_user_balance = state.compute_total_user_balance();

        // Total user balance should equal sum of individual balances
        assert_eq!(total_user_balance, originator_balance + investor_balance);

        // Verify history consistency
        let originator_history = state.get_user_balance_history(&originator_id, None);
        let investor_history = state.get_user_balance_history(&investor_id, None);

        // Each user should have latest balance matching their history
        if let Some(latest_originator) = originator_history.first() {
            assert_eq!(latest_originator.new_balance, originator_balance);
        }
        if let Some(latest_investor) = investor_history.first() {
            assert_eq!(latest_investor.new_balance, investor_balance);
        }

        // Verify deal is fully paid off
        let deal = state.get_loan(&deal_id).unwrap(); // Keep using get_loan for compatibility
        assert_eq!(deal.outstanding_balance, 0);
        assert_eq!(deal.loan_status, LoanStatus::PaidOff); // Keep loan_status field for compatibility

        // Investor should have received full repayment (200_000 initial - 100_000 invested + 100_000 repaid)
        assert_eq!(investor_balance, 200_000);
    }
}
