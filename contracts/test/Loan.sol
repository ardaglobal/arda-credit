// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Test, console} from "forge-std/Test.sol";
import {stdJson} from "forge-std/StdJson.sol";
import {Loan} from "../src/Loan.sol";
import {SP1VerifierGateway} from "@sp1-contracts/SP1VerifierGateway.sol";

struct SP1ProofFixtureJson {
    int32 initial_balance;
    int32 final_balance;
    bytes proof;
    bytes publicValues;
    bytes32 vkey;
}

contract LoanGroth16Test is Test {
    using stdJson for string;

    address verifier;
    Loan public loan;

    function loadFixture() public view returns (SP1ProofFixtureJson memory) {
        string memory root = vm.projectRoot();
        string memory path = string.concat(
            root,
            "/src/fixtures/groth16-fixture.json"
        );
        string memory json = vm.readFile(path);
        bytes memory jsonBytes = json.parseRaw(".");
        return abi.decode(jsonBytes, (SP1ProofFixtureJson));
    }

    function setUp() public {
        SP1ProofFixtureJson memory fixture = loadFixture();

        verifier = address(new SP1VerifierGateway(address(1)));
        loan = new Loan(verifier, fixture.vkey);
    }

    function test_ValidLoanProof() public {
        SP1ProofFixtureJson memory fixture = loadFixture();

        vm.mockCall(
            verifier,
            abi.encodeWithSelector(SP1VerifierGateway.verifyProof.selector),
            abi.encode(true)
        );

        int32 result = loan.verifyLoanProof(
            fixture.publicValues,
            fixture.proof
        );

        // Function returns final_balance which should be 12 based on our public values
        assert(result == 12);
    }

    function testRevert_InvalidLoanProof() public {
        vm.expectRevert();

        SP1ProofFixtureJson memory fixture = loadFixture();

        // Create a fake proof.
        bytes memory fakeProof = new bytes(fixture.proof.length);

        loan.verifyLoanProof(fixture.publicValues, fakeProof);
    }
}

contract LoanPlonkTest is Test {
    using stdJson for string;

    address verifier;
    Loan public loan;

    function loadFixture() public view returns (SP1ProofFixtureJson memory) {
        string memory root = vm.projectRoot();
        string memory path = string.concat(
            root,
            "/src/fixtures/plonk-fixture.json"
        );
        string memory json = vm.readFile(path);
        bytes memory jsonBytes = json.parseRaw(".");
        return abi.decode(jsonBytes, (SP1ProofFixtureJson));
    }

    function setUp() public {
        SP1ProofFixtureJson memory fixture = loadFixture();

        verifier = address(new SP1VerifierGateway(address(1)));
        loan = new Loan(verifier, fixture.vkey);
    }

    function test_ValidLoanProof() public {
        SP1ProofFixtureJson memory fixture = loadFixture();

        vm.mockCall(
            verifier,
            abi.encodeWithSelector(SP1VerifierGateway.verifyProof.selector),
            abi.encode(true)
        );

        int32 result = loan.verifyLoanProof(
            fixture.publicValues,
            fixture.proof
        );

        // Function returns final_balance which should be 12 based on our public values
        assert(result == 12);
    }

    function testRevert_InvalidLoanProof() public {
        vm.expectRevert();

        SP1ProofFixtureJson memory fixture = loadFixture();

        // Create a fake proof.
        bytes memory fakeProof = new bytes(fixture.proof.length);

        loan.verifyLoanProof(fixture.publicValues, fakeProof);
    }
}

contract LoanVerificationKeyTest is Test {
    Loan public loan;
    bytes32 constant TEST_VKEY =
        0x035a6b230490471fe1a84470ae9bf66a6521fd76d559d50488f30d5b1ccbfc2b;
    bytes32 constant WRONG_VKEY =
        0x1234567890123456789012345678901234567890123456789012345678901234;

    function setUp() public {
        address verifier = address(new SP1VerifierGateway(address(1)));
        loan = new Loan(verifier, TEST_VKEY);
    }

    function test_GetProgramVerificationKey() public view {
        bytes32 vkey = loan.getProgramVerificationKey();
        assertEq(vkey, TEST_VKEY);
    }

    function test_IsValidProgramVerificationKey_ValidKey() public view {
        bool isValid = loan.isValidProgramVerificationKey(TEST_VKEY);
        assertTrue(isValid);
    }

    function test_IsValidProgramVerificationKey_InvalidKey() public view {
        bool isValid = loan.isValidProgramVerificationKey(WRONG_VKEY);
        assertFalse(isValid);
    }

    function test_GetVerificationInfo() public view {
        (bytes32 vkey, address verifierAddr) = loan.getVerificationInfo();
        assertEq(vkey, TEST_VKEY);
        assertTrue(verifierAddr != address(0));
    }

    function test_ValidateProofCompatibility_ValidVKey() public view {
        bytes memory mockProof = "mock_proof_data";
        bytes memory mockPublicValues = "mock_public_values";

        (bool isValid, string memory message) = loan
            .validateProofCompatibility(TEST_VKEY, mockPublicValues, mockProof);

        assertTrue(isValid);
        assertEq(message, "Proof format is compatible with this contract");
    }

    function test_ValidateProofCompatibility_InvalidVKey() public view {
        bytes memory mockProof = "mock_proof_data";
        bytes memory mockPublicValues = "mock_public_values";

        (bool isValid, ) = loan.validateProofCompatibility(
            WRONG_VKEY,
            mockPublicValues,
            mockProof
        );

        assertFalse(isValid);
    }
}

contract StateRootTest is Test {
    Loan public loan;
    address public verifier;

    address public owner = address(0x123);
    address public authorizedUser = address(0x456);
    bytes32 public constant PROGRAM_VKEY = bytes32(uint256(0x1234));

    // Test state identifiers
    bytes32 public constant STATE_ID_1 = keccak256("test-state-1");
    bytes32 public constant STATE_ID_2 = keccak256("test-state-2");

    // Test state roots
    bytes32 public constant STATE_ROOT_1 = keccak256("state-root-1");
    bytes32 public constant STATE_ROOT_2 = keccak256("state-root-2");
    bytes32 public constant STATE_ROOT_3 = keccak256("state-root-3");

    function setUp() public {
        vm.startPrank(owner);

        // Deploy SP1 verifier
        verifier = address(new SP1VerifierGateway(address(1)));
        loan = new Loan(verifier, PROGRAM_VKEY);

        // Authorize a user for testing
        loan.setAuthorization(authorizedUser, true);

        vm.stopPrank();
    }

    function test_GetPreviousStateRoot_NoHistory() public view {
        // Should return zero when there's no history
        bytes32 previousState = loan.getPreviousStateRoot(STATE_ID_1);
        assertEq(
            previousState,
            bytes32(0),
            "Should return zero for no history"
        );
    }

    function test_GetPreviousStateRoot_SingleState() public {
        vm.startPrank(authorizedUser);

        // Mock verifier to always succeed
        vm.mockCall(
            verifier,
            abi.encodeWithSelector(SP1VerifierGateway.verifyProof.selector),
            abi.encode(true)
        );

        // Post first state update
        bytes memory mockProof = "mock_proof_1";
        bytes memory mockPublicValues = "mock_public_values_1";

        loan.updateState(
            STATE_ID_1,
            STATE_ROOT_1,
            mockProof,
            mockPublicValues
        );

        // Should return zero when there's only one state in history
        bytes32 previousState = loan.getPreviousStateRoot(STATE_ID_1);
        assertEq(
            previousState,
            bytes32(0),
            "Should return zero for single state"
        );

        vm.stopPrank();
    }

    function test_GetPreviousStateRoot_MultipleStates() public {
        vm.startPrank(authorizedUser);

        // Mock verifier to always succeed
        vm.mockCall(
            verifier,
            abi.encodeWithSelector(SP1VerifierGateway.verifyProof.selector),
            abi.encode(true)
        );

        // Post first state update
        bytes memory mockProof1 = "mock_proof_1";
        bytes memory mockPublicValues1 = "mock_public_values_1";

        loan.updateState(
            STATE_ID_1,
            STATE_ROOT_1,
            mockProof1,
            mockPublicValues1
        );

        // Post second state update
        bytes memory mockProof2 = "mock_proof_2";
        bytes memory mockPublicValues2 = "mock_public_values_2";

        loan.updateState(
            STATE_ID_1,
            STATE_ROOT_2,
            mockProof2,
            mockPublicValues2
        );

        // Should return the first state root (previous to current)
        bytes32 previousState = loan.getPreviousStateRoot(STATE_ID_1);
        assertEq(
            previousState,
            STATE_ROOT_1,
            "Should return first state root as previous"
        );

        // Post third state update
        bytes memory mockProof3 = "mock_proof_3";
        bytes memory mockPublicValues3 = "mock_public_values_3";

        loan.updateState(
            STATE_ID_1,
            STATE_ROOT_3,
            mockProof3,
            mockPublicValues3
        );

        // Should return the second state root (previous to current)
        bytes32 previousState2 = loan.getPreviousStateRoot(STATE_ID_1);
        assertEq(
            previousState2,
            STATE_ROOT_2,
            "Should return second state root as previous"
        );

        // Verify current state is still correct
        bytes32 currentState = loan.getCurrentState(STATE_ID_1);
        assertEq(
            currentState,
            STATE_ROOT_3,
            "Current state should be the latest"
        );

        vm.stopPrank();
    }

    function test_GetPreviousStateRoot_DifferentStateIds() public {
        vm.startPrank(authorizedUser);

        // Mock verifier to always succeed
        vm.mockCall(
            verifier,
            abi.encodeWithSelector(SP1VerifierGateway.verifyProof.selector),
            abi.encode(true)
        );

        // Post states for STATE_ID_1
        bytes memory mockProof1 = "mock_proof_1";
        bytes memory mockPublicValues1 = "mock_public_values_1";
        loan.updateState(
            STATE_ID_1,
            STATE_ROOT_1,
            mockProof1,
            mockPublicValues1
        );

        bytes memory mockProof2 = "mock_proof_2";
        bytes memory mockPublicValues2 = "mock_public_values_2";
        loan.updateState(
            STATE_ID_1,
            STATE_ROOT_2,
            mockProof2,
            mockPublicValues2
        );

        // Post single state for STATE_ID_2
        bytes memory mockProof3 = "mock_proof_3";
        bytes memory mockPublicValues3 = "mock_public_values_3";
        loan.updateState(
            STATE_ID_2,
            STATE_ROOT_3,
            mockProof3,
            mockPublicValues3
        );

        // Check previous states for different state IDs
        bytes32 previousState1 = loan.getPreviousStateRoot(STATE_ID_1);
        bytes32 previousState2 = loan.getPreviousStateRoot(STATE_ID_2);

        assertEq(
            previousState1,
            STATE_ROOT_1,
            "STATE_ID_1 should have correct previous state"
        );
        assertEq(
            previousState2,
            bytes32(0),
            "STATE_ID_2 should have no previous state"
        );

        vm.stopPrank();
    }
}
