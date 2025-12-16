// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Script} from "forge-std/Script.sol";
import {console2} from "forge-std/console2.sol";
import {ARDA} from "../src/ARDA.sol";
import {ARDAFaucet} from "../src/ARDAFaucet.sol";
import {MockUSDC} from "../test/mocks/MockUSDC.sol";
import {SP1MockVerifier} from "@sp1-contracts/SP1MockVerifier.sol";

/// @title ARDA Deployment Script
/// @notice Environment-aware deployment script for ARDA contracts
/// @dev Deploys different contract sets based on SERVER_ENVIRONMENT:
///      - local/dev/staging: MockUSDC + ARDAFaucet + ARDA + SP1MockVerifier (Anvil)
///      - sandbox: ARDA only on Sepolia (assumes real USDC and verifier exist)
///      - production: ARDA only on mainnet (assumes real USDC and verifier exist)
contract Deploy is Script {
    /*//////////////////////////////////////////////////////////////
                              ENVIRONMENT
    //////////////////////////////////////////////////////////////*/

    /// @dev Read SERVER_ENVIRONMENT from env vars (default: local)
    function getEnvironment() internal view returns (string memory) {
        try vm.envString("SERVER_ENVIRONMENT") returns (string memory env) {
            return env;
        } catch {
            return "local";
        }
    }

    /// @dev Check if environment uses Anvil (local, dev, staging)
    function isAnvilEnvironment(string memory env) internal pure returns (bool) {
        return
            keccak256(bytes(env)) == keccak256(bytes("local")) ||
            keccak256(bytes(env)) == keccak256(bytes("dev")) ||
            keccak256(bytes(env)) == keccak256(bytes("staging"));
    }

    /*//////////////////////////////////////////////////////////////
                            MAIN DEPLOYMENT
    //////////////////////////////////////////////////////////////*/

    function run() external {
        string memory environment = getEnvironment();
        console2.log("==================================");
        console2.log("ARDA Contract Deployment");
        console2.log("==================================");
        console2.log("Environment:", environment);
        console2.log("");

        if (isAnvilEnvironment(environment)) {
            deployAnvil();
        } else {
            deployProduction();
        }
    }

    /*//////////////////////////////////////////////////////////////
                         ANVIL DEPLOYMENT (LOCAL/DEV/STAGING)
    //////////////////////////////////////////////////////////////*/

    /// @notice Deploy complete Anvil environment with mocks
    /// @dev Used for local, dev, and staging environments
    function deployAnvil() internal {
        console2.log("Deploying ANVIL environment (with mocks)...");
        console2.log("");

        // Get private key - handle both with and without 0x prefix
        string memory pkString = vm.envString("ETHEREUM_WALLET_PRIVATE_KEY");
        uint256 deployerPrivateKey;
        
        // Check if it starts with 0x
        if (bytes(pkString).length >= 2 && bytes(pkString)[0] == "0" && bytes(pkString)[1] == "x") {
            deployerPrivateKey = vm.parseUint(pkString);
        } else {
            // Add 0x prefix
            deployerPrivateKey = vm.parseUint(string.concat("0x", pkString));
        }
        
        address deployer = vm.addr(deployerPrivateKey);

        console2.log("Deployer address:", deployer);
        console2.log("");

        vm.startBroadcast(deployerPrivateKey);

        // 1. Deploy MockUSDC
        console2.log("1. Deploying MockUSDC...");
        MockUSDC usdc = new MockUSDC();
        console2.log("   MockUSDC deployed at:", address(usdc));

        // 2. Deploy SP1MockVerifier
        console2.log("");
        console2.log("2. Deploying SP1MockVerifier...");
        SP1MockVerifier mockVerifier = new SP1MockVerifier();
        address verifier = address(mockVerifier);
        console2.log("   SP1MockVerifier deployed at:", verifier);

        // 3. Deploy ARDA Contract
        console2.log("");
        console2.log("3. Deploying ARDA contract...");
        
        bytes32 programVKey = vm.envOr(
            "PROGRAM_VKEY",
            bytes32(0x035a6b230490471fe1a84470ae9bf66a6521fd76d559d50488f30d5b1ccbfc2b)
        );
        
        ARDA arda = new ARDA(verifier, programVKey);
        console2.log("   ARDA deployed at:", address(arda));

        // Set USDC token address in ARDA contract
        arda.setUsdcTokenForTesting(address(usdc));
        console2.log("   USDC token configured in ARDA");

        // 4. Deploy ARDAFaucet
        console2.log("");
        console2.log("4. Deploying ARDAFaucet...");
        ARDAFaucet faucet = new ARDAFaucet(address(usdc));
        console2.log("   ARDAFaucet deployed at:", address(faucet));

        // 5. Mint initial USDC to faucet (100M USDC for testing)
        console2.log("");
        console2.log("5. Funding faucet with initial USDC...");
        uint256 initialFaucetAmount = 100_000_000 * 10**6; // 100M USDC
        usdc.mint(address(faucet), initialFaucetAmount);
        console2.log("   Minted", initialFaucetAmount / 10**6, "USDC to faucet");

        // 6. Authorize deployer to post state updates
        console2.log("");
        console2.log("6. Setting up permissions...");
        arda.setAuthorization(deployer, true);
        console2.log("   Deployer authorized to post updates");

        vm.stopBroadcast();

        // Print deployment summary
        console2.log("");
        console2.log("==================================");
        console2.log("ANVIL DEPLOYMENT COMPLETE");
        console2.log("==================================");
        console2.log("");
        console2.log("Contract Addresses:");
        console2.log("  MockUSDC:    ", address(usdc));
        console2.log("  Verifier:    ", verifier);
        console2.log("  ARDA:        ", address(arda));
        console2.log("  Faucet:      ", address(faucet));
        console2.log("");
        console2.log("Configuration:");
        console2.log("  Deployer:    ", deployer);
        console2.log("  Program VKey:", vm.toString(programVKey));
        console2.log("  Faucet Amount: 1000000 USDC per request");
        console2.log("  Cooldown:     1 hour(s)");
        console2.log("");
        console2.log("Add these to your .env file:");
        console2.log("  ETHEREUM_RPC_URL=http://127.0.0.1:8545");
        console2.log("  ETHEREUM_CONTRACT_ADDRESS=", vm.toString(address(arda)));
        console2.log("  ETHEREUM_DEPLOYER_ADDRESS=", vm.toString(deployer));
        console2.log("  VERIFIER_CONTRACT_ADDRESS=", vm.toString(verifier));
        console2.log("  USDC_CONTRACT_ADDRESS=", vm.toString(address(usdc)));
        console2.log("  FAUCET_CONTRACT_ADDRESS=", vm.toString(address(faucet)));
        console2.log("");

        // Save deployment to JSON file
        _saveDeployment(
            getEnvironment(),
            address(arda),
            address(usdc),
            address(faucet),
            verifier,
            programVKey,
            deployer
        );
    }

    /*//////////////////////////////////////////////////////////////
                      PRODUCTION/SANDBOX DEPLOYMENT
    //////////////////////////////////////////////////////////////*/

    /// @notice Deploy for production/sandbox (no mocks)
    function deployProduction() internal {
        console2.log("Deploying PRODUCTION/SANDBOX environment...");
        console2.log("");

        // Get private key - handle both with and without 0x prefix
        string memory pkString = vm.envString("ETHEREUM_WALLET_PRIVATE_KEY");
        uint256 deployerPrivateKey;
        
        // Check if it starts with 0x
        if (bytes(pkString).length >= 2 && bytes(pkString)[0] == "0" && bytes(pkString)[1] == "x") {
            deployerPrivateKey = vm.parseUint(pkString);
        } else {
            // Add 0x prefix
            deployerPrivateKey = vm.parseUint(string.concat("0x", pkString));
        }
        
        address deployer = vm.addr(deployerPrivateKey);

        console2.log("Deployer address:", deployer);
        console2.log("");

        // Required environment variables for production
        address verifier = vm.envAddress("VERIFIER_CONTRACT_ADDRESS");
        bytes32 programVKey = vm.envBytes32("PROGRAM_VKEY");
        
        // Optional: Real USDC address (uses default if not provided)
        address usdcAddress = vm.envOr(
            "USDC_CONTRACT_ADDRESS",
            address(0xA0b86a33E6411cFd2BfB35B2bC5D2c60d7ED1BA6) // Default Sepolia USDC
        );

        vm.startBroadcast(deployerPrivateKey);

        // Deploy ARDA Contract
        console2.log("Deploying ARDA contract...");
        ARDA arda = new ARDA(verifier, programVKey);
        console2.log("   ARDA deployed at:", address(arda));

        // Set USDC token if different from default
        if (usdcAddress != address(0xA0b86a33E6411cFd2BfB35B2bC5D2c60d7ED1BA6)) {
            arda.setUsdcTokenForTesting(usdcAddress);
            console2.log("   Custom USDC token configured:", usdcAddress);
        }

        // Authorize deployer
        arda.setAuthorization(deployer, true);
        console2.log("   Deployer authorized to post updates");

        vm.stopBroadcast();

        // Print deployment summary
        console2.log("");
        console2.log("==================================");
        console2.log("PRODUCTION DEPLOYMENT COMPLETE");
        console2.log("==================================");
        console2.log("");
        console2.log("Contract Addresses:");
        console2.log("  ARDA:        ", address(arda));
        console2.log("  Verifier:    ", verifier);
        console2.log("  USDC:        ", usdcAddress);
        console2.log("");
        console2.log("Configuration:");
        console2.log("  Deployer:    ", deployer);
        console2.log("  Program VKey:", vm.toString(programVKey));
        console2.log("");
        console2.log("Add these to your .env file:");
        console2.log("  ETHEREUM_CONTRACT_ADDRESS=", vm.toString(address(arda)));
        console2.log("  ETHEREUM_DEPLOYER_ADDRESS=", vm.toString(deployer));
        console2.log("");

        // Save deployment to JSON file
        _saveDeployment(
            getEnvironment(),
            address(arda),
            usdcAddress,
            address(0), // No faucet in production
            verifier,
            programVKey,
            deployer
        );
    }

    /*//////////////////////////////////////////////////////////////
                          DEPLOYMENT TRACKING
    //////////////////////////////////////////////////////////////*/

    /// @dev Save deployment info to JSON file for tracking
    function _saveDeployment(
        string memory environment,
        address ardaAddress,
        address usdcAddress,
        address faucetAddress,
        address verifierAddress,
        bytes32 programVKey,
        address deployer
    ) internal {
        string memory jsonKey = "deployment";
        
        vm.serializeString(jsonKey, "environment", environment);
        vm.serializeAddress(jsonKey, "arda", ardaAddress);
        vm.serializeAddress(jsonKey, "usdc", usdcAddress);
        vm.serializeAddress(jsonKey, "faucet", faucetAddress);
        vm.serializeAddress(jsonKey, "verifier", verifierAddress);
        vm.serializeBytes32(jsonKey, "programVKey", programVKey);
        vm.serializeAddress(jsonKey, "deployer", deployer);
        vm.serializeUint(jsonKey, "timestamp", block.timestamp);
        vm.serializeUint(jsonKey, "chainId", block.chainid);
        
        string memory json = vm.serializeUint(jsonKey, "blockNumber", block.number);
        
        string memory filename = string.concat(
            "deployments/",
            environment,
            "-deployment.json"
        );
        
        vm.writeJson(json, filename);
        console2.log("Deployment saved to:", filename);
    }
}

