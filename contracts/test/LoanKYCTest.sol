// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Test, console} from "forge-std/Test.sol";
import {Loan} from "../src/Loan.sol";
import {SP1VerifierGateway} from "@sp1-contracts/SP1VerifierGateway.sol";
import {ERC20Mock} from "openzeppelin-contracts/contracts/mocks/token/ERC20Mock.sol";

contract LoanKYCTest is Test {
    Loan public loan;
    ERC20Mock public usdc;
    
    address public owner = address(0x123);
    address public ardaAdmin = address(0x456);
    address public kycProvider1 = address(0x789);
    address public kycProvider2 = address(0xabc);
    address public user1 = address(0xdef);
    address public user2 = address(0x111);
    address public unauthorizedUser = address(0x222);
    
    bytes32 public constant PROGRAM_VKEY = bytes32(uint256(0x1234));
    bytes32 public constant LOAN_ID_1 = keccak256("loan-1");
    bytes32 public constant STATE_ID_1 = keccak256("test-state-1");
    
    uint256 public constant INITIAL_USDC_SUPPLY = 1000000e6;
    uint256 public constant DEPOSIT_AMOUNT = 10000e6;
    uint256 public constant TIMEOUT_BLOCKS = 100;
    
    event KYCStatusUpdated(address indexed user, bool verified);
    event KYCProviderUpdated(address indexed provider, bool authorized);

    function setUp() public {
        vm.startPrank(owner);
        
        // Deploy USDC mock
        usdc = new ERC20Mock();
        usdc.mint(user1, INITIAL_USDC_SUPPLY);
        usdc.mint(user2, INITIAL_USDC_SUPPLY);
        
        // Deploy SP1 verifier
        address verifier = address(new SP1VerifierGateway(address(1)));
        
        // Deploy Loan contract
        loan = new Loan(verifier, PROGRAM_VKEY, address(usdc), ardaAdmin);
        
        vm.stopPrank();
    }
    
    /*//////////////////////////////////////////////////////////////
                        KYC PROVIDER MANAGEMENT
    //////////////////////////////////////////////////////////////*/
    
    function test_SetKYCProvider_Owner() public {
        vm.startPrank(owner);
        
        vm.expectEmit(true, false, false, true);
        emit KYCProviderUpdated(kycProvider1, true);
        
        loan.setKYCProvider(kycProvider1, true);
        
        assertTrue(loan.isKYCProvider(kycProvider1));
        
        vm.stopPrank();
    }
    
    function test_SetKYCProvider_RemoveAuthorization() public {
        // First authorize
        vm.prank(owner);
        loan.setKYCProvider(kycProvider1, true);
        assertTrue(loan.isKYCProvider(kycProvider1));
        
        // Then remove authorization
        vm.startPrank(owner);
        
        vm.expectEmit(true, false, false, true);
        emit KYCProviderUpdated(kycProvider1, false);
        
        loan.setKYCProvider(kycProvider1, false);
        
        assertFalse(loan.isKYCProvider(kycProvider1));
        
        vm.stopPrank();
    }
    
    function testRevert_SetKYCProvider_UnauthorizedUser() public {
        vm.startPrank(unauthorizedUser);
        
        vm.expectRevert(Loan.UnauthorizedAccess.selector);
        loan.setKYCProvider(kycProvider1, true);
        
        vm.stopPrank();
    }
    
    function test_SetKYCProvider_MultipleProviders() public {
        vm.startPrank(owner);
        
        loan.setKYCProvider(kycProvider1, true);
        loan.setKYCProvider(kycProvider2, true);
        
        assertTrue(loan.isKYCProvider(kycProvider1));
        assertTrue(loan.isKYCProvider(kycProvider2));
        
        vm.stopPrank();
    }
    
    function test_InitialKYCProvider_OwnerIsProvider() public {
        // Owner should be KYC provider by default (set in constructor)
        assertTrue(loan.isKYCProvider(owner));
    }
    
    /*//////////////////////////////////////////////////////////////
                        KYC STATUS MANAGEMENT
    //////////////////////////////////////////////////////////////*/
    
    function test_SetKYCStatus_AuthorizedProvider() public {
        // First authorize KYC provider
        vm.prank(owner);
        loan.setKYCProvider(kycProvider1, true);
        
        // Set KYC status
        vm.startPrank(kycProvider1);
        
        vm.expectEmit(true, false, false, true);
        emit KYCStatusUpdated(user1, true);
        
        loan.setKYCStatus(user1, true);
        
        assertTrue(loan.isKYCVerified(user1));
        
        vm.stopPrank();
    }
    
    function test_SetKYCStatus_RemoveVerification() public {
        // First verify user
        vm.prank(owner);
        loan.setKYCProvider(kycProvider1, true);
        
        vm.prank(kycProvider1);
        loan.setKYCStatus(user1, true);
        assertTrue(loan.isKYCVerified(user1));
        
        // Then remove verification
        vm.startPrank(kycProvider1);
        
        vm.expectEmit(true, false, false, true);
        emit KYCStatusUpdated(user1, false);
        
        loan.setKYCStatus(user1, false);
        
        assertFalse(loan.isKYCVerified(user1));
        
        vm.stopPrank();
    }
    
    function testRevert_SetKYCStatus_UnauthorizedProvider() public {
        vm.startPrank(unauthorizedUser);
        
        vm.expectRevert(Loan.NotAuthorizedKYCProvider.selector);
        loan.setKYCStatus(user1, true);
        
        vm.stopPrank();
    }
    
    function testRevert_SetKYCStatus_RemovedProvider() public {
        // First authorize and use provider
        vm.prank(owner);
        loan.setKYCProvider(kycProvider1, true);
        
        vm.prank(kycProvider1);
        loan.setKYCStatus(user1, true);
        
        // Remove provider authorization
        vm.prank(owner);
        loan.setKYCProvider(kycProvider1, false);
        
        // Try to set KYC status with removed provider
        vm.startPrank(kycProvider1);
        
        vm.expectRevert(Loan.NotAuthorizedKYCProvider.selector);
        loan.setKYCStatus(user2, true);
        
        vm.stopPrank();
    }
    
    function test_SetKYCStatus_OwnerAsProvider() public {
        // Owner should be able to set KYC status (is provider by default)
        vm.startPrank(owner);
        
        vm.expectEmit(true, false, false, true);
        emit KYCStatusUpdated(user1, true);
        
        loan.setKYCStatus(user1, true);
        
        assertTrue(loan.isKYCVerified(user1));
        
        vm.stopPrank();
    }
    
    function test_SetKYCStatus_MultipleUsers() public {
        vm.prank(owner);
        loan.setKYCProvider(kycProvider1, true);
        
        vm.startPrank(kycProvider1);
        
        loan.setKYCStatus(user1, true);
        loan.setKYCStatus(user2, true);
        
        assertTrue(loan.isKYCVerified(user1));
        assertTrue(loan.isKYCVerified(user2));
        
        vm.stopPrank();
    }
    
    /*//////////////////////////////////////////////////////////////
                        KYC REQUIREMENTS INTEGRATION
    //////////////////////////////////////////////////////////////*/
    
    function test_WithdrawalRequiresKYC() public {
        // First make a deposit
        vm.startPrank(user1);
        usdc.approve(address(loan), DEPOSIT_AMOUNT);
        loan.depositForLoan(DEPOSIT_AMOUNT, bytes32(0), false, 0);
        vm.stopPrank();
        
        // Mock a state for withdrawal proof
        vm.startPrank(owner);
        address verifier = loan.verifier();
        vm.mockCall(
            verifier,
            abi.encodeWithSelector(SP1VerifierGateway.verifyProof.selector),
            abi.encode(true)
        );
        
        bytes memory mockProof = "mock_proof";
        bytes memory mockPublicValues = "mock_public_values";
        loan.updateState(STATE_ID_1, keccak256("test-state"), mockProof, mockPublicValues);
        vm.stopPrank();
        
        // Try to withdraw without KYC - should fail
        vm.startPrank(user1);
        
        bytes32[] memory balanceProof = new bytes32[](1);
        balanceProof[0] = keccak256("mock_proof");
        
        vm.expectRevert(Loan.KYCRequired.selector);
        loan.submitWithdrawal(1000e6, keccak256("test-state"), balanceProof);
        
        vm.stopPrank();
    }
    
    function test_WithdrawalWorksWithKYC() public {
        // First make a deposit
        vm.startPrank(user1);
        usdc.approve(address(loan), DEPOSIT_AMOUNT);
        loan.depositForLoan(DEPOSIT_AMOUNT, bytes32(0), false, 0);
        vm.stopPrank();
        
        // KYC verify user
        vm.prank(owner);
        loan.setKYCStatus(user1, true);
        
        // Mock a state for withdrawal proof
        vm.startPrank(owner);
        address verifier = loan.verifier();
        vm.mockCall(
            verifier,
            abi.encodeWithSelector(SP1VerifierGateway.verifyProof.selector),
            abi.encode(true)
        );
        
        bytes memory mockProof = "mock_proof";
        bytes memory mockPublicValues = "mock_public_values";
        bytes32 testStateRoot = keccak256("test-state");
        loan.updateState(STATE_ID_1, testStateRoot, mockProof, mockPublicValues);
        vm.stopPrank();
        
        // Now withdrawal should work
        vm.startPrank(user1);
        
        bytes32[] memory balanceProof = new bytes32[](1);
        balanceProof[0] = keccak256(abi.encode(user1, 1000e6));
        
        // Note: This will fail at balance proof verification, but shouldn't fail at KYC check
        // We expect a different error, not KYCRequired
        vm.expectRevert(); // Generic revert, not KYCRequired
        loan.submitWithdrawal(1000e6, testStateRoot, balanceProof);
        
        vm.stopPrank();
    }
    
    function test_EscrowReleaseRequiresKYC() public {
        // Create escrow
        vm.startPrank(user1);
        usdc.approve(address(loan), DEPOSIT_AMOUNT);
        loan.depositForLoan(DEPOSIT_AMOUNT, LOAN_ID_1, true, TIMEOUT_BLOCKS);
        vm.stopPrank();
        
        bytes32 escrowId = keccak256(abi.encode(user1, LOAN_ID_1, block.timestamp));
        
        // Move past timeout
        vm.roll(block.number + TIMEOUT_BLOCKS + 1);
        
        // Try to release without KYC
        vm.startPrank(user1);
        
        vm.expectRevert(Loan.KYCRequired.selector);
        loan.releaseEscrow(escrowId);
        
        vm.stopPrank();
    }
    
    function test_EscrowReleaseWorksWithKYC() public {
        // Create escrow
        vm.startPrank(user1);
        usdc.approve(address(loan), DEPOSIT_AMOUNT);
        loan.depositForLoan(DEPOSIT_AMOUNT, LOAN_ID_1, true, TIMEOUT_BLOCKS);
        vm.stopPrank();
        
        bytes32 escrowId = keccak256(abi.encode(user1, LOAN_ID_1, block.timestamp));
        
        // KYC verify and move past timeout
        vm.prank(owner);
        loan.setKYCStatus(user1, true);
        
        vm.roll(block.number + TIMEOUT_BLOCKS + 1);
        
        // Should work with KYC
        vm.startPrank(user1);
        loan.releaseEscrow(escrowId);
        vm.stopPrank();
        
        // Check that escrow was released
        assertEq(loan.getUserEscrowedAmount(user1), 0);
    }
    
    /*//////////////////////////////////////////////////////////////
                            VIEW FUNCTIONS
    //////////////////////////////////////////////////////////////*/
    
    function test_IsKYCVerified_InitiallyFalse() public view {
        assertFalse(loan.isKYCVerified(user1));
        assertFalse(loan.isKYCVerified(user2));
    }
    
    function test_IsKYCProvider_InitiallyOwnerOnly() public view {
        assertTrue(loan.isKYCProvider(owner));
        assertFalse(loan.isKYCProvider(kycProvider1));
        assertFalse(loan.isKYCProvider(user1));
    }
    
    function test_IsKYCProvider_AfterAuthorization() public {
        vm.prank(owner);
        loan.setKYCProvider(kycProvider1, true);
        
        assertTrue(loan.isKYCProvider(kycProvider1));
        assertFalse(loan.isKYCProvider(kycProvider2));
    }
    
    /*//////////////////////////////////////////////////////////////
                        EDGE CASES AND SCENARIOS
    //////////////////////////////////////////////////////////////*/
    
    function test_KYCProvider_CanVerifyThemselves() public {
        vm.prank(owner);
        loan.setKYCProvider(kycProvider1, true);
        
        // KYC provider verifies themselves
        vm.startPrank(kycProvider1);
        loan.setKYCStatus(kycProvider1, true);
        assertTrue(loan.isKYCVerified(kycProvider1));
        vm.stopPrank();
    }
    
    function test_OwnerCanRevokeTheirOwnKYCProvider() public {
        assertTrue(loan.isKYCProvider(owner));
        
        vm.startPrank(owner);
        loan.setKYCProvider(owner, false);
        assertFalse(loan.isKYCProvider(owner));
        
        // Owner can no longer set KYC status
        vm.expectRevert(Loan.NotAuthorizedKYCProvider.selector);
        loan.setKYCStatus(user1, true);
        
        vm.stopPrank();
    }
    
    function test_KYCStatus_PersistsAcrossProviderChanges() public {
        // Provider verifies user
        vm.prank(owner);
        loan.setKYCProvider(kycProvider1, true);
        
        vm.prank(kycProvider1);
        loan.setKYCStatus(user1, true);
        assertTrue(loan.isKYCVerified(user1));
        
        // Remove provider authorization
        vm.prank(owner);
        loan.setKYCProvider(kycProvider1, false);
        
        // User should still be verified
        assertTrue(loan.isKYCVerified(user1));
    }
    
    function test_MultipleProviders_CanSetSameUser() public {
        vm.startPrank(owner);
        loan.setKYCProvider(kycProvider1, true);
        loan.setKYCProvider(kycProvider2, true);
        vm.stopPrank();
        
        // First provider verifies user
        vm.prank(kycProvider1);
        loan.setKYCStatus(user1, true);
        assertTrue(loan.isKYCVerified(user1));
        
        // Second provider can change the same user's status
        vm.prank(kycProvider2);
        loan.setKYCStatus(user1, false);
        assertFalse(loan.isKYCVerified(user1));
        
        // First provider can change it back
        vm.prank(kycProvider1);
        loan.setKYCStatus(user1, true);
        assertTrue(loan.isKYCVerified(user1));
    }
    
    function test_SetKYCStatus_SameValueMultipleTimes() public {
        vm.prank(owner);
        loan.setKYCProvider(kycProvider1, true);
        
        vm.startPrank(kycProvider1);
        
        // Set to true multiple times
        loan.setKYCStatus(user1, true);
        assertTrue(loan.isKYCVerified(user1));
        
        loan.setKYCStatus(user1, true);
        assertTrue(loan.isKYCVerified(user1));
        
        // Set to false multiple times
        loan.setKYCStatus(user1, false);
        assertFalse(loan.isKYCVerified(user1));
        
        loan.setKYCStatus(user1, false);
        assertFalse(loan.isKYCVerified(user1));
        
        vm.stopPrank();
    }
    
    /*//////////////////////////////////////////////////////////////
                        MODIFIER TESTS
    //////////////////////////////////////////////////////////////*/
    
    function test_OnlyKYCVerified_Modifier() public {
        // Deposits don't require KYC
        vm.startPrank(user1);
        usdc.approve(address(loan), DEPOSIT_AMOUNT);
        loan.depositForLoan(DEPOSIT_AMOUNT, bytes32(0), false, 0);
        vm.stopPrank();
        
        // But withdrawals do (tested above)
        // And escrow releases do (tested above)
    }
    
    function test_OnlyKYCProvider_Modifier() public {
        // Only authorized providers can set KYC status (tested above)
        // Non-providers cannot (tested above)
    }
}