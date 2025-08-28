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
    
    
    
    
    /*//////////////////////////////////////////////////////////////
                            FEE WITHDRAWAL
    //////////////////////////////////////////////////////////////*/
    
    
    
    
    
    
    function testRevert_WithdrawFees_ZeroFeesAvailable() public {
        vm.startPrank(ardaAdmin);
        
        vm.expectRevert("Insufficient fees available");
        loan.withdrawFees(100e6);
        
        vm.stopPrank();
    }
    
    /*//////////////////////////////////////////////////////////////
                        FEE CALCULATION TESTS
    //////////////////////////////////////////////////////////////*/
    
    
    
    
    /*//////////////////////////////////////////////////////////////
                            VIEW FUNCTIONS
    //////////////////////////////////////////////////////////////*/
    
    function test_GetFeeInfo_Structure() public {
        // Test that getFeeInfo returns proper data
        // We can't access struct fields directly, but can verify it doesn't revert
        loan.getFeeInfo();
    }
    
    
    
    /*//////////////////////////////////////////////////////////////
                            INTEGRATION TESTS
    //////////////////////////////////////////////////////////////*/
    
    
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
    
    
}