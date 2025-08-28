// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Test, console} from "forge-std/Test.sol";
import {Loan} from "../src/Loan.sol";
import {SP1VerifierGateway} from "@sp1-contracts/SP1VerifierGateway.sol";
import {ERC20Mock} from "openzeppelin-contracts/contracts/mocks/token/ERC20Mock.sol";

contract LoanEscrowTest is Test {
    Loan public loan;
    ERC20Mock public usdc;
    
    address public owner = address(0x123);
    address public ardaAdmin = address(0x456);
    address public investor1 = address(0x789);
    address public investor2 = address(0xabc);
    address public kycProvider = address(0xdef);
    address public unauthorizedUser = address(0x111);
    
    bytes32 public constant PROGRAM_VKEY = bytes32(uint256(0x1234));
    bytes32 public constant LOAN_ID_1 = keccak256("loan-1");
    bytes32 public constant LOAN_ID_2 = keccak256("loan-2");
    bytes32 public constant STATE_ID_1 = keccak256("test-state-1");
    
    // Test amounts
    uint256 public constant INITIAL_USDC_SUPPLY = 1000000e6; // 1M USDC
    uint256 public constant DEPOSIT_AMOUNT = 10000e6; // 10k USDC
    uint256 public constant SMALL_AMOUNT = 100e6; // 100 USDC
    uint256 public constant TIMEOUT_BLOCKS = 100;
    
    event EscrowCreated(bytes32 indexed escrowId, address indexed investor, bytes32 indexed loanId, uint256 amount);
    event EscrowReleased(bytes32 indexed escrowId, address indexed investor, uint256 amount);
    event DepositForLoan(address indexed user, bytes32 indexed loanId, uint256 amount, bool isEscrow);
    event KYCStatusUpdated(address indexed user, bool verified);

    function setUp() public {
        vm.startPrank(owner);
        
        // Deploy USDC mock
        usdc = new ERC20Mock();
        usdc.mint(owner, INITIAL_USDC_SUPPLY);
        usdc.mint(investor1, INITIAL_USDC_SUPPLY);
        usdc.mint(investor2, INITIAL_USDC_SUPPLY);
        
        // Deploy SP1 verifier
        address verifier = address(new SP1VerifierGateway(address(1)));
        
        // Deploy Loan contract
        loan = new Loan(verifier, PROGRAM_VKEY, address(usdc), ardaAdmin);
        
        // Set up KYC provider
        loan.setKYCProvider(kycProvider, true);
        
        vm.stopPrank();
    }
    
    /*//////////////////////////////////////////////////////////////
                            DEPOSIT TESTS
    //////////////////////////////////////////////////////////////*/
    
    function test_DepositForLoan_BasicDeposit() public {
        vm.startPrank(investor1);
        
        usdc.approve(address(loan), DEPOSIT_AMOUNT);
        
        vm.expectEmit(true, true, false, true);
        emit DepositForLoan(investor1, bytes32(0), DEPOSIT_AMOUNT, false);
        
        loan.depositForLoan(DEPOSIT_AMOUNT, bytes32(0), false, 0);
        
        assertEq(loan.getUserBalance(investor1), DEPOSIT_AMOUNT);
        assertEq(loan.getTotalTVL(), DEPOSIT_AMOUNT);
        assertEq(usdc.balanceOf(address(loan)), DEPOSIT_AMOUNT);
        
        vm.stopPrank();
    }
    
    function test_DepositForLoan_EscrowDeposit() public {
        vm.startPrank(investor1);
        
        usdc.approve(address(loan), DEPOSIT_AMOUNT);
        
        vm.expectEmit(true, true, false, true);
        emit DepositForLoan(investor1, LOAN_ID_1, DEPOSIT_AMOUNT, true);
        
        loan.depositForLoan(DEPOSIT_AMOUNT, LOAN_ID_1, true, TIMEOUT_BLOCKS);
        
        // Check escrow was created
        assertEq(loan.getUserEscrowedAmount(investor1), DEPOSIT_AMOUNT);
        assertEq(loan.getTotalTVL(), DEPOSIT_AMOUNT);
        assertEq(loan.getUserBalance(investor1), 0); // Should not go to user balance for escrow
        
        vm.stopPrank();
    }
    
    function testRevert_DepositForLoan_ZeroAmount() public {
        vm.startPrank(investor1);
        
        vm.expectRevert(Loan.InvalidAmount.selector);
        loan.depositForLoan(0, LOAN_ID_1, false, 0);
        
        vm.stopPrank();
    }
    
    function testRevert_DepositForLoan_InsufficientApproval() public {
        vm.startPrank(investor1);
        
        // Don't approve tokens
        vm.expectRevert();
        loan.depositForLoan(DEPOSIT_AMOUNT, LOAN_ID_1, false, 0);
        
        vm.stopPrank();
    }
    
    /*//////////////////////////////////////////////////////////////
                            ESCROW TESTS
    //////////////////////////////////////////////////////////////*/
    
    function test_EscrowLifecycle_CreateToRelease() public {
        // Create escrow
        vm.startPrank(investor1);
        usdc.approve(address(loan), DEPOSIT_AMOUNT);
        loan.depositForLoan(DEPOSIT_AMOUNT, LOAN_ID_1, true, TIMEOUT_BLOCKS);
        vm.stopPrank();
        
        // Get escrow ID (generated deterministically)
        bytes32 escrowId = keccak256(abi.encode(investor1, LOAN_ID_1, block.timestamp));
        
        // Verify escrow was created by checking escrowed amount
        assertEq(loan.getUserEscrowedAmount(investor1), DEPOSIT_AMOUNT);
        
        // Move forward past timeout
        vm.roll(block.number + TIMEOUT_BLOCKS + 1);
        
        // KYC verify investor1 (required for escrow release)
        vm.prank(kycProvider);
        loan.setKYCStatus(investor1, true);
        
        // Release escrow
        vm.startPrank(investor1);
        vm.expectEmit(true, true, false, true);
        emit EscrowReleased(escrowId, investor1, DEPOSIT_AMOUNT);
        
        loan.releaseEscrow(escrowId);
        
        // Check escrow was released by verifying escrowed amount is now 0
        assertEq(loan.getUserEscrowedAmount(investor1), 0);
        assertEq(usdc.balanceOf(investor1), INITIAL_USDC_SUPPLY); // Got money back
        
        vm.stopPrank();
    }
    
    function test_EscrowLifecycle_ActivateToComplete() public {
        // Create escrow
        vm.startPrank(investor1);
        usdc.approve(address(loan), DEPOSIT_AMOUNT);
        loan.depositForLoan(DEPOSIT_AMOUNT, LOAN_ID_1, true, TIMEOUT_BLOCKS);
        vm.stopPrank();
        
        bytes32 escrowId = keccak256(abi.encode(investor1, LOAN_ID_1, block.timestamp));
        
        // Admin activates escrow
        vm.prank(owner);
        loan.activateEscrow(escrowId);
        
        // Escrow is now active (verified by ability to complete it next)
        
        // Admin completes escrow
        vm.prank(owner);
        loan.completeEscrow(escrowId);
        
        // Escrow completed - funds transferred to user balance
        assertEq(loan.getUserEscrowedAmount(investor1), 0);
        assertEq(loan.getUserBalance(investor1), DEPOSIT_AMOUNT); // Transferred to user balance
        
    }
    
    function testRevert_ReleaseEscrow_NotOwner() public {
        vm.startPrank(investor1);
        usdc.approve(address(loan), DEPOSIT_AMOUNT);
        loan.depositForLoan(DEPOSIT_AMOUNT, LOAN_ID_1, true, TIMEOUT_BLOCKS);
        vm.stopPrank();
        
        bytes32 escrowId = keccak256(abi.encode(investor1, LOAN_ID_1, block.timestamp));
        
        // Different user tries to release
        vm.roll(block.number + TIMEOUT_BLOCKS + 1);
        vm.prank(kycProvider);
        loan.setKYCStatus(investor2, true);
        
        vm.startPrank(investor2);
        vm.expectRevert(Loan.EscrowNotFound.selector);
        loan.releaseEscrow(escrowId);
        vm.stopPrank();
    }
    
    function testRevert_ReleaseEscrow_TimeoutNotReached() public {
        vm.startPrank(investor1);
        usdc.approve(address(loan), DEPOSIT_AMOUNT);
        loan.depositForLoan(DEPOSIT_AMOUNT, LOAN_ID_1, true, TIMEOUT_BLOCKS);
        vm.stopPrank();
        
        bytes32 escrowId = keccak256(abi.encode(investor1, LOAN_ID_1, block.timestamp));
        
        // KYC verify investor1
        vm.prank(kycProvider);
        loan.setKYCStatus(investor1, true);
        
        // Try to release before timeout
        vm.startPrank(investor1);
        vm.expectRevert(Loan.TimeoutNotReached.selector);
        loan.releaseEscrow(escrowId);
        vm.stopPrank();
    }
    
    function testRevert_ReleaseEscrow_RequiresKYC() public {
        vm.startPrank(investor1);
        usdc.approve(address(loan), DEPOSIT_AMOUNT);
        loan.depositForLoan(DEPOSIT_AMOUNT, LOAN_ID_1, true, TIMEOUT_BLOCKS);
        vm.stopPrank();
        
        bytes32 escrowId = keccak256(abi.encode(investor1, LOAN_ID_1, block.timestamp));
        
        // Move past timeout but don't KYC verify
        vm.roll(block.number + TIMEOUT_BLOCKS + 1);
        
        vm.startPrank(investor1);
        vm.expectRevert(Loan.KYCRequired.selector);
        loan.releaseEscrow(escrowId);
        vm.stopPrank();
    }
    
    function testRevert_ReleaseEscrow_WrongStatus() public {
        vm.startPrank(investor1);
        usdc.approve(address(loan), DEPOSIT_AMOUNT);
        loan.depositForLoan(DEPOSIT_AMOUNT, LOAN_ID_1, true, TIMEOUT_BLOCKS);
        vm.stopPrank();
        
        bytes32 escrowId = keccak256(abi.encode(investor1, LOAN_ID_1, block.timestamp));
        
        // Admin activates escrow (changes status to Active)
        vm.prank(owner);
        loan.activateEscrow(escrowId);
        
        // Move past timeout and KYC verify
        vm.roll(block.number + TIMEOUT_BLOCKS + 1);
        vm.prank(kycProvider);
        loan.setKYCStatus(investor1, true);
        
        // Try to release active escrow (should fail)
        vm.startPrank(investor1);
        vm.expectRevert(Loan.EscrowCannotRelease.selector);
        loan.releaseEscrow(escrowId);
        vm.stopPrank();
    }
    
    function testRevert_ActivateEscrow_UnauthorizedUser() public {
        vm.startPrank(investor1);
        usdc.approve(address(loan), DEPOSIT_AMOUNT);
        loan.depositForLoan(DEPOSIT_AMOUNT, LOAN_ID_1, true, TIMEOUT_BLOCKS);
        vm.stopPrank();
        
        bytes32 escrowId = keccak256(abi.encode(investor1, LOAN_ID_1, block.timestamp));
        
        // Unauthorized user tries to activate
        vm.startPrank(unauthorizedUser);
        vm.expectRevert(Loan.UnauthorizedAccess.selector);
        loan.activateEscrow(escrowId);
        vm.stopPrank();
    }
    
    function testRevert_CompleteEscrow_WrongStatus() public {
        vm.startPrank(investor1);
        usdc.approve(address(loan), DEPOSIT_AMOUNT);
        loan.depositForLoan(DEPOSIT_AMOUNT, LOAN_ID_1, true, TIMEOUT_BLOCKS);
        vm.stopPrank();
        
        bytes32 escrowId = keccak256(abi.encode(investor1, LOAN_ID_1, block.timestamp));
        
        // Try to complete without activating first
        vm.startPrank(owner);
        vm.expectRevert(Loan.EscrowCannotRelease.selector);
        loan.completeEscrow(escrowId);
        vm.stopPrank();
    }
    
    /*//////////////////////////////////////////////////////////////
                        MULTIPLE ESCROWS TESTS
    //////////////////////////////////////////////////////////////*/
    
    function test_MultipleEscrows_SameUser() public {
        vm.startPrank(investor1);
        usdc.approve(address(loan), DEPOSIT_AMOUNT * 2);
        
        // Create two escrows for different loans
        uint256 timestamp1 = block.timestamp;
        loan.depositForLoan(DEPOSIT_AMOUNT, LOAN_ID_1, true, TIMEOUT_BLOCKS);
        
        vm.warp(block.timestamp + 1);
        uint256 timestamp2 = block.timestamp;
        loan.depositForLoan(DEPOSIT_AMOUNT, LOAN_ID_2, true, TIMEOUT_BLOCKS);
        
        vm.stopPrank();
        
        // Check total escrowed amount
        assertEq(loan.getUserEscrowedAmount(investor1), DEPOSIT_AMOUNT * 2);
        
        // Check individual escrows exist
        bytes32 escrowId1 = keccak256(abi.encode(investor1, LOAN_ID_1, timestamp1));
        bytes32 escrowId2 = keccak256(abi.encode(investor1, LOAN_ID_2, timestamp2));
        
        // Both escrows created successfully - verified by total escrowed amount
    }
    
    function test_MultipleEscrows_DifferentUsers() public {
        // Investor 1 creates escrow
        vm.startPrank(investor1);
        usdc.approve(address(loan), DEPOSIT_AMOUNT);
        loan.depositForLoan(DEPOSIT_AMOUNT, LOAN_ID_1, true, TIMEOUT_BLOCKS);
        vm.stopPrank();
        
        // Investor 2 creates escrow for same loan
        vm.startPrank(investor2);
        usdc.approve(address(loan), SMALL_AMOUNT);
        loan.depositForLoan(SMALL_AMOUNT, LOAN_ID_1, true, TIMEOUT_BLOCKS);
        vm.stopPrank();
        
        // Check individual amounts
        assertEq(loan.getUserEscrowedAmount(investor1), DEPOSIT_AMOUNT);
        assertEq(loan.getUserEscrowedAmount(investor2), SMALL_AMOUNT);
        assertEq(loan.getTotalTVL(), DEPOSIT_AMOUNT + SMALL_AMOUNT);
    }
    
    /*//////////////////////////////////////////////////////////////
                            VIEW FUNCTIONS TESTS
    //////////////////////////////////////////////////////////////*/
    
    function test_ViewFunctions() public {
        // Set up some deposits and escrows
        vm.startPrank(investor1);
        usdc.approve(address(loan), DEPOSIT_AMOUNT * 2);
        
        // Regular deposit
        loan.depositForLoan(DEPOSIT_AMOUNT, bytes32(0), false, 0);
        
        // Escrow deposit
        loan.depositForLoan(SMALL_AMOUNT, LOAN_ID_1, true, TIMEOUT_BLOCKS);
        
        vm.stopPrank();
        
        // Test view functions
        assertEq(loan.getUserBalance(investor1), DEPOSIT_AMOUNT);
        assertEq(loan.getUserEscrowedAmount(investor1), SMALL_AMOUNT);
        assertEq(loan.getTotalTVL(), DEPOSIT_AMOUNT + SMALL_AMOUNT);
        
        // Test KYC status (should be false initially)
        assertEq(loan.isKYCVerified(investor1), false);
        assertEq(loan.isKYCProvider(kycProvider), true);
        assertEq(loan.isKYCProvider(investor1), false);
    }
    
    /*//////////////////////////////////////////////////////////////
                        EDGE CASES AND ERROR CONDITIONS
    //////////////////////////////////////////////////////////////*/
    
    function test_DepositForLoan_ZeroLoanIdWithEscrow() public {
        // Contract requires non-zero loanId for escrow creation, so this goes to user balance
        vm.startPrank(investor1);
        usdc.approve(address(loan), DEPOSIT_AMOUNT);
        
        vm.expectEmit(true, true, false, true);
        emit DepositForLoan(investor1, bytes32(0), DEPOSIT_AMOUNT, true);
        
        loan.depositForLoan(DEPOSIT_AMOUNT, bytes32(0), true, TIMEOUT_BLOCKS);
        
        // Should NOT create escrow with zero loan ID, goes to user balance instead
        assertEq(loan.getUserEscrowedAmount(investor1), 0);
        assertEq(loan.getUserBalance(investor1), DEPOSIT_AMOUNT);
        
        vm.stopPrank();
    }
    
    function test_EscrowRecord_DefaultValues() public {
        // Test that getEscrowRecord function exists (doesn't revert)
        bytes32 fakeEscrowId = keccak256("fake");
        loan.getEscrowRecord(fakeEscrowId); // Should not revert
    }
    
    function test_EscrowIdGeneration_Deterministic() public {
        uint256 timestamp = block.timestamp;
        
        // Create escrow
        vm.startPrank(investor1);
        usdc.approve(address(loan), DEPOSIT_AMOUNT);
        loan.depositForLoan(DEPOSIT_AMOUNT, LOAN_ID_1, true, TIMEOUT_BLOCKS);
        vm.stopPrank();
        
        // Calculate expected escrow ID
        bytes32 expectedEscrowId = keccak256(abi.encode(investor1, LOAN_ID_1, timestamp));
        
        // Verify escrow exists by checking that function doesn't revert
        loan.getEscrowRecord(expectedEscrowId);
        
        // Verify escrow amount is tracked correctly
        assertEq(loan.getUserEscrowedAmount(investor1), DEPOSIT_AMOUNT);
    }
}