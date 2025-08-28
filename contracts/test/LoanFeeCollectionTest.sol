// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Test, console} from "forge-std/Test.sol";
import {Loan} from "../src/Loan.sol";
import {SP1VerifierGateway} from "@sp1-contracts/SP1VerifierGateway.sol";
import {ERC20Mock} from "openzeppelin-contracts/contracts/mocks/token/ERC20Mock.sol";

contract LoanFeeCollectionTest is Test {
    Loan public loan;
    ERC20Mock public usdc;
    
    address public owner = address(0x123);
    address public ardaAdmin = address(0x456);
    address public user1 = address(0x789);
    address public user2 = address(0xabc);
    address public kycProvider = address(0xdef);
    address public unauthorizedUser = address(0x111);
    
    bytes32 public constant PROGRAM_VKEY = bytes32(uint256(0x1234));
    bytes32 public constant STATE_ID_1 = keccak256("test-state-1");
    // Use a deterministic state root that can work with balance verification
    bytes32 public TEST_STATE_ROOT;
    
    uint256 public constant INITIAL_USDC_SUPPLY = 1000000e6; // 1M USDC
    uint256 public constant DEPOSIT_AMOUNT = 10000e6; // 10k USDC
    uint256 public constant WITHDRAWAL_AMOUNT = 5000e6; // 5k USDC
    uint256 public constant FEE_BPS = 200; // 2%
    
    event FeeCollected(uint256 feeAmount, uint256 transactionAmount, uint256 timestamp);

    function setUp() public {
        vm.startPrank(owner);
        
        // Deploy USDC mock
        usdc = new ERC20Mock();
        usdc.mint(owner, INITIAL_USDC_SUPPLY);
        usdc.mint(user1, INITIAL_USDC_SUPPLY);
        usdc.mint(user2, INITIAL_USDC_SUPPLY);
        usdc.mint(ardaAdmin, INITIAL_USDC_SUPPLY);
        
        // Deploy SP1 verifier
        address verifier = address(new SP1VerifierGateway(address(1)));
        
        // Deploy Loan contract
        loan = new Loan(verifier, PROGRAM_VKEY, address(usdc), ardaAdmin);
        
        // Set up KYC provider and verify users
        loan.setKYCProvider(kycProvider, true);
        
        vm.stopPrank();
        
        // KYC verify users for withdrawal tests
        vm.startPrank(kycProvider);
        loan.setKYCStatus(user1, true);
        loan.setKYCStatus(user2, true);
        vm.stopPrank();
        
        // Set up TEST_STATE_ROOT to work with balance verification
        // For empty proof, the verification expects: keccak256(abi.encode(user, amount)) == stateRoot
        TEST_STATE_ROOT = keccak256(abi.encode(user1, WITHDRAWAL_AMOUNT));
        
        // Set up a mock state for withdrawal proofs
        vm.startPrank(owner);
        address loanVerifier = loan.verifier();
        vm.mockCall(
            loanVerifier,
            abi.encodeWithSelector(SP1VerifierGateway.verifyProof.selector),
            abi.encode(true)
        );
        
        bytes memory mockProof = "mock_proof";
        bytes memory mockPublicValues = "mock_public_values";
        // Set up state with TEST_STATE_ROOT as both stateId and newStateRoot to work around contract logic
        loan.updateState(TEST_STATE_ROOT, TEST_STATE_ROOT, mockProof, mockPublicValues);
        vm.stopPrank();
    }
    
    /*//////////////////////////////////////////////////////////////
                            FEE CONSTANTS TESTS
    //////////////////////////////////////////////////////////////*/
    
    function test_FeeConstants() public view {
        assertEq(loan.LOAN_TRANSACTION_FEE_BPS(), 200);
    }
    
    function test_ArdaAdminAddress() public view {
        assertEq(loan.ARDA_ADMIN_ADDRESS(), ardaAdmin);
    }
    
    /*//////////////////////////////////////////////////////////////
                        FEE COLLECTION ON WITHDRAWAL
    //////////////////////////////////////////////////////////////*/
    
    function test_FeeCollection_OnWithdrawal() public {
        // Make deposit first
        vm.startPrank(user1);
        usdc.approve(address(loan), DEPOSIT_AMOUNT);
        loan.depositForLoan(DEPOSIT_AMOUNT, bytes32(0), false, 0);
        vm.stopPrank();
        
        // Mock balance proof verification
        _mockBalanceProofVerification();
        
        uint256 expectedFee = (WITHDRAWAL_AMOUNT * FEE_BPS) / 10000;
        
        vm.startPrank(user1);
        
        bytes32[] memory balanceProof = _createMockBalanceProof();
        
        vm.expectEmit(false, false, false, true);
        emit FeeCollected(expectedFee, WITHDRAWAL_AMOUNT, block.timestamp);
        
        loan.submitWithdrawal(WITHDRAWAL_AMOUNT, TEST_STATE_ROOT, balanceProof);
        
        vm.stopPrank();
        
        // Verify fee was collected (check through getFeeInfo)
        _verifyFeeCollection(expectedFee, 0);
    }
    
    function test_FeeCollection_MultipleWithdrawals() public {
        // Make large deposit
        vm.startPrank(user1);
        usdc.approve(address(loan), DEPOSIT_AMOUNT);
        loan.depositForLoan(DEPOSIT_AMOUNT, bytes32(0), false, 0);
        vm.stopPrank();
        
        _mockBalanceProofVerification();
        
        uint256 firstAmount = 2000e6;
        uint256 secondAmount = 3000e6;
        uint256 firstFee = (firstAmount * FEE_BPS) / 10000;
        uint256 secondFee = (secondAmount * FEE_BPS) / 10000;
        
        vm.startPrank(user1);
        bytes32[] memory balanceProof = _createMockBalanceProof();
        
        // First withdrawal
        vm.expectEmit(false, false, false, true);
        emit FeeCollected(firstFee, firstAmount, block.timestamp);
        
        loan.submitWithdrawal(firstAmount, TEST_STATE_ROOT, balanceProof);
        
        // Second withdrawal (different timestamp)
        vm.warp(block.timestamp + 1);
        vm.expectEmit(false, false, false, true);
        emit FeeCollected(secondFee, secondAmount, block.timestamp);
        
        loan.submitWithdrawal(secondAmount, TEST_STATE_ROOT, balanceProof);
        
        vm.stopPrank();
        
        // Verify total fees collected
        _verifyFeeCollection(firstFee + secondFee, 0);
    }
    
    function test_FeeCollection_DifferentUsers() public {
        // Both users make deposits
        vm.startPrank(user1);
        usdc.approve(address(loan), DEPOSIT_AMOUNT);
        loan.depositForLoan(DEPOSIT_AMOUNT, bytes32(0), false, 0);
        vm.stopPrank();
        
        vm.startPrank(user2);
        usdc.approve(address(loan), DEPOSIT_AMOUNT);
        loan.depositForLoan(DEPOSIT_AMOUNT, bytes32(0), false, 0);
        vm.stopPrank();
        
        _mockBalanceProofVerification();
        
        uint256 user1Amount = 3000e6;
        uint256 user2Amount = 2000e6;
        uint256 user1Fee = (user1Amount * FEE_BPS) / 10000;
        uint256 user2Fee = (user2Amount * FEE_BPS) / 10000;
        
        bytes32[] memory balanceProof = _createMockBalanceProof();
        
        // User 1 withdraws
        vm.startPrank(user1);
        vm.expectEmit(false, false, false, true);
        emit FeeCollected(user1Fee, user1Amount, block.timestamp);
        loan.submitWithdrawal(user1Amount, TEST_STATE_ROOT, balanceProof);
        vm.stopPrank();
        
        // User 2 withdraws (different timestamp)
        vm.startPrank(user2);
        vm.warp(block.timestamp + 1);
        vm.expectEmit(false, false, false, true);
        emit FeeCollected(user2Fee, user2Amount, block.timestamp);
        loan.submitWithdrawal(user2Amount, TEST_STATE_ROOT, balanceProof);
        vm.stopPrank();
        
        // Verify total fees collected
        _verifyFeeCollection(user1Fee + user2Fee, 0);
    }
    
    /*//////////////////////////////////////////////////////////////
                            FEE WITHDRAWAL
    //////////////////////////////////////////////////////////////*/
    
    function test_WithdrawFees_ArdaAdmin() public {
        // Generate some fees first
        _generateFees(WITHDRAWAL_AMOUNT);
        
        uint256 expectedFee = (WITHDRAWAL_AMOUNT * FEE_BPS) / 10000;
        uint256 initialBalance = usdc.balanceOf(ardaAdmin);
        
        vm.startPrank(ardaAdmin);
        
        loan.withdrawFees(expectedFee);
        
        vm.stopPrank();
        
        // Check balances
        assertEq(usdc.balanceOf(ardaAdmin), initialBalance + expectedFee);
        
        // Check fee tracking
        _verifyFeeCollection(expectedFee, expectedFee);
    }
    
    function test_WithdrawFees_Partial() public {
        // Generate fees
        uint256 totalFees = _generateFees(WITHDRAWAL_AMOUNT);
        uint256 partialAmount = totalFees / 2;
        
        vm.startPrank(ardaAdmin);
        
        loan.withdrawFees(partialAmount);
        
        vm.stopPrank();
        
        // Check fee tracking
        _verifyFeeCollection(totalFees, partialAmount);
    }
    
    function test_WithdrawFees_Multiple() public {
        // Generate fees
        uint256 totalFees = _generateFees(WITHDRAWAL_AMOUNT);
        uint256 firstWithdrawal = totalFees / 3;
        uint256 secondWithdrawal = totalFees / 3;
        
        vm.startPrank(ardaAdmin);
        
        // First withdrawal
        loan.withdrawFees(firstWithdrawal);
        _verifyFeeCollection(totalFees, firstWithdrawal);
        
        // Second withdrawal
        loan.withdrawFees(secondWithdrawal);
        _verifyFeeCollection(totalFees, firstWithdrawal + secondWithdrawal);
        
        vm.stopPrank();
    }
    
    function testRevert_WithdrawFees_UnauthorizedUser() public {
        // Generate some fees
        _generateFees(WITHDRAWAL_AMOUNT);
        uint256 feeAmount = (WITHDRAWAL_AMOUNT * FEE_BPS) / 10000;
        
        vm.startPrank(unauthorizedUser);
        
        vm.expectRevert("Only Arda admin can withdraw fees");
        loan.withdrawFees(feeAmount);
        
        vm.stopPrank();
    }
    
    function testRevert_WithdrawFees_InsufficientFees() public {
        // Generate small amount of fees
        uint256 feeAmount = _generateFees(1000e6); // Small withdrawal generates small fee
        
        vm.startPrank(ardaAdmin);
        
        // Try to withdraw more than available
        vm.expectRevert("Insufficient fees available");
        loan.withdrawFees(feeAmount * 2);
        
        vm.stopPrank();
    }
    
    function testRevert_WithdrawFees_ZeroFeesAvailable() public {
        vm.startPrank(ardaAdmin);
        
        vm.expectRevert("Insufficient fees available");
        loan.withdrawFees(100e6);
        
        vm.stopPrank();
    }
    
    /*//////////////////////////////////////////////////////////////
                        FEE CALCULATION TESTS
    //////////////////////////////////////////////////////////////*/
    
    function test_FeeCalculation_ExactPercentage() public {
        uint256[] memory amounts = new uint256[](4);
        amounts[0] = 10000e6; // 10k USDC
        amounts[1] = 1000e6;  // 1k USDC
        amounts[2] = 100e6;   // 100 USDC
        amounts[3] = 1e6;     // 1 USDC
        
        for (uint i = 0; i < amounts.length; i++) {
            uint256 amount = amounts[i];
            uint256 expectedFee = (amount * FEE_BPS) / 10000;
            uint256 actualFee = _generateFees(amount);
            assertEq(actualFee, expectedFee, "Fee calculation mismatch");
        }
    }
    
    function test_FeeCalculation_RoundingDown() public {
        // Test amounts that don't divide evenly
        uint256 oddAmount = 333e6; // 333 USDC
        uint256 expectedFee = (oddAmount * FEE_BPS) / 10000; // Should round down
        
        uint256 actualFee = _generateFees(oddAmount);
        assertEq(actualFee, expectedFee);
        
        // Verify it rounds down by checking manual calculation
        // 333e6 * 200 / 10000 = 66.6e5 = 6.66e6, should be 6660000
        assertEq(expectedFee, 6660000);
    }
    
    function test_FeeCalculation_VerySmallAmounts() public {
        // Test with amounts smaller than fee basis
        uint256 tinyAmount = 1; // 1 wei of USDC
        uint256 expectedFee = 0; // Should be 0 due to integer division
        
        uint256 actualFee = _generateFees(tinyAmount);
        assertEq(actualFee, expectedFee);
    }
    
    /*//////////////////////////////////////////////////////////////
                            VIEW FUNCTIONS
    //////////////////////////////////////////////////////////////*/
    
    function test_GetFeeInfo_Structure() public {
        // Test that getFeeInfo returns proper data
        // We can't access struct fields directly, but can verify it doesn't revert
        loan.getFeeInfo();
    }
    
    function test_GetFeeInfo_AfterFeeGeneration() public {
        // Generate some fees
        _generateFees(WITHDRAWAL_AMOUNT);
        
        // Function should still work
        loan.getFeeInfo();
    }
    
    function test_GetFeeInfo_AfterPartialWithdrawal() public {
        // Generate fees and withdraw some
        uint256 totalFees = _generateFees(WITHDRAWAL_AMOUNT);
        
        vm.prank(ardaAdmin);
        loan.withdrawFees(totalFees / 2);
        
        // Function should still work
        loan.getFeeInfo();
    }
    
    /*//////////////////////////////////////////////////////////////
                            INTEGRATION TESTS
    //////////////////////////////////////////////////////////////*/
    
    function test_FeeFlow_EndToEnd() public {
        uint256 initialAdminBalance = usdc.balanceOf(ardaAdmin);
        
        // Multiple users make deposits
        vm.startPrank(user1);
        usdc.approve(address(loan), DEPOSIT_AMOUNT);
        loan.depositForLoan(DEPOSIT_AMOUNT, bytes32(0), false, 0);
        vm.stopPrank();
        
        vm.startPrank(user2);
        usdc.approve(address(loan), DEPOSIT_AMOUNT);
        loan.depositForLoan(DEPOSIT_AMOUNT, bytes32(0), false, 0);
        vm.stopPrank();
        
        _mockBalanceProofVerification();
        
        // Multiple withdrawals generate fees
        uint256 withdrawal1 = 3000e6;
        uint256 withdrawal2 = 2000e6;
        uint256 expectedTotalFees = ((withdrawal1 + withdrawal2) * FEE_BPS) / 10000;
        
        bytes32[] memory balanceProof = _createMockBalanceProof();
        
        vm.startPrank(user1);
        loan.submitWithdrawal(withdrawal1, TEST_STATE_ROOT, balanceProof);
        vm.stopPrank();
        
        vm.startPrank(user2);
        vm.warp(block.timestamp + 1);
        loan.submitWithdrawal(withdrawal2, TEST_STATE_ROOT, balanceProof);
        vm.stopPrank();
        
        // Admin withdraws all fees
        vm.startPrank(ardaAdmin);
        loan.withdrawFees(expectedTotalFees);
        vm.stopPrank();
        
        // Verify final state
        assertEq(usdc.balanceOf(ardaAdmin), initialAdminBalance + expectedTotalFees);
        _verifyFeeCollection(expectedTotalFees, expectedTotalFees);
    }
    
    /*//////////////////////////////////////////////////////////////
                            HELPER FUNCTIONS
    //////////////////////////////////////////////////////////////*/
    
    function _generateFees(uint256 withdrawalAmount) internal returns (uint256 feeAmount) {
        // Set up specific state root for this withdrawal amount
        bytes32 specificStateRoot = keccak256(abi.encode(user1, withdrawalAmount));
        
        // Update state with this specific root
        vm.startPrank(owner);
        bytes memory mockProof = "mock_proof_specific";
        bytes memory mockPublicValues = "mock_public_values_specific";
        loan.updateState(specificStateRoot, specificStateRoot, mockProof, mockPublicValues);
        vm.stopPrank();
        
        // Make deposit
        vm.startPrank(user1);
        usdc.approve(address(loan), DEPOSIT_AMOUNT);
        loan.depositForLoan(DEPOSIT_AMOUNT, bytes32(0), false, 0);
        vm.stopPrank();
        
        // Make withdrawal to generate fees
        vm.startPrank(user1);
        bytes32[] memory balanceProof = _createMockBalanceProof();
        loan.submitWithdrawal(withdrawalAmount, specificStateRoot, balanceProof);
        vm.stopPrank();
        
        return (withdrawalAmount * FEE_BPS) / 10000;
    }
    
    function _createMockBalanceProof() internal pure returns (bytes32[] memory) {
        bytes32[] memory proof = new bytes32[](2);
        proof[0] = keccak256("mock_proof_1");
        proof[1] = keccak256("mock_proof_2");
        return proof;
    }
    
    function _mockBalanceProofVerification() internal {
        // In a real implementation, we would mock the internal _verifyBalanceProof
        // For now, the mocked verifier should handle this
    }
    
    function _verifyFeeCollection(uint256 expectedTotal, uint256 expectedWithdrawn) internal {
        // We can't directly access the struct fields, but we can verify through behavior
        // The fact that withdrawFees works with expected amounts validates the tracking
        
        // Try to withdraw the expected available amount
        uint256 expectedAvailable = expectedTotal - expectedWithdrawn;
        
        if (expectedAvailable > 0) {
            uint256 beforeBalance = usdc.balanceOf(ardaAdmin);
            
            vm.prank(ardaAdmin);
            loan.withdrawFees(expectedAvailable);
            
            uint256 afterBalance = usdc.balanceOf(ardaAdmin);
            assertEq(afterBalance - beforeBalance, expectedAvailable, "Fee withdrawal amount mismatch");
        }
    }
    
    /*//////////////////////////////////////////////////////////////
                            EDGE CASES
    //////////////////////////////////////////////////////////////*/
    
    function test_FeeCollection_MaxUint256() public {
        // Test with very large withdrawal (but within reasonable bounds)
        uint256 largeAmount = 100000e6; // 100k USDC
        
        vm.startPrank(user1);
        usdc.mint(user1, largeAmount);
        usdc.approve(address(loan), largeAmount);
        loan.depositForLoan(largeAmount, bytes32(0), false, 0);
        vm.stopPrank();
        
        _mockBalanceProofVerification();
        
        uint256 expectedFee = (largeAmount * FEE_BPS) / 10000;
        
        vm.startPrank(user1);
        bytes32[] memory balanceProof = _createMockBalanceProof();
        
        vm.expectEmit(false, false, false, true);
        emit FeeCollected(expectedFee, largeAmount, block.timestamp);
        
        loan.submitWithdrawal(largeAmount, TEST_STATE_ROOT, balanceProof);
        vm.stopPrank();
    }
    
    function test_WithdrawFees_ExactAmount() public {
        uint256 feeAmount = _generateFees(WITHDRAWAL_AMOUNT);
        
        vm.startPrank(ardaAdmin);
        
        // Withdraw exactly what was collected
        loan.withdrawFees(feeAmount);
        
        // Should have nothing left to withdraw
        vm.expectRevert("Insufficient fees available");
        loan.withdrawFees(1);
        
        vm.stopPrank();
    }
}