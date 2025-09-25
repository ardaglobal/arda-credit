// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Test} from "forge-std/Test.sol";
import {ARDA} from "../src/ARDA.sol";
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

// Mock SP1 Verifier for testing
contract MockSP1Verifier {
    function verifyProof(bytes32, bytes calldata, bytes calldata) external pure returns (bool) {
        return true;
    }
}

contract UsdcWithdrawalTest is Test {
    ARDA public arda;
    MockUSDC public usdc;
    MockSP1Verifier public verifier;
    
    address public owner = address(this);
    address public user1 = address(0x123);
    address public user2 = address(0x456);
    address public recipient = address(0x789);
    
    bytes32 public constant PROGRAM_VKEY = 0x00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff;
    
    event Withdrawal(
        address indexed user,
        address indexed to,
        uint256 indexed amount,
        uint256 timestamp
    );

    function setUp() public {
        verifier = new MockSP1Verifier();
        arda = new ARDA(address(verifier), PROGRAM_VKEY);
        
        // Deploy mock USDC token
        usdc = new MockUSDC();
        
        // Set USDC token in contract for testing
        arda.setUsdcTokenForTesting(address(usdc));
        
        // Mint USDC to users for testing
        usdc.mint(user1, 1000 * 10**6); // 1000 USDC
        usdc.mint(user2, 2000 * 10**6); // 2000 USDC
        
        // Users need to deposit first before they can withdraw
        vm.startPrank(user1);
        usdc.approve(address(arda), 1000 * 10**6);
        arda.deposit(500 * 10**6); // Deposit 500 USDC
        vm.stopPrank();
        
        vm.startPrank(user2);
        usdc.approve(address(arda), 2000 * 10**6);
        arda.deposit(1000 * 10**6); // Deposit 1000 USDC
        vm.stopPrank();
    }

    function testWithdraw() public {
        uint256 withdrawAmount = 200 * 10**6; // 200 USDC
        
        // Check initial state
        assertEq(arda.getUserDeposit(user1), 500 * 10**6);
        assertEq(usdc.balanceOf(recipient), 0);
        
        // Perform withdrawal
        vm.expectEmit(true, true, true, true);
        emit Withdrawal(user1, recipient, withdrawAmount, block.timestamp);
        
        vm.prank(user1);
        arda.withdraw(withdrawAmount, recipient);
        
        // Verify balances after withdrawal
        assertEq(arda.getUserDeposit(user1), 300 * 10**6); // 500 - 200 = 300
        assertEq(usdc.balanceOf(recipient), withdrawAmount);
        assertEq(arda.getTotalDeposits(), 1300 * 10**6); // Total: 1500 - 200 = 1300
    }

    function testWithdrawAll() public {
        uint256 initialBalance = arda.getUserDeposit(user1);
        
        // Check initial state
        assertEq(initialBalance, 500 * 10**6);
        assertEq(usdc.balanceOf(user1), 500 * 10**6); // Remaining USDC
        
        // Perform withdrawal of all funds to user's own address
        vm.expectEmit(true, true, true, true);
        emit Withdrawal(user1, user1, initialBalance, block.timestamp);
        
        vm.prank(user1);
        arda.withdrawAll();
        
        // Verify balances after withdrawal
        assertEq(arda.getUserDeposit(user1), 0);
        assertEq(usdc.balanceOf(user1), 1000 * 10**6); // Got back to 1000 USDC
        assertEq(arda.getTotalDeposits(), 1000 * 10**6); // Only user2's deposit remains
    }

    function testWithdrawMultipleTimes() public {
        uint256 firstWithdraw = 100 * 10**6;  // 100 USDC
        uint256 secondWithdraw = 150 * 10**6; // 150 USDC
        
        vm.startPrank(user1);
        
        // First withdrawal
        arda.withdraw(firstWithdraw, recipient);
        assertEq(arda.getUserDeposit(user1), 400 * 10**6);
        assertEq(usdc.balanceOf(recipient), firstWithdraw);
        
        // Second withdrawal
        arda.withdraw(secondWithdraw, recipient);
        assertEq(arda.getUserDeposit(user1), 250 * 10**6);
        assertEq(usdc.balanceOf(recipient), firstWithdraw + secondWithdraw);
        
        vm.stopPrank();
    }

    function testRevertWhenZeroWithdrawal() public {
        vm.prank(user1);
        vm.expectRevert("Withdrawal amount must be greater than zero");
        arda.withdraw(0, recipient);
    }

    function testRevertWhenWithdrawToZeroAddress() public {
        vm.prank(user1);
        vm.expectRevert("Cannot withdraw to zero address");
        arda.withdraw(100 * 10**6, address(0));
    }

    function testRevertWhenInsufficientBalance() public {
        uint256 withdrawAmount = 600 * 10**6; // More than deposited (500)
        
        vm.prank(user1);
        vm.expectRevert("Insufficient balance");
        arda.withdraw(withdrawAmount, recipient);
    }

    function testRevertWithdrawAllWhenNoBalance() public {
        // User with no deposits
        address user3 = address(0xabc);
        
        vm.prank(user3);
        vm.expectRevert("No balance to withdraw");
        arda.withdrawAll();
    }

    function testWithdrawAfterPartialWithdrawalAll() public {
        // User1 withdraws partially first
        uint256 partialWithdraw = 200 * 10**6;
        vm.prank(user1);
        arda.withdraw(partialWithdraw, recipient);
        
        uint256 remainingBalance = arda.getUserDeposit(user1);
        assertEq(remainingBalance, 300 * 10**6);
        
        // Then withdraws all remaining
        vm.expectEmit(true, true, true, true);
        emit Withdrawal(user1, user1, remainingBalance, block.timestamp);
        
        vm.prank(user1);
        arda.withdrawAll();
        
        assertEq(arda.getUserDeposit(user1), 0);
    }

    function testContractUsdcBalanceAfterWithdrawals() public {
        uint256 initialContractBalance = arda.getContractUsdcBalance();
        assertEq(initialContractBalance, 1500 * 10**6); // Total deposits
        
        // User1 withdraws 200 USDC
        vm.prank(user1);
        arda.withdraw(200 * 10**6, recipient);
        
        assertEq(arda.getContractUsdcBalance(), 1300 * 10**6);
        
        // User2 withdraws all (1000 USDC)
        vm.prank(user2);
        arda.withdrawAll();
        
        assertEq(arda.getContractUsdcBalance(), 300 * 10**6); // Only user1's remaining balance
    }

    function testWithdrawToSelfVsDifferentAddress() public {
        uint256 withdrawAmount = 100 * 10**6;
        
        // Withdraw to self
        uint256 userBalanceBefore = usdc.balanceOf(user1);
        vm.prank(user1);
        arda.withdraw(withdrawAmount, user1);
        assertEq(usdc.balanceOf(user1), userBalanceBefore + withdrawAmount);
        
        // Withdraw to different address
        uint256 recipientBalanceBefore = usdc.balanceOf(recipient);
        vm.prank(user1);
        arda.withdraw(withdrawAmount, recipient);
        assertEq(usdc.balanceOf(recipient), recipientBalanceBefore + withdrawAmount);
    }

    function testWithdrawEventEmission() public {
        uint256 withdrawAmount = 250 * 10**6;
        
        // Test event emission
        vm.expectEmit(true, true, true, false); // Don't check timestamp exact value
        emit Withdrawal(user2, recipient, withdrawAmount, 0);
        
        vm.prank(user2);
        arda.withdraw(withdrawAmount, recipient);
    }
}