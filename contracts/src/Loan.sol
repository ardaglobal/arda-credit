// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {ISP1Verifier} from "@sp1-contracts/ISP1Verifier.sol";
import {EventHelpers} from "./EventHelpers.sol";
import {IERC20} from "lib/sp1-contracts/contracts/lib/openzeppelin-contracts/contracts/token/ERC20/IERC20.sol";
import {SafeERC20} from "lib/sp1-contracts/contracts/lib/openzeppelin-contracts/contracts/token/ERC20/utils/SafeERC20.sol";

struct LoanBatchPublicValues {
    bytes32 previous_state_root;
    bytes32 new_state_root;
    uint32 transaction_count;
    uint64 batch_timestamp;
    bytes32 transaction_types_hash;
    uint128 previous_total_balance;
    uint128 current_total_balance;
}

/// @title Loan.
/// @author Arda Global
/// @notice This contract implements verification of batch processing proofs with continuous balance tracking.
///         Individual transactions remain private while only initial and final balances are public.
contract Loan is EventHelpers {
    /// @notice The address of the SP1 verifier contract.
    /// @dev This can either be a specific SP1Verifier for a specific version, or the
    ///      SP1VerifierGateway which can be used to verify proofs for any version of SP1.
    ///      For the list of supported verifiers on each chain, see:
    ///      https://github.com/succinctlabs/sp1-contracts/tree/main/contracts/deployments
    address public verifier;

    /// @notice The verification key for the loan program.
    bytes32 public loanProgramVKey;

    /// @notice Store state by state ID.
    mapping(bytes32 => bytes32) public currentState;

    /// @notice Store ZK proofs by proof ID.
    mapping(bytes32 => bytes) public storedProofs;

    /// @notice Store verification results by proof ID.
    mapping(bytes32 => bytes) public storedResults;

    /// @notice Track which proofs have been verified.
    mapping(bytes32 => bool) public verifiedProofs;

    /// @notice Store state history for each state ID.
    mapping(bytes32 => bytes32[]) public stateHistory;

    /// @notice Contract owner for access control.
    address public owner;

    /// @notice Authorized addresses that can post state updates.
    mapping(address => bool) public authorizedPosters;

    /// @notice Current total portfolio balance across all loans
    uint128 public totalPortfolioBalance;

    /// @notice Historical portfolio balances for audit trail
    mapping(uint256 => uint128) public historicalBalances;
    uint256 public balanceHistoryIndex;

    /// @notice Proof metadata structure
    struct ProofMetadata {
        bytes32 proofId;
        bytes32 stateId;
        address submitter;
        uint256 timestamp;
        bool verified;
        bool exists;
    }

    /// @notice Store proof metadata by proof ID.
    mapping(bytes32 => ProofMetadata) public proofMetadata;

    /// @notice Store proof IDs by state ID for quick lookup.
    mapping(bytes32 => bytes32[]) public stateToProofs;

    /// @notice Array of all proof IDs for enumeration.
    bytes32[] public allProofIds;

    /// @notice Mapping from proof ID to index in allProofIds array.
    mapping(bytes32 => uint256) public proofIdToIndex;

    /// @notice Store latest proof ID for each state ID.
    mapping(bytes32 => bytes32) public latestProofForState;

    /*//////////////////////////////////////////////////////////////
                        EVENT FILTERING HELPERS
    //////////////////////////////////////////////////////////////*/

    /// @notice Event type constants for filtering.
    string public constant EVENT_STATE_UPDATED = "StateUpdated";
    string public constant EVENT_BATCH_STATE_UPDATED = "BatchStateUpdated";
    string public constant EVENT_PROOF_STORED = "ProofStored";
    string public constant EVENT_PROOF_VERIFIED = "ProofVerified";
    string public constant EVENT_STATE_READ = "StateRead";
    string public constant EVENT_PROOF_READ = "ProofRead";

    /// @notice Pre-computed hashes for gas-efficient event type comparisons.
    bytes32 private constant STATE_UPDATE_HASH =
        keccak256(abi.encodePacked("state_update"));
    bytes32 private constant BATCH_UPDATE_HASH =
        keccak256(abi.encodePacked("batch_update"));
    bytes32 private constant PROOF_STORED_HASH =
        keccak256(abi.encodePacked("proof_stored"));

    /// @notice Global event statistics.
    EventStats public eventStats;

    /// @notice Track events by submitter for filtering.
    mapping(address => uint256) public eventCountBySubmitter;

    /// @notice Track events by state ID for filtering.
    mapping(bytes32 => uint256) public eventCountByStateId;

    /// @notice Track daily event counts for analytics.
    mapping(uint256 => uint256) public dailyEventCounts; // day => count

    /// @notice Enable/disable read event tracking (gas optimization).
    bool public readEventTrackingEnabled = true;

    /*//////////////////////////////////////////////////////////////
                        USDC DEPOSIT FUNCTIONALITY
    //////////////////////////////////////////////////////////////*/

    /// @notice USDC token contract address
    IERC20 public immutable usdcToken;

    /// @notice Track deposits by user address
    mapping(address => uint256) public userDeposits;

    /// @notice Track total deposits across all users
    uint256 public totalDeposits;

    /// @notice Array of all depositors for enumeration
    address[] public depositors;

    /// @notice Mapping to check if address has deposited (to avoid duplicates in depositors array)
    mapping(address => bool) public hasDeposited;

    /*//////////////////////////////////////////////////////////////
                        COMPREHENSIVE EVENT SYSTEM
    //////////////////////////////////////////////////////////////*/

    /// @notice Enhanced state update event with comprehensive data.
    event StateUpdated(
        bytes32 indexed stateId,
        bytes32 indexed newState,
        bytes32 indexed proofId,
        address updater,
        uint256 timestamp
    );

    /// @notice Batch state update event for multiple state changes.
    event BatchStateUpdated(
        bytes32[] stateIds,
        bytes32[] newStates,
        address indexed updater,
        uint256 indexed timestamp
    );

    /// @notice Event emitted when state is read for monitoring.
    event StateReadRequested(
        bytes32 indexed stateId,
        address indexed reader,
        uint256 indexed timestamp
    );

    /// @notice Enhanced proof storage event.
    event ProofStored(
        bytes32 indexed proofId,
        bytes32 indexed stateId,
        address indexed submitter,
        uint256 timestamp
    );

    /// @notice Enhanced proof verification event with result data.
    event ProofVerified(
        bytes32 indexed proofId,
        bool indexed success,
        bytes result,
        uint256 timestamp
    );

    /// @notice Event emitted when proof is read for monitoring.
    event ProofReadRequested(
        bytes32 indexed proofId,
        address indexed reader,
        uint256 indexed timestamp
    );

    /// @notice Emitted when portfolio balance is updated
    event PortfolioBalanceUpdated(
        uint128 previousBalance,
        uint128 currentBalance,
        bool increased
    );

    /// @notice Emitted when a user deposits USDC
    event Deposit(
        address indexed user,
        uint256 indexed amount,
        uint256 indexed timestamp,
        bytes32 transactionHash
    );

    /// @notice Legacy events for backwards compatibility.
    event AuthorizationChanged(address indexed account, bool authorized);
    event OwnershipTransferred(
        address indexed previousOwner,
        address indexed newOwner
    );

    /// @notice Event emitted for bulk operations.
    event BulkOperationExecuted(
        string indexed operationType,
        uint256 indexed itemCount,
        address indexed executor,
        uint256 timestamp
    );

    /// @notice Event emitted when contract state is queried.
    event ContractStateQueried(
        address indexed querier,
        string queryType,
        uint256 timestamp
    );

    /// @notice Custom errors for gas optimization.
    error UnauthorizedAccess();
    error InvalidArrayLength();
    error StateNotFound();
    error ProofNotFound();
    error InvalidLimit();
    error InvalidIndex();
    error ProofAlreadyExists();

    /// @notice Modifier to restrict access to owner only.
    modifier onlyOwner() {
        if (msg.sender != owner) revert UnauthorizedAccess();
        _;
    }

    /// @notice Modifier to restrict access to authorized posters only.
    modifier onlyAuthorized() {
        if (!authorizedPosters[msg.sender] && msg.sender != owner)
            revert UnauthorizedAccess();
        _;
    }

    constructor(address _verifier, bytes32 _loanProgramVKey, address _usdcToken) {
        verifier = _verifier;
        loanProgramVKey = _loanProgramVKey;
        usdcToken = IERC20(_usdcToken);
        owner = msg.sender;
        authorizedPosters[msg.sender] = true;
        emit AuthorizationChanged(msg.sender, true);
    }

    /// @notice The entrypoint for verifying the proof of a batch processing operation.
    /// @param _proofBytes The encoded proof.
    /// @param _publicValues The encoded public values containing loan batch information.
    function verifyLoanProof(
        bytes calldata _publicValues,
        bytes calldata _proofBytes
    ) public view returns (uint128) {
        ISP1Verifier(verifier).verifyProof(
            loanProgramVKey,
            _publicValues,
            _proofBytes
        );

        // Check if this is the new LoanBatchPublicValues format (224 bytes = 7 * 32)
        if (_publicValues.length == 224) {
            LoanBatchPublicValues memory publicValues = abi.decode(
                _publicValues,
                (LoanBatchPublicValues)
            );
            return publicValues.current_total_balance;
        }
        // Legacy format compatibility (64 bytes = 2 * 32 for initial_balance and final_balance)
        else if (_publicValues.length == 64) {
            (, int32 finalBalance) = abi.decode(_publicValues, (int32, int32));
            // Return final_balance as uint128 for legacy compatibility
            return uint128(uint256(int256(finalBalance)));
        } else {
            revert("Unsupported public values format");
        }
    }

    /// @notice Update state with ZK proof verification.
    /// @param stateId The state identifier.
    /// @param newStateRoot The new state root to store.
    /// @param proof The ZK proof to verify.
    /// @param publicValues The encoded public values for proof verification.
    function updateState(
        bytes32 stateId,
        bytes32 newStateRoot,
        bytes calldata proof,
        bytes calldata publicValues
    ) external onlyAuthorized {
        // Verify the proof first
        ISP1Verifier(verifier).verifyProof(
            loanProgramVKey,
            publicValues,
            proof
        );

        // Handle portfolio balance tracking if publicValues includes balance data
        if (publicValues.length >= 224) {
            // 7 * 32 bytes for new LoanBatchPublicValues structure
            try this.decodeLoanBatchPublicValues(publicValues) returns (
                bytes32,
                bytes32,
                uint32,
                uint64,
                bytes32,
                uint128 previousBalance,
                uint128 currentBalance
            ) {
                // Validate balance transition
                if (totalPortfolioBalance != 0) {
                    require(
                        previousBalance == totalPortfolioBalance,
                        "Portfolio balance mismatch"
                    );
                }

                // Update portfolio balance
                totalPortfolioBalance = currentBalance;

                // Store historical balance for audit trail
                historicalBalances[balanceHistoryIndex] = currentBalance;
                balanceHistoryIndex++;

                // Emit portfolio balance event
                emit PortfolioBalanceUpdated(
                    previousBalance,
                    currentBalance,
                    currentBalance > previousBalance
                );
            } catch {
                // If decoding fails, continue with legacy behavior
            }
        }

        bytes32 proofHash = keccak256(proof);

        currentState[stateId] = newStateRoot;
        stateHistory[stateId].push(newStateRoot);
        storedProofs[proofHash] = proof;
        storedResults[proofHash] = publicValues;
        verifiedProofs[proofHash] = true;

        // Store proof metadata
        _storeProofMetadata(proofHash, stateId, msg.sender);

        emit StateUpdated(
            stateId,
            newStateRoot,
            proofHash,
            msg.sender,
            block.timestamp
        );

        // Update event statistics
        eventCountByStateId[stateId]++;
        _updateEventStats("state_update");
    }

    /// @notice Get the current state root for a given state ID.
    /// @param stateId The state identifier.
    /// @return The current state root.
    function getCurrentStateRoot(
        bytes32 stateId
    ) external view returns (bytes32) {
        return currentState[stateId];
    }

    /// @notice Read current state with existence check.
    /// @param stateId The state identifier.
    /// @return state The current state root.
    /// @return exists True if the state exists.
    function readCurrentState(
        bytes32 stateId
    ) external returns (bytes32 state, bool exists) {
        state = currentState[stateId];
        exists = (state != bytes32(0) || stateHistory[stateId].length > 0);

        // Track read event
        _trackReadEvent(stateId, bytes32(0));
    }

    /// @notice Read state history for a given state ID.
    /// @param stateId The state identifier.
    /// @param limit Maximum number of recent states to return (0 for all).
    /// @return states Array of recent state roots.
    function readStateHistory(
        bytes32 stateId,
        uint256 limit
    ) external view returns (bytes32[] memory states) {
        bytes32[] storage history = stateHistory[stateId];
        uint256 length = history.length;

        if (length == 0) {
            return new bytes32[](0);
        }

        if (limit == 0 || limit > length) {
            limit = length;
        }

        states = new bytes32[](limit);
        uint256 startIndex = length - limit;

        for (uint256 i = 0; i < limit; i++) {
            states[i] = history[startIndex + i];
        }
    }

    /// @notice Get a stored proof by proof ID.
    /// @param proofId The proof identifier (hash).
    /// @return The stored proof bytes.
    function getStoredProof(
        bytes32 proofId
    ) external view returns (bytes memory) {
        if (storedProofs[proofId].length == 0) revert ProofNotFound();
        return storedProofs[proofId];
    }

    /// @notice Get a stored proof by providing the raw proof bytes.
    /// @param proof The raw proof bytes.
    /// @return The stored proof bytes.
    function getStoredProofByProof(
        bytes calldata proof
    ) external view returns (bytes memory) {
        bytes32 proofId = keccak256(proof);
        if (storedProofs[proofId].length == 0) revert ProofNotFound();
        return storedProofs[proofId];
    }

    /// @notice Get stored verification result by proof ID.
    /// @param proofId The proof identifier (hash).
    /// @return The stored result bytes.
    function getStoredResult(
        bytes32 proofId
    ) external view returns (bytes memory) {
        return storedResults[proofId];
    }

    /// @notice Read comprehensive proof details.
    /// @param proofId The proof identifier (hash).
    /// @return proof The stored proof bytes.
    /// @return result The stored result bytes.
    /// @return verified True if the proof has been verified.
    function readProofDetails(
        bytes32 proofId
    )
        external
        view
        returns (bytes memory proof, bytes memory result, bool verified)
    {
        proof = storedProofs[proofId];
        result = storedResults[proofId];
        verified = verifiedProofs[proofId];

        if (proof.length == 0) revert ProofNotFound();
    }

    /// @notice Update multiple states in a single transaction.
    /// @param stateIds Array of state identifiers.
    /// @param newStates Array of new state roots.
    /// @param proofs Array of ZK proofs.
    /// @param results Array of verification results.
    /// @return successes Array indicating success/failure for each update.
    function batchUpdateStates(
        bytes32[] calldata stateIds,
        bytes32[] calldata newStates,
        bytes[] calldata proofs,
        bytes[] calldata results
    ) external onlyAuthorized returns (bool[] memory successes) {
        uint256 length = stateIds.length;

        if (
            length != newStates.length ||
            length != proofs.length ||
            length != results.length
        ) {
            revert InvalidArrayLength();
        }

        successes = new bool[](length);

        // Cache frequently accessed values to reduce external calls
        address cachedVerifier = verifier;
        bytes32 cachedVKey = loanProgramVKey;
        address cachedSender = msg.sender;
        uint256 cachedTimestamp = block.timestamp;

        // Pre-verify all proofs to fail fast if any are invalid
        for (uint256 i = 0; i < length; ) {
            try
                ISP1Verifier(cachedVerifier).verifyProof(
                    cachedVKey,
                    results[i],
                    proofs[i]
                )
            {
                successes[i] = true;
            } catch {
                successes[i] = false;
            }

            unchecked {
                ++i;
            }
        }

        // Batch process all successful verifications
        for (uint256 i = 0; i < length; ) {
            if (successes[i]) {
                bytes32 proofHash = keccak256(proofs[i]);

                // Batch storage updates
                currentState[stateIds[i]] = newStates[i];
                stateHistory[stateIds[i]].push(newStates[i]);
                storedProofs[proofHash] = proofs[i];
                storedResults[proofHash] = results[i];
                verifiedProofs[proofHash] = true;

                // Store proof metadata (optimized call)
                _storeProofMetadata(proofHash, stateIds[i], cachedSender);

                // Emit individual state updated events
                emit StateUpdated(
                    stateIds[i],
                    newStates[i],
                    proofHash,
                    cachedSender,
                    cachedTimestamp
                );

                // Update event statistics
                eventCountByStateId[stateIds[i]]++;
            }

            unchecked {
                ++i;
            }
        }

        // Emit batch event and update stats once
        emit BatchStateUpdated(
            stateIds,
            newStates,
            cachedSender,
            cachedTimestamp
        );
        _updateEventStats("batch_update");
    }

    /// @notice Read multiple current states.
    /// @param stateIds Array of state identifiers.
    /// @return states Array of current state roots.
    function batchReadStates(
        bytes32[] calldata stateIds
    ) external view returns (bytes32[] memory states) {
        uint256 length = stateIds.length;
        states = new bytes32[](length);

        for (uint256 i = 0; i < length; ) {
            states[i] = currentState[stateIds[i]];

            unchecked {
                ++i;
            }
        }
    }

    /// @notice Grant or revoke authorization to post state updates.
    /// @param account The account to modify authorization for.
    /// @param authorized True to grant, false to revoke authorization.
    function setAuthorization(
        address account,
        bool authorized
    ) external onlyOwner {
        authorizedPosters[account] = authorized;
        emit AuthorizationChanged(account, authorized);
    }

    /// @notice Transfer ownership of the contract.
    /// @param newOwner The new owner address.
    function transferOwnership(address newOwner) external onlyOwner {
        require(newOwner != address(0), "new owner is zero address");
        address previousOwner = owner;
        owner = newOwner;

        authorizedPosters[previousOwner] = false;
        authorizedPosters[newOwner] = true;

        emit AuthorizationChanged(previousOwner, false);
        emit AuthorizationChanged(newOwner, true);
        emit OwnershipTransferred(previousOwner, newOwner);
    }

    /// @notice Check if an address is authorized to post state updates.
    /// @param account The account to check.
    /// @return True if authorized.
    function isAuthorized(address account) external view returns (bool) {
        return authorizedPosters[account] || account == owner;
    }

    /// @notice Check if a proof exists.
    /// @param proof The proof to check.
    /// @return True if the proof exists.
    function proofExists(bytes calldata proof) external view returns (bool) {
        return storedProofs[keccak256(proof)].length > 0;
    }

    /// @notice Get the total number of states in history for a state ID.
    /// @param stateId The state identifier.
    /// @return The number of historical states.
    function getStateHistoryLength(
        bytes32 stateId
    ) external view returns (uint256) {
        return stateHistory[stateId].length;
    }

    /// @notice Get the previous state root for a given state ID.
    /// @param stateId The state identifier.
    /// @return previousState The previous state root (returns zero if no history or only one state).
    function getPreviousStateRoot(
        bytes32 stateId
    ) external view returns (bytes32 previousState) {
        bytes32[] storage history = stateHistory[stateId];
        uint256 length = history.length;

        // If there are less than 2 states in history, there's no previous state
        if (length < 2) {
            return bytes32(0);
        }

        // Return the second-to-last state (previous state)
        return history[length - 2];
    }

    // Add helper function for decoding
    function decodeLoanBatchPublicValues(
        bytes calldata data
    )
        external
        pure
        returns (
            bytes32 previous_state_root,
            bytes32 new_state_root,
            uint32 transaction_count,
            uint64 batch_timestamp,
            bytes32 transaction_types_hash,
            uint128 previous_total_balance,
            uint128 current_total_balance
        )
    {
        (
            previous_state_root,
            new_state_root,
            transaction_count,
            batch_timestamp,
            transaction_types_hash,
            previous_total_balance,
            current_total_balance
        ) = abi.decode(
            data,
            (bytes32, bytes32, uint32, uint64, bytes32, uint128, uint128)
        );
    }

    /// @notice Get current portfolio balance
    function getCurrentPortfolioBalance() external view returns (uint128) {
        return totalPortfolioBalance;
    }

    /// @notice Get portfolio metrics for monitoring dashboard
    function getPortfolioMetrics()
        external
        view
        returns (
            uint128 currentBalance,
            uint256 historicalCount,
            uint256 lastUpdated
        )
    {
        return (totalPortfolioBalance, balanceHistoryIndex, block.timestamp);
    }

    /// @notice Get portfolio balance history for audit
    function getPortfolioBalanceHistory(
        uint256 fromIndex,
        uint256 toIndex
    ) external view returns (uint128[] memory) {
        require(fromIndex < balanceHistoryIndex, "Invalid from index");
        require(toIndex <= balanceHistoryIndex, "Invalid to index");
        require(fromIndex <= toIndex, "Invalid range");

        uint256 length = toIndex - fromIndex;
        uint128[] memory balances = new uint128[](length);

        for (uint256 i = 0; i < length; i++) {
            balances[i] = historicalBalances[fromIndex + i];
        }

        return balances;
    }

    /*//////////////////////////////////////////////////////////////
                        PROOF READING FUNCTIONS
    //////////////////////////////////////////////////////////////*/

    /// @notice Get proof by proof ID.
    /// @param proofId The proof identifier (hash).
    /// @return proof The proof bytes.
    /// @return exists True if the proof exists.
    function getProofById(
        bytes32 proofId
    ) external returns (bytes memory proof, bool exists) {
        proof = storedProofs[proofId];
        exists = proofMetadata[proofId].exists;

        // Track read event
        _trackReadEvent(bytes32(0), proofId);
    }

    /// @notice Get proof by state ID (returns latest proof for that state).
    /// @param stateId The state identifier.
    /// @return proof The latest proof bytes for the state.
    /// @return proofId The proof identifier.
    function getProofByStateId(
        bytes32 stateId
    ) external returns (bytes memory proof, bytes32 proofId) {
        proofId = latestProofForState[stateId];
        if (proofId == bytes32(0)) {
            return (new bytes(0), bytes32(0));
        }
        proof = storedProofs[proofId];

        // Track read events for both state and proof
        _trackReadEvent(stateId, proofId);
    }

    /// @notice Get latest proof for a state ID with timestamp.
    /// @param stateId The state identifier.
    /// @return proof The latest proof bytes.
    /// @return proofId The proof identifier.
    /// @return timestamp When the proof was submitted.
    function getLatestProof(
        bytes32 stateId
    )
        external
        view
        returns (bytes memory proof, bytes32 proofId, uint256 timestamp)
    {
        proofId = latestProofForState[stateId];
        if (proofId == bytes32(0)) {
            return (new bytes(0), bytes32(0), 0);
        }

        proof = storedProofs[proofId];
        timestamp = proofMetadata[proofId].timestamp;
    }

    /*//////////////////////////////////////////////////////////////
                    PROOF VERIFICATION STATUS
    //////////////////////////////////////////////////////////////*/

    /// @notice Check if a proof is verified.
    /// @param proofId The proof identifier.
    /// @return True if the proof is verified.
    function isProofVerified(bytes32 proofId) external view returns (bool) {
        return proofMetadata[proofId].verified;
    }

    /// @notice Get verification result for a proof.
    /// @param proofId The proof identifier.
    /// @return verified True if the proof is verified.
    /// @return result The verification result bytes.
    function getVerificationResult(
        bytes32 proofId
    ) external view returns (bool verified, bytes memory result) {
        verified = proofMetadata[proofId].verified;
        result = storedResults[proofId];
    }

    /// @notice Get timestamp when proof was submitted.
    /// @param proofId The proof identifier.
    /// @return The timestamp of proof submission.
    function getProofTimestamp(
        bytes32 proofId
    ) external view returns (uint256) {
        return proofMetadata[proofId].timestamp;
    }

    /*//////////////////////////////////////////////////////////////
                        PROOF ENUMERATION
    //////////////////////////////////////////////////////////////*/

    /// @notice Get total number of stored proofs.
    /// @return The total count of proofs.
    function getProofCount() external view returns (uint256) {
        return allProofIds.length;
    }

    /// @notice Get proof by index in the enumeration.
    /// @param index The index in the proof array.
    /// @return proofId The proof identifier at the index.
    /// @return proof The proof bytes.
    function getProofByIndex(
        uint256 index
    ) external view returns (bytes32 proofId, bytes memory proof) {
        if (index >= allProofIds.length) revert InvalidIndex();

        proofId = allProofIds[index];
        proof = storedProofs[proofId];
    }

    /// @notice Get recent proofs up to a specified limit.
    /// @param limit Maximum number of recent proofs to return.
    /// @return proofIds Array of recent proof identifiers.
    /// @return proofs Array of recent proof bytes.
    function getRecentProofs(
        uint256 limit
    ) external view returns (bytes32[] memory proofIds, bytes[] memory proofs) {
        uint256 totalProofs = allProofIds.length;
        if (totalProofs == 0) {
            return (new bytes32[](0), new bytes[](0));
        }

        if (limit == 0 || limit > totalProofs) {
            limit = totalProofs;
        }

        proofIds = new bytes32[](limit);
        proofs = new bytes[](limit);

        uint256 startIndex = totalProofs - limit;

        for (uint256 i = 0; i < limit; ) {
            bytes32 proofId = allProofIds[startIndex + i];
            proofIds[i] = proofId;
            proofs[i] = storedProofs[proofId];

            unchecked {
                ++i;
            }
        }
    }

    /// @notice Get all proof IDs for a specific state ID.
    /// @param stateId The state identifier.
    /// @return proofIds Array of proof identifiers for the state.
    function getProofsByStateId(
        bytes32 stateId
    ) external view returns (bytes32[] memory proofIds) {
        return stateToProofs[stateId];
    }

    /// @notice Get proof metadata by proof ID.
    /// @param proofId The proof identifier.
    /// @return metadata The complete proof metadata.
    function getProofMetadata(
        bytes32 proofId
    ) external view returns (ProofMetadata memory metadata) {
        metadata = proofMetadata[proofId];
        if (!metadata.exists) revert ProofNotFound();
    }

    /// @notice Get submitter address for a proof.
    /// @param proofId The proof identifier.
    /// @return submitter The address that submitted the proof.
    function getProofSubmitter(
        bytes32 proofId
    ) external view returns (address submitter) {
        if (!proofMetadata[proofId].exists) revert ProofNotFound();
        return proofMetadata[proofId].submitter;
    }

    /*//////////////////////////////////////////////////////////////
                        INTERNAL PROOF MANAGEMENT
    //////////////////////////////////////////////////////////////*/

    /// @notice Internal function to store proof metadata.
    /// @param proofId The proof identifier.
    /// @param stateId The associated state identifier.
    /// @param submitter The address submitting the proof.
    function _storeProofMetadata(
        bytes32 proofId,
        bytes32 stateId,
        address submitter
    ) internal {
        // Revert if proof already exists to prevent duplicate submissions
        if (proofMetadata[proofId].exists) {
            revert ProofAlreadyExists();
        }

        proofMetadata[proofId] = ProofMetadata({
            proofId: proofId,
            stateId: stateId,
            submitter: submitter,
            timestamp: block.timestamp,
            verified: true, // Set to true since we verify before storing
            exists: true
        });

        // Add to enumeration
        proofIdToIndex[proofId] = allProofIds.length;
        allProofIds.push(proofId);

        // Add to state mapping
        stateToProofs[stateId].push(proofId);

        // Update latest proof for state
        latestProofForState[stateId] = proofId;

        emit ProofStored(proofId, stateId, submitter, block.timestamp);
        emit ProofVerified(proofId, true, new bytes(0), block.timestamp);

        // Update event statistics
        _updateEventStats("proof_stored");
    }

    /*//////////////////////////////////////////////////////////////
                        EVENT HELPER IMPLEMENTATIONS
    //////////////////////////////////////////////////////////////*/

    /// @notice Internal function to update event statistics.
    function _updateEventStats(string memory eventType) internal override {
        eventStats.lastEventTimestamp = block.timestamp;

        // Update daily counts
        uint256 today = block.timestamp / 86400;
        dailyEventCounts[today]++;

        // Update submitter counts
        eventCountBySubmitter[msg.sender]++;

        // Update specific event type counts
        bytes32 eventHash = keccak256(abi.encodePacked(eventType));

        if (eventHash == STATE_UPDATE_HASH) {
            eventStats.totalStateUpdates++;
        } else if (eventHash == BATCH_UPDATE_HASH) {
            eventStats.totalBatchUpdates++;
        } else if (eventHash == PROOF_STORED_HASH) {
            eventStats.totalProofStored++;
            eventStats.totalProofVerified++; // Proofs are verified when stored
        }
    }

    /// @notice Internal function to track read events.
    function _trackReadEvent(
        bytes32 stateId,
        bytes32 proofId
    ) internal override {
        if (!readEventTrackingEnabled) return;

        eventStats.lastEventTimestamp = block.timestamp;

        // Update daily counts
        uint256 today = block.timestamp / 86400;
        dailyEventCounts[today]++;

        // Update submitter counts
        eventCountBySubmitter[msg.sender]++;

        if (stateId != bytes32(0)) {
            eventStats.totalStateReads++;
            eventCountByStateId[stateId]++;
            emit StateReadRequested(stateId, msg.sender, block.timestamp);
        }

        if (proofId != bytes32(0)) {
            eventStats.totalProofReads++;
            emit ProofReadRequested(proofId, msg.sender, block.timestamp);
        }
    }

    /// @notice Get event statistics for monitoring.
    function getEventStats()
        external
        view
        override
        returns (EventStats memory)
    {
        return eventStats;
    }

    /// @notice Get event count by submitter address.
    function getEventCountBySubmitter(
        address submitter
    ) external view override returns (uint256) {
        return eventCountBySubmitter[submitter];
    }

    /// @notice Get event count by state ID.
    function getEventCountByStateId(
        bytes32 stateId
    ) external view override returns (uint256) {
        return eventCountByStateId[stateId];
    }

    /// @notice Get daily event count for a specific day.
    function getDailyEventCount(
        uint256 day
    ) external view override returns (uint256) {
        return dailyEventCounts[day];
    }

    /// @notice Toggle read event tracking (gas optimization).
    function setReadEventTracking(bool enabled) external override onlyOwner {
        readEventTrackingEnabled = enabled;
    }

    /// @notice Get aggregated event counts for a time range.
    function getEventCountInRange(
        uint256 startTime,
        uint256 endTime
    ) external view override returns (uint256 totalEvents) {
        if (startTime >= endTime) {
            return 0;
        }

        uint256 startDay = startTime / 86400;
        uint256 endDay = endTime / 86400;

        for (uint256 day = startDay; day <= endDay; day++) {
            totalEvents += dailyEventCounts[day];
        }
    }

    /*//////////////////////////////////////////////////////////////
                    LOAN BATCH PUBLIC VALUES GETTERS
    //////////////////////////////////////////////////////////////*/

    /// @notice Get the previous state root from the latest batch public values
    /// @param stateId The state identifier
    /// @return previous_state_root The previous state root from the last verified batch
    function getPreviousStateRootFromBatch(
        bytes32 stateId
    ) external view returns (bytes32 previous_state_root) {
        bytes32 latestProofId = latestProofForState[stateId];
        if (latestProofId == bytes32(0)) {
            return bytes32(0);
        }

        bytes memory publicValues = storedResults[latestProofId];
        if (publicValues.length == 0) {
            return bytes32(0);
        }

        try this.decodeLoanBatchPublicValues(publicValues) returns (
            bytes32 prev_state_root,
            bytes32,
            uint32,
            uint64,
            bytes32,
            uint128,
            uint128
        ) {
            return prev_state_root;
        } catch {
            return bytes32(0);
        }
    }

    /// @notice Get the new state root from the latest batch public values
    /// @param stateId The state identifier
    /// @return new_state_root The new state root from the last verified batch
    function getNewStateRootFromBatch(
        bytes32 stateId
    ) external view returns (bytes32 new_state_root) {
        bytes32 latestProofId = latestProofForState[stateId];
        if (latestProofId == bytes32(0)) {
            return bytes32(0);
        }

        bytes memory publicValues = storedResults[latestProofId];
        if (publicValues.length == 0) {
            return bytes32(0);
        }

        try this.decodeLoanBatchPublicValues(publicValues) returns (
            bytes32,
            bytes32 new_state_root_val,
            uint32,
            uint64,
            bytes32,
            uint128,
            uint128
        ) {
            return new_state_root_val;
        } catch {
            return bytes32(0);
        }
    }

    /// @notice Get the previous total balance from the latest batch
    /// @param stateId The state identifier
    /// @return previous_total_balance The previous total balance from the last verified batch
    function getPreviousTotalBalance(
        bytes32 stateId
    ) external view returns (uint128 previous_total_balance) {
        bytes32 latestProofId = latestProofForState[stateId];
        if (latestProofId == bytes32(0)) {
            return 0;
        }

        bytes memory publicValues = storedResults[latestProofId];
        if (publicValues.length == 0) {
            return 0;
        }

        try this.decodeLoanBatchPublicValues(publicValues) returns (
            bytes32,
            bytes32,
            uint32,
            uint64,
            bytes32,
            uint128 prev_total_balance,
            uint128
        ) {
            return prev_total_balance;
        } catch {
            return 0;
        }
    }

    /// @notice Get the current total balance from the latest batch
    /// @param stateId The state identifier
    /// @return current_total_balance The current total balance from the last verified batch
    function getCurrentTotalBalance(
        bytes32 stateId
    ) external view returns (uint128 current_total_balance) {
        bytes32 latestProofId = latestProofForState[stateId];
        if (latestProofId == bytes32(0)) {
            return 0;
        }

        bytes memory publicValues = storedResults[latestProofId];
        if (publicValues.length == 0) {
            return 0;
        }

        try this.decodeLoanBatchPublicValues(publicValues) returns (
            bytes32,
            bytes32,
            uint32,
            uint64,
            bytes32,
            uint128,
            uint128 curr_total_balance
        ) {
            return curr_total_balance;
        } catch {
            return 0;
        }
    }

    /*//////////////////////////////////////////////////////////////
                    VERIFICATION KEY UTILITIES
    //////////////////////////////////////////////////////////////*/

    /// @notice Get the current program verification key used by this contract.
    /// @return vKey The 32-byte verification key for the loan program.
    function getProgramVerificationKey() external view returns (bytes32 vKey) {
        return loanProgramVKey;
    }

    /// @notice Check if a given verification key matches the contract's program vkey.
    /// @param vKey The verification key to check against the contract's vkey.
    /// @return matches True if the provided vkey matches the contract's vkey.
    function isValidProgramVerificationKey(
        bytes32 vKey
    ) external view returns (bool matches) {
        return vKey == loanProgramVKey;
    }

    /// @notice Get verification key information for debugging.
    /// @return contractVKey The contract's verification key.
    /// @return verifierAddress The SP1 verifier contract address.
    function getVerificationInfo()
        external
        view
        returns (bytes32 contractVKey, address verifierAddress)
    {
        return (loanProgramVKey, verifier);
    }

    /// @notice Validate that a proof would be accepted by this contract (without executing).
    /// @dev This is a view function that checks vkey compatibility without state changes.
    /// @param expectedVKey The verification key you expect to use.
    /// @param publicValues The public values for the proof.
    /// @param proof The proof bytes.
    /// @return isValid True if the proof format is compatible with this contract.
    /// @return message Human-readable validation message.
    function validateProofCompatibility(
        bytes32 expectedVKey,
        bytes calldata publicValues,
        bytes calldata proof
    ) external view returns (bool isValid, string memory message) {
        // Check if the verification key matches
        if (expectedVKey != loanProgramVKey) {
            return (
                false,
                string(
                    abi.encodePacked(
                        "Verification key mismatch. Expected: 0x",
                        _bytes32ToHex(loanProgramVKey),
                        ", Got: 0x",
                        _bytes32ToHex(expectedVKey)
                    )
                )
            );
        }

        // Check if proof and public values are non-empty
        if (proof.length == 0) {
            return (false, "Proof is empty");
        }

        if (publicValues.length == 0) {
            return (false, "Public values are empty");
        }

        return (true, "Proof format is compatible with this contract");
    }

    /// @notice Internal helper to convert bytes32 to hex string.
    function _bytes32ToHex(bytes32 data) internal pure returns (string memory) {
        bytes memory alphabet = "0123456789abcdef";
        bytes memory str = new bytes(64);

        for (uint256 i = 0; i < 32; i++) {
            str[i * 2] = alphabet[uint8(data[i] >> 4)];
            str[1 + i * 2] = alphabet[uint8(data[i] & 0x0f)];
        }

        return string(str);
    }

    /*//////////////////////////////////////////////////////////////
                        USDC DEPOSIT FUNCTIONS
    //////////////////////////////////////////////////////////////*/

    /// @notice Deposit USDC tokens to the contract
    /// @param amount The amount of USDC to deposit (in USDC's native decimals)
    function deposit(uint256 amount) external {
        require(amount > 0, "Deposit amount must be greater than zero");
        
        // Calculate transaction hash for the event
        bytes32 transactionHash = keccak256(abi.encodePacked(
            msg.sender,
            amount,
            block.timestamp,
            block.number
        ));

        // Transfer USDC from user to this contract
        SafeERC20.safeTransferFrom(usdcToken, msg.sender, address(this), amount);

        // Update user deposit balance
        userDeposits[msg.sender] += amount;
        totalDeposits += amount;

        // Add to depositors array if first time depositing
        if (!hasDeposited[msg.sender]) {
            depositors.push(msg.sender);
            hasDeposited[msg.sender] = true;
        }

        // Emit deposit event
        emit Deposit(msg.sender, amount, block.timestamp, transactionHash);
    }

    /// @notice Get the deposit balance for a specific user
    /// @param user The user address to check
    /// @return The amount of USDC deposited by the user
    function getUserDeposit(address user) external view returns (uint256) {
        return userDeposits[user];
    }

    /// @notice Get the total deposits across all users
    /// @return The total amount of USDC deposited
    function getTotalDeposits() external view returns (uint256) {
        return totalDeposits;
    }

    /// @notice Get all depositors and their amounts
    /// @return users Array of user addresses who have deposited
    /// @return amounts Array of corresponding deposit amounts
    function getAllDepositors() external view returns (address[] memory users, uint256[] memory amounts) {
        uint256 length = depositors.length;
        users = new address[](length);
        amounts = new uint256[](length);
        
        for (uint256 i = 0; i < length; i++) {
            users[i] = depositors[i];
            amounts[i] = userDeposits[depositors[i]];
        }
    }

    /// @notice Get the number of unique depositors
    /// @return The total number of users who have deposited
    function getDepositorCount() external view returns (uint256) {
        return depositors.length;
    }

    /// @notice Get depositor by index
    /// @param index The index in the depositors array
    /// @return user The depositor address
    /// @return amount The amount deposited by the user
    function getDepositorByIndex(uint256 index) external view returns (address user, uint256 amount) {
        require(index < depositors.length, "Index out of bounds");
        user = depositors[index];
        amount = userDeposits[user];
    }

    /// @notice Check if a user has made any deposits
    /// @param user The user address to check
    /// @return True if the user has deposited, false otherwise
    function hasUserDeposited(address user) external view returns (bool) {
        return hasDeposited[user];
    }

    /// @notice Get USDC token address
    /// @return The address of the USDC token contract
    function getUsdcTokenAddress() external view returns (address) {
        return address(usdcToken);
    }

    /// @notice Get contract's USDC balance
    /// @return The total USDC balance held by this contract
    function getContractUsdcBalance() external view returns (uint256) {
        return usdcToken.balanceOf(address(this));
    }
}
