// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {IERC20} from "lib/sp1-contracts/contracts/lib/openzeppelin-contracts/contracts/token/ERC20/IERC20.sol";
import {SafeERC20} from "lib/sp1-contracts/contracts/lib/openzeppelin-contracts/contracts/token/ERC20/utils/SafeERC20.sol";

/// @title ARDA Faucet
/// @notice Rate-limited faucet for distributing test USDC tokens in local/staging environments
/// @dev Designed for local Anvil devnet testing - DO NOT use in production
contract ARDAFaucet {
    using SafeERC20 for IERC20;

    /*//////////////////////////////////////////////////////////////
                                CONSTANTS
    //////////////////////////////////////////////////////////////*/

    /// @notice Amount distributed per request (1M USDC with 6 decimals)
    uint256 public constant FAUCET_AMOUNT = 1_000_000 * 10**6;

    /// @notice Cooldown period between requests (1 hour)
    uint256 public constant COOLDOWN_PERIOD = 1 hours;

    /*//////////////////////////////////////////////////////////////
                                 STATE
    //////////////////////////////////////////////////////////////*/

    /// @notice USDC token contract
    IERC20 public immutable usdc;

    /// @notice Contract owner for management functions
    address public owner;

    /// @notice Last request timestamp for each address
    mapping(address => uint256) public lastRequestTime;

    /// @notice Total amount distributed per address
    mapping(address => uint256) public totalDistributed;

    /// @notice Total addresses served
    uint256 public uniqueRecipients;

    /// @notice Total amount distributed overall
    uint256 public totalDistributedOverall;

    /// @notice Whether the faucet is enabled
    bool public enabled;

    /*//////////////////////////////////////////////////////////////
                                EVENTS
    //////////////////////////////////////////////////////////////*/

    event FundsRequested(
        address indexed recipient,
        uint256 amount,
        uint256 timestamp
    );

    event FaucetRefilled(address indexed funder, uint256 amount);
    event FaucetEnabled(bool enabled);
    event OwnershipTransferred(
        address indexed previousOwner,
        address indexed newOwner
    );

    /*//////////////////////////////////////////////////////////////
                                 ERRORS
    //////////////////////////////////////////////////////////////*/

    error CooldownNotElapsed(uint256 remainingTime);
    error FaucetDisabled();
    error InsufficientFaucetBalance(uint256 requested, uint256 available);
    error Unauthorized();
    error ZeroAddress();

    /*//////////////////////////////////////////////////////////////
                              CONSTRUCTOR
    //////////////////////////////////////////////////////////////*/

    /// @notice Initialize the faucet with USDC token address
    /// @param _usdcToken Address of the USDC token contract
    constructor(address _usdcToken) {
        if (_usdcToken == address(0)) revert ZeroAddress();

        usdc = IERC20(_usdcToken);
        owner = msg.sender;
        enabled = true;
    }

    /*//////////////////////////////////////////////////////////////
                            FAUCET FUNCTIONS
    //////////////////////////////////////////////////////////////*/

    /// @notice Request faucet funds (rate-limited to once per hour)
    /// @dev Transfers FAUCET_AMOUNT USDC to caller if cooldown elapsed
    function requestFunds() external {
        if (!enabled) revert FaucetDisabled();

        uint256 timeSinceLastRequest = block.timestamp - lastRequestTime[msg.sender];

        if (timeSinceLastRequest < COOLDOWN_PERIOD) {
            revert CooldownNotElapsed(COOLDOWN_PERIOD - timeSinceLastRequest);
        }

        _distributeFunds(msg.sender, FAUCET_AMOUNT);
    }

    /// @notice Request immediate funds without cooldown (for testing/admin use)
    /// @dev Only owner can call this to bypass rate limiting
    /// @param recipient Address to receive funds
    function requestFundsImmediate(address recipient) external {
        if (msg.sender != owner) revert Unauthorized();
        if (!enabled) revert FaucetDisabled();
        if (recipient == address(0)) revert ZeroAddress();

        _distributeFunds(recipient, FAUCET_AMOUNT);
    }

    /// @notice Request custom amount (admin only, for special testing scenarios)
    /// @param recipient Address to receive funds
    /// @param amount Custom amount to distribute
    function requestCustomAmount(address recipient, uint256 amount) external {
        if (msg.sender != owner) revert Unauthorized();
        if (!enabled) revert FaucetDisabled();
        if (recipient == address(0)) revert ZeroAddress();

        _distributeFunds(recipient, amount);
    }

    /*//////////////////////////////////////////////////////////////
                          INTERNAL FUNCTIONS
    //////////////////////////////////////////////////////////////*/

    /// @dev Internal function to distribute funds
    /// @param recipient Address receiving the funds
    /// @param amount Amount to distribute
    function _distributeFunds(address recipient, uint256 amount) internal {
        uint256 faucetBalance = usdc.balanceOf(address(this));

        if (faucetBalance < amount) {
            revert InsufficientFaucetBalance(amount, faucetBalance);
        }

        // Track first-time recipients
        if (totalDistributed[recipient] == 0) {
            uniqueRecipients++;
        }

        // Update tracking
        lastRequestTime[recipient] = block.timestamp;
        totalDistributed[recipient] += amount;
        totalDistributedOverall += amount;

        // Transfer USDC
        usdc.safeTransfer(recipient, amount);

        emit FundsRequested(recipient, amount, block.timestamp);
    }

    /*//////////////////////////////////////////////////////////////
                          MANAGEMENT FUNCTIONS
    //////////////////////////////////////////////////////////////*/

    /// @notice Refill the faucet with USDC
    /// @dev Anyone can refill the faucet
    /// @param amount Amount of USDC to add to faucet
    function refill(uint256 amount) external {
        usdc.safeTransferFrom(msg.sender, address(this), amount);
        emit FaucetRefilled(msg.sender, amount);
    }

    /// @notice Enable or disable the faucet
    /// @param _enabled New enabled state
    function setEnabled(bool _enabled) external {
        if (msg.sender != owner) revert Unauthorized();
        enabled = _enabled;
        emit FaucetEnabled(_enabled);
    }

    /// @notice Transfer ownership
    /// @param newOwner New owner address
    function transferOwnership(address newOwner) external {
        if (msg.sender != owner) revert Unauthorized();
        if (newOwner == address(0)) revert ZeroAddress();

        emit OwnershipTransferred(owner, newOwner);
        owner = newOwner;
    }

    /*//////////////////////////////////////////////////////////////
                            VIEW FUNCTIONS
    //////////////////////////////////////////////////////////////*/

    /// @notice Get faucet balance
    /// @return Current USDC balance of the faucet
    function getFaucetBalance() external view returns (uint256) {
        return usdc.balanceOf(address(this));
    }

    /// @notice Get time until next request is available for an address
    /// @param user Address to check
    /// @return Seconds until next request (0 if available now)
    function getTimeUntilNextRequest(address user) external view returns (uint256) {
        uint256 timeSinceLastRequest = block.timestamp - lastRequestTime[user];

        if (timeSinceLastRequest >= COOLDOWN_PERIOD) {
            return 0;
        }

        return COOLDOWN_PERIOD - timeSinceLastRequest;
    }

    /// @notice Check if address can request funds now
    /// @param user Address to check
    /// @return True if cooldown has elapsed
    function canRequestNow(address user) external view returns (bool) {
        if (!enabled) return false;
        return block.timestamp >= lastRequestTime[user] + COOLDOWN_PERIOD;
    }

    /// @notice Get faucet statistics
    /// @return faucetBalance Current faucet balance
    /// @return totalRecipients Number of unique recipients
    /// @return totalDistributedAmount Total amount distributed
    function getStats()
        external
        view
        returns (
            uint256 faucetBalance,
            uint256 totalRecipients,
            uint256 totalDistributedAmount
        )
    {
        return (
            usdc.balanceOf(address(this)),
            uniqueRecipients,
            totalDistributedOverall
        );
    }
}

