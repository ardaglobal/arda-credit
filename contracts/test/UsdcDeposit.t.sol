// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Test} from "forge-std/Test.sol";
import {Loan} from "../src/Loan.sol";
import {IERC20} from "lib/sp1-contracts/contracts/lib/openzeppelin-contracts/contracts/token/ERC20/IERC20.sol";
import {ERC20} from "lib/sp1-contracts/contracts/lib/openzeppelin-contracts/contracts/token/ERC20/ERC20.sol";

// Mock USDC token for testing
contract MockUSDC is ERC20 {
    constructor() ERC20("USD Coin", "USDC") {}
    
    function mint(address to, uint256 amount) external {
        _mint(to, amount);
    }
    
    function decimals() public view virtual override returns (uint8) {
        return 6;
    }
}

contract UsdcDepositTest is Test {
    Loan public loan;
    MockUSDC public usdc;
    address public verifier = address(0x1); // Mock verifier address
    bytes32 public constant TEST_VKEY = keccak256("test_vkey");
    
    address public user1 = address(0x100);
    address public user2 = address(0x200);
    
    function setUp() public {
        // Deploy mock USDC
        usdc = new MockUSDC();
        
        // Deploy Loan contract with USDC address
        loan = new Loan(verifier, TEST_VKEY, address(usdc));
        
        // Mint USDC to test users
        usdc.mint(user1, 1000 * 10**6); // 1000 USDC
        usdc.mint(user2, 2000 * 10**6); // 2000 USDC
        
        // Approve loan contract to spend users' USDC
        vm.prank(user1);
        usdc.approve(address(loan), type(uint256).max);
        
        vm.prank(user2);
        usdc.approve(address(loan), type(uint256).max);
    }
    
    function testDeposit() public {
        uint256 depositAmount = 100 * 10**6; // 100 USDC
        
        // Record event
        vm.expectEmit(true, true, true, false);
        emit Loan.Deposit(user1, depositAmount, block.timestamp, bytes32(0));
        
        // User1 deposits
        vm.prank(user1);
        loan.deposit(depositAmount);
        
        // Check balances
        assertEq(loan.getUserDeposit(user1), depositAmount);
        assertEq(loan.getTotalDeposits(), depositAmount);
        assertEq(loan.getDepositorCount(), 1);
        assertTrue(loan.hasUserDeposited(user1));
        assertFalse(loan.hasUserDeposited(user2));
    }
    
    function testMultipleDeposits() public {
        uint256 deposit1 = 100 * 10**6; // 100 USDC
        uint256 deposit2 = 50 * 10**6;  // 50 USDC
        
        // First deposit from user1
        vm.prank(user1);
        loan.deposit(deposit1);
        
        // Second deposit from user1
        vm.prank(user1);
        loan.deposit(deposit2);
        
        // Deposit from user2
        vm.prank(user2);
        loan.deposit(200 * 10**6);
        
        // Check user1's total
        assertEq(loan.getUserDeposit(user1), deposit1 + deposit2);
        assertEq(loan.getUserDeposit(user2), 200 * 10**6);
        assertEq(loan.getTotalDeposits(), deposit1 + deposit2 + 200 * 10**6);
        assertEq(loan.getDepositorCount(), 2);
    }
    
    function testGetAllDepositors() public {
        // Make deposits
        vm.prank(user1);
        loan.deposit(100 * 10**6);
        
        vm.prank(user2);
        loan.deposit(200 * 10**6);
        
        // Get all depositors
        (address[] memory users, uint256[] memory amounts) = loan.getAllDepositors();
        
        assertEq(users.length, 2);
        assertEq(amounts.length, 2);
        assertEq(users[0], user1);
        assertEq(users[1], user2);
        assertEq(amounts[0], 100 * 10**6);
        assertEq(amounts[1], 200 * 10**6);
    }
    
    function testGetDepositorByIndex() public {
        vm.prank(user1);
        loan.deposit(100 * 10**6);
        
        (address user, uint256 amount) = loan.getDepositorByIndex(0);
        assertEq(user, user1);
        assertEq(amount, 100 * 10**6);
    }
    
    function testRevertWhenZeroDeposit() public {
        vm.prank(user1);
        vm.expectRevert("Deposit amount must be greater than zero");
        loan.deposit(0);
    }
    
    function testRevertWhenInsufficientBalance() public {
        vm.prank(user1);
        vm.expectRevert(); // ERC20 insufficient balance revert
        loan.deposit(2000 * 10**6); // User1 only has 1000 USDC
    }
    
    function testContractBalance() public {
        uint256 depositAmount = 100 * 10**6;
        
        uint256 contractBalanceBefore = loan.getContractUsdcBalance();
        
        vm.prank(user1);
        loan.deposit(depositAmount);
        
        uint256 contractBalanceAfter = loan.getContractUsdcBalance();
        
        assertEq(contractBalanceAfter, contractBalanceBefore + depositAmount);
    }
    
    function testUsdcTokenAddress() public {
        assertEq(loan.getUsdcTokenAddress(), address(usdc));
    }
}