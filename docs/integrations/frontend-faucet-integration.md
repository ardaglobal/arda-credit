# Frontend Faucet Integration Guide

## Overview

This guide shows how to integrate the USDC faucet into the Arda Credit frontend for local/staging environments. The faucet provides automatic test USDC distribution during user onboarding, creating a smooth developer and tester experience.

## Architecture Overview

```
┌─────────────┐         ┌─────────────┐         ┌──────────────┐         ┌─────────────┐
│   Frontend  │────────▶│  Backend    │────────▶│   Anvil      │────────▶│   Faucet    │
│   (React)   │         │   API       │         │   RPC        │         │   Contract  │
└─────────────┘         └─────────────┘         └──────────────┘         └─────────────┘
      │                       │                         │                        │
      │ 1. Check environment  │                         │                        │
      ├──────────────────────▶│                         │                        │
      │                       │                         │                        │
      │ 2. Get faucet info    │                         │                        │
      │◀──────────────────────┤                         │                        │
      │                       │                         │                        │
      │ 3. User clicks "Request Test USDC"                                       │
      │                       │                         │                        │
      │ 4. Call faucet contract via MetaMask             │                        │
      ├──────────────────────────────────────────────────▶                       │
      │                       │                         │ requestFunds()         │
      │                       │                         ├───────────────────────▶│
      │                       │                         │                        │
      │ 5. Sign transaction   │                         │◀──────Emits Event─────┤
      │◀─MetaMask Popup───────┤                         │                        │
      │                       │                         │                        │
      │ 6. Transaction confirmed                         │                        │
      │◀──────────────────────────────────────────────────                       │
      │                       │                         │                        │
      │ 7. Balance updated! (1M USDC)                    │                        │
      └───────────────────────┴─────────────────────────┴────────────────────────┘
```

## Backend API Endpoints

### 1. Get Faucet Information

**Endpoint:** `GET /api/v1/faucet/info`

Returns environment information and faucet contract details.

**Response:**
```json
{
  "available": true,
  "environment": "local",
  "faucet_address": "0xDc64a140Aa3E981100a9becA4E685f962f0cF6C9",
  "usdc_address": "0x5FbDB2315678afecb367f032d93F642f64180aa3",
  "amount_per_request": "1000000",
  "cooldown_seconds": 3600,
  "rpc_url": "http://127.0.0.1:8545",
  "chain_id": "31337",
  "network_config": {
    "network_name": "Local Anvil Devnet",
    "rpc_url": "http://127.0.0.1:8545",
    "chain_id": "31337",
    "currency_symbol": "ETH",
    "block_explorer_url": null
  }
}
```

### 2. Check Faucet Status

**Endpoint:** `GET /api/v1/faucet/status?address=0x...`

Returns whether a specific address can request funds.

**Response:**
```json
{
  "can_request_now": true,
  "time_until_next_request": 0,
  "user_balance": "1000000000000",
  "faucet_balance": "99000000000000",
  "error": null
}
```

### 3. Get Faucet Config

**Endpoint:** `GET /api/v1/faucet/config`

Returns faucet configuration parameters.

**Response:**
```json
{
  "environment": "local",
  "faucet_enabled": true,
  "amount_per_request_usdc": "1000000",
  "cooldown_hours": 1
}
```

## Frontend Implementation

### Step 1: Environment Detection Hook

Create a hook to detect if faucet is available:

```typescript
// hooks/useFaucet.ts
import { useState, useEffect } from 'react';
import axios from 'axios';

interface FaucetInfo {
  available: boolean;
  environment: string;
  faucet_address?: string;
  usdc_address?: string;
  amount_per_request?: string;
  cooldown_seconds?: number;
  rpc_url?: string;
  chain_id?: string;
  network_config?: {
    network_name: string;
    rpc_url: string;
    chain_id: string;
    currency_symbol: string;
    block_explorer_url?: string;
  };
}

export const useFaucetInfo = () => {
  const [faucetInfo, setFaucetInfo] = useState<FaucetInfo | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const fetchFaucetInfo = async () => {
      try {
        const response = await axios.get<FaucetInfo>('/api/v1/faucet/info');
        setFaucetInfo(response.data);
      } catch (err) {
        console.error('Failed to fetch faucet info:', err);
        setError('Failed to load faucet information');
      } finally {
        setLoading(false);
      }
    };

    fetchFaucetInfo();
  }, []);

  return { faucetInfo, loading, error };
};
```

### Step 2: MetaMask Network Setup Hook

Create a hook to add/switch to Anvil network in MetaMask:

```typescript
// hooks/useAnvilNetwork.ts
import { useState } from 'react';

interface NetworkConfig {
  network_name: string;
  rpc_url: string;
  chain_id: string;
  currency_symbol: string;
  block_explorer_url?: string;
}

export const useAnvilNetwork = () => {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const addAnvilNetwork = async (networkConfig: NetworkConfig) => {
    setLoading(true);
    setError(null);

    try {
      // Check if MetaMask is installed
      if (!window.ethereum) {
        throw new Error('MetaMask is not installed');
      }

      // Request network switch/add
      await window.ethereum.request({
        method: 'wallet_addEthereumChain',
        params: [
          {
            chainId: `0x${parseInt(networkConfig.chain_id).toString(16)}`, // Convert to hex
            chainName: networkConfig.network_name,
            nativeCurrency: {
              name: 'Ether',
              symbol: networkConfig.currency_symbol,
              decimals: 18,
            },
            rpcUrls: [networkConfig.rpc_url],
            blockExplorerUrls: networkConfig.block_explorer_url
              ? [networkConfig.block_explorer_url]
              : null,
          },
        ],
      });

      console.log('✅ Anvil network added to MetaMask');
      return true;
    } catch (err: any) {
      console.error('Failed to add network:', err);
      
      // User rejected request
      if (err.code === 4001) {
        setError('User rejected network addition');
      } else {
        setError(err.message || 'Failed to add network');
      }
      
      return false;
    } finally {
      setLoading(false);
    }
  };

  const switchToAnvilNetwork = async (chainId: string) => {
    try {
      await window.ethereum.request({
        method: 'wallet_switchEthereumChain',
        params: [{ chainId: `0x${parseInt(chainId).toString(16)}` }],
      });
      return true;
    } catch (switchError: any) {
      // Network not added yet, try adding it
      if (switchError.code === 4902) {
        return false; // Caller should add the network
      }
      throw switchError;
    }
  };

  return { addAnvilNetwork, switchToAnvilNetwork, loading, error };
};
```

### Step 3: Faucet Request Hook

Create a hook to request funds from the faucet contract:

```typescript
// hooks/useFaucetRequest.ts
import { useState } from 'react';
import { ethers } from 'ethers';

// Faucet contract ABI (only the functions we need)
const FAUCET_ABI = [
  'function requestFunds() external',
  'function canRequestNow(address user) external view returns (bool)',
  'function getTimeUntilNextRequest(address user) external view returns (uint256)',
  'function getFaucetBalance() external view returns (uint256)',
  'event FundsRequested(address indexed recipient, uint256 amount, uint256 timestamp)',
];

export const useFaucetRequest = (faucetAddress: string, rpcUrl: string) => {
  const [requesting, setRequesting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [txHash, setTxHash] = useState<string | null>(null);

  const requestFunds = async () => {
    setRequesting(true);
    setError(null);
    setTxHash(null);

    try {
      // Check if MetaMask is installed
      if (!window.ethereum) {
        throw new Error('MetaMask is not installed. Please install MetaMask to continue.');
      }

      // Request account access
      const accounts = await window.ethereum.request({
        method: 'eth_requestAccounts',
      });

      if (!accounts || accounts.length === 0) {
        throw new Error('No accounts found. Please connect your wallet.');
      }

      const userAddress = accounts[0];

      // Create provider and signer
      const provider = new ethers.BrowserProvider(window.ethereum);
      const signer = await provider.getSigner();

      // Create contract instance
      const faucetContract = new ethers.Contract(
        faucetAddress,
        FAUCET_ABI,
        signer
      );

      // Check if user can request now
      const canRequest = await faucetContract.canRequestNow(userAddress);
      
      if (!canRequest) {
        const timeLeft = await faucetContract.getTimeUntilNextRequest(userAddress);
        const hoursLeft = Math.ceil(Number(timeLeft) / 3600);
        throw new Error(
          `Cooldown active. Please wait ${hoursLeft} hour(s) before requesting again.`
        );
      }

      // Request funds from faucet
      console.log('🚀 Requesting funds from faucet...');
      const tx = await faucetContract.requestFunds();
      
      console.log('⏳ Waiting for transaction confirmation...');
      setTxHash(tx.hash);
      
      // Wait for confirmation
      const receipt = await tx.wait();
      
      console.log('✅ Faucet request confirmed!', receipt);
      
      // Return success
      return {
        success: true,
        txHash: receipt.hash,
        blockNumber: receipt.blockNumber,
      };
      
    } catch (err: any) {
      console.error('❌ Faucet request failed:', err);
      
      // Parse error message
      let errorMessage = 'Failed to request funds';
      
      if (err.message?.includes('Cooldown')) {
        errorMessage = err.message;
      } else if (err.code === 4001) {
        errorMessage = 'Transaction rejected by user';
      } else if (err.message?.includes('InsufficientFaucetBalance')) {
        errorMessage = 'Faucet is out of funds. Please contact support.';
      } else if (err.message) {
        errorMessage = err.message;
      }
      
      setError(errorMessage);
      
      return {
        success: false,
        error: errorMessage,
      };
    } finally {
      setRequesting(false);
    }
  };

  return { requestFunds, requesting, error, txHash };
};
```

### Step 4: Faucet UI Component

Create a reusable faucet component:

```typescript
// components/FaucetButton.tsx
import React, { useState, useEffect } from 'react';
import { useFaucetInfo } from '../hooks/useFaucetInfo';
import { useAnvilNetwork } from '../hooks/useAnvilNetwork';
import { useFaucetRequest } from '../hooks/useFaucetRequest';
import { useAccount, useNetwork } from 'wagmi'; // If using wagmi for wallet connection

interface FaucetButtonProps {
  onSuccess?: (txHash: string) => void;
  onError?: (error: string) => void;
}

export const FaucetButton: React.FC<FaucetButtonProps> = ({ onSuccess, onError }) => {
  const { faucetInfo, loading: infoLoading } = useFaucetInfo();
  const { addAnvilNetwork, switchToAnvilNetwork } = useAnvilNetwork();
  const { address, isConnected } = useAccount(); // Using wagmi
  const { chain } = useNetwork();
  
  const [needsNetworkSwitch, setNeedsNetworkSwitch] = useState(false);
  const [addingNetwork, setAddingNetwork] = useState(false);

  const { requestFunds, requesting, error, txHash } = useFaucetRequest(
    faucetInfo?.faucet_address || '',
    faucetInfo?.rpc_url || ''
  );

  // Check if we're on the correct network
  useEffect(() => {
    if (faucetInfo && chain) {
      const expectedChainId = parseInt(faucetInfo.chain_id || '31337');
      setNeedsNetworkSwitch(chain.id !== expectedChainId);
    }
  }, [faucetInfo, chain]);

  // Don't show faucet button if not available
  if (infoLoading) {
    return <div className="text-gray-500">Loading faucet info...</div>;
  }

  if (!faucetInfo?.available) {
    return null; // Don't show in production/sandbox
  }

  // Handle network switch
  const handleNetworkSetup = async () => {
    if (!faucetInfo?.network_config) return;

    setAddingNetwork(true);

    try {
      // Try to switch first
      const switched = await switchToAnvilNetwork(faucetInfo.chain_id!);
      
      if (!switched) {
        // Network not added, add it
        await addAnvilNetwork(faucetInfo.network_config);
      }

      setNeedsNetworkSwitch(false);
    } catch (err) {
      console.error('Network setup failed:', err);
    } finally {
      setAddingNetwork(false);
    }
  };

  // Handle faucet request
  const handleFaucetRequest = async () => {
    if (!isConnected) {
      alert('Please connect your wallet first');
      return;
    }

    if (needsNetworkSwitch) {
      await handleNetworkSetup();
      return;
    }

    const result = await requestFunds();

    if (result.success && result.txHash) {
      onSuccess?.(result.txHash);
    } else if (result.error) {
      onError?.(result.error);
    }
  };

  return (
    <div className="faucet-container">
      {/* Environment Badge */}
      <div className="mb-4 inline-flex items-center px-3 py-1 rounded-full text-sm bg-yellow-100 text-yellow-800">
        <span className="mr-2">🧪</span>
        <span className="font-medium">{faucetInfo.environment.toUpperCase()} Environment</span>
      </div>

      {/* Network Switch Warning */}
      {needsNetworkSwitch && (
        <div className="mb-4 p-4 bg-yellow-50 border border-yellow-200 rounded-lg">
          <p className="text-sm text-yellow-800 mb-2">
            ⚠️ Please switch to {faucetInfo.network_config?.network_name}
          </p>
          <button
            onClick={handleNetworkSetup}
            disabled={addingNetwork}
            className="px-4 py-2 bg-yellow-600 text-white rounded hover:bg-yellow-700 disabled:opacity-50"
          >
            {addingNetwork ? 'Adding Network...' : 'Add/Switch to Local Network'}
          </button>
        </div>
      )}

      {/* Faucet Request Button */}
      <div className="faucet-card p-6 bg-gradient-to-br from-blue-50 to-indigo-50 rounded-lg border border-blue-200">
        <div className="flex items-start justify-between">
          <div className="flex-1">
            <h3 className="text-lg font-semibold text-gray-900 mb-2">
              💰 Test USDC Faucet
            </h3>
            <p className="text-sm text-gray-600 mb-4">
              Request {parseInt(faucetInfo.amount_per_request || '0').toLocaleString()} USDC for testing
            </p>
            
            {error && (
              <div className="mb-4 p-3 bg-red-50 border border-red-200 rounded text-sm text-red-700">
                {error}
              </div>
            )}
            
            {txHash && !error && (
              <div className="mb-4 p-3 bg-green-50 border border-green-200 rounded text-sm text-green-700">
                ✅ Funds requested! TX: {txHash.substring(0, 10)}...
              </div>
            )}
          </div>
        </div>

        <button
          onClick={handleFaucetRequest}
          disabled={!isConnected || requesting || needsNetworkSwitch}
          className="w-full px-6 py-3 bg-blue-600 text-white font-medium rounded-lg hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
        >
          {requesting
            ? '⏳ Requesting...'
            : needsNetworkSwitch
            ? '⚠️ Switch Network First'
            : !isConnected
            ? '🔌 Connect Wallet First'
            : '💰 Request Test USDC'}
        </button>

        <div className="mt-4 text-xs text-gray-500 text-center">
          <p>Cooldown: {(faucetInfo.cooldown_seconds || 3600) / 3600} hour between requests</p>
          <p className="mt-1">Faucet: {faucetInfo.faucet_address?.substring(0, 10)}...</p>
        </div>
      </div>
    </div>
  );
};
```

### Step 5: Onboarding Integration

Integrate the faucet into your onboarding flow:

```typescript
// pages/Onboarding.tsx
import React, { useState } from 'react';
import { FaucetButton } from '../components/FaucetButton';
import { useFaucetInfo } from '../hooks/useFaucetInfo';

export const OnboardingPage: React.FC = () => {
  const { faucetInfo } = useFaucetInfo();
  const [step, setStep] = useState(1);
  const [faucetTxHash, setFaucetTxHash] = useState<string | null>(null);

  const handleFaucetSuccess = (txHash: string) => {
    console.log('✅ Faucet request successful:', txHash);
    setFaucetTxHash(txHash);
    
    // Show success message
    alert('Success! You received 1M test USDC. Check your wallet balance.');
    
    // Auto-advance to next step
    setTimeout(() => setStep(step + 1), 2000);
  };

  const handleFaucetError = (error: string) => {
    console.error('❌ Faucet request failed:', error);
    alert(`Faucet request failed: ${error}`);
  };

  return (
    <div className="onboarding-container max-w-2xl mx-auto p-6">
      <h1 className="text-3xl font-bold mb-8">Welcome to Arda Credit</h1>

      {/* Step 1: Connect Wallet */}
      {step === 1 && (
        <div className="step-1">
          <h2 className="text-xl font-semibold mb-4">Step 1: Connect Your Wallet</h2>
          <ConnectWalletButton onConnect={() => setStep(2)} />
        </div>
      )}

      {/* Step 2: Get Test USDC (only in local/staging) */}
      {step === 2 && faucetInfo?.available && (
        <div className="step-2">
          <h2 className="text-xl font-semibold mb-4">Step 2: Get Test USDC</h2>
          <p className="text-gray-600 mb-4">
            To get started with testing, you'll need some test USDC. Click the button below to
            receive 1 million test USDC tokens.
          </p>
          <FaucetButton
            onSuccess={handleFaucetSuccess}
            onError={handleFaucetError}
          />
          
          <button
            onClick={() => setStep(3)}
            className="mt-4 text-blue-600 hover:underline"
          >
            Skip this step →
          </button>
        </div>
      )}

      {/* Step 3: Continue with onboarding */}
      {step === 3 && (
        <div className="step-3">
          <h2 className="text-xl font-semibold mb-4">Step 3: Complete Your Profile</h2>
          {/* Rest of onboarding flow */}
        </div>
      )}
    </div>
  );
};
```

### Step 6: USDC Balance Display

Show USDC balance with faucet option:

```typescript
// components/USDCBalance.tsx
import React from 'react';
import { useState, useEffect } from 'react';
import { ethers } from 'ethers';
import { useFaucetInfo } from '../hooks/useFaucetInfo';
import { FaucetButton } from './FaucetButton';

const ERC20_ABI = [
  'function balanceOf(address owner) view returns (uint256)',
  'function decimals() view returns (uint8)',
  'function symbol() view returns (string)',
];

interface USDCBalanceProps {
  address: string;
}

export const USDCBalance: React.FC<USDCBalanceProps> = ({ address }) => {
  const { faucetInfo } = useFaucetInfo();
  const [balance, setBalance] = useState<string>('0');
  const [loading, setLoading] = useState(true);
  const [showFaucet, setShowFaucet] = useState(false);

  useEffect(() => {
    const fetchBalance = async () => {
      if (!faucetInfo?.usdc_address || !address) return;

      try {
        const provider = new ethers.JsonRpcProvider(faucetInfo.rpc_url);
        const usdcContract = new ethers.Contract(
          faucetInfo.usdc_address,
          ERC20_ABI,
          provider
        );

        const balanceWei = await usdcContract.balanceOf(address);
        const decimals = await usdcContract.decimals();
        
        // Convert to human-readable format
        const balanceFormatted = ethers.formatUnits(balanceWei, decimals);
        setBalance(balanceFormatted);
      } catch (err) {
        console.error('Failed to fetch USDC balance:', err);
      } finally {
        setLoading(false);
      }
    };

    fetchBalance();
    
    // Refresh balance every 10 seconds
    const interval = setInterval(fetchBalance, 10000);
    return () => clearInterval(interval);
  }, [faucetInfo, address]);

  return (
    <div className="usdc-balance-container">
      <div className="flex items-center justify-between p-4 bg-white rounded-lg shadow">
        <div>
          <p className="text-sm text-gray-500">USDC Balance</p>
          <p className="text-2xl font-bold text-gray-900">
            {loading ? '...' : parseFloat(balance).toLocaleString()} USDC
          </p>
        </div>
        
        {/* Show "Get Test USDC" button in local/staging */}
        {faucetInfo?.available && (
          <button
            onClick={() => setShowFaucet(!showFaucet)}
            className="px-4 py-2 bg-blue-600 text-white text-sm rounded hover:bg-blue-700"
          >
            💰 Get Test USDC
          </button>
        )}
      </div>

      {/* Faucet modal/drawer */}
      {showFaucet && faucetInfo?.available && (
        <div className="mt-4">
          <FaucetButton
            onSuccess={(txHash) => {
              console.log('Faucet success:', txHash);
              setShowFaucet(false);
              // Refresh balance immediately
              setTimeout(() => window.location.reload(), 2000);
            }}
            onError={(err) => {
              console.error('Faucet error:', err);
            }}
          />
        </div>
      )}
    </div>
  );
};
```

## Complete Integration Example

Here's a full page that combines everything:

```typescript
// pages/Dashboard.tsx
import React, { useEffect, useState } from 'react';
import { useAccount, useConnect, useDisconnect, useNetwork, useSwitchNetwork } from 'wagmi';
import { InjectedConnector } from 'wagmi/connectors/injected';
import { useFaucetInfo } from '../hooks/useFaucetInfo';
import { useAnvilNetwork } from '../hooks/useAnvilNetwork';
import { FaucetButton } from '../components/FaucetButton';
import { USDCBalance } from '../components/USDCBalance';

export const Dashboard: React.FC = () => {
  const { address, isConnected } = useAccount();
  const { connect } = useConnect({
    connector: new InjectedConnector(),
  });
  const { disconnect } = useDisconnect();
  const { chain } = useNetwork();
  const { switchNetwork } = useSwitchNetwork();
  
  const { faucetInfo, loading: faucetLoading } = useFaucetInfo();
  const { addAnvilNetwork } = useAnvilNetwork();

  const [showFaucetOnboarding, setShowFaucetOnboarding] = useState(false);

  // Auto-show faucet onboarding for new users in local/staging
  useEffect(() => {
    if (isConnected && faucetInfo?.available) {
      // Check if user has requested funds before (you'd store this in localStorage or backend)
      const hasRequestedBefore = localStorage.getItem('faucet_requested');
      
      if (!hasRequestedBefore) {
        setShowFaucetOnboarding(true);
      }
    }
  }, [isConnected, faucetInfo]);

  const handleFaucetSuccess = (txHash: string) => {
    // Mark that user has requested from faucet
    localStorage.setItem('faucet_requested', 'true');
    localStorage.setItem('faucet_tx_hash', txHash);
    setShowFaucetOnboarding(false);

    // Show success notification
    alert('🎉 Success! You received 1M test USDC');
  };

  return (
    <div className="dashboard p-8">
      {/* Header */}
      <div className="mb-8">
        <h1 className="text-3xl font-bold">Arda Credit Dashboard</h1>
        
        {/* Environment indicator (only show in local/staging) */}
        {faucetInfo?.available && (
          <div className="mt-2 inline-flex items-center px-3 py-1 rounded-full bg-yellow-100 text-yellow-800 text-sm">
            <span className="mr-2">🧪</span>
            <span>Testing Environment ({faucetInfo.environment})</span>
          </div>
        )}
      </div>

      {/* Wallet Connection */}
      <div className="mb-8">
        {!isConnected ? (
          <button
            onClick={() => connect()}
            className="px-6 py-3 bg-indigo-600 text-white font-medium rounded-lg hover:bg-indigo-700"
          >
            Connect Wallet
          </button>
        ) : (
          <div className="flex items-center gap-4">
            <div className="flex items-center px-4 py-2 bg-green-50 border border-green-200 rounded-lg">
              <span className="mr-2">🟢</span>
              <span className="text-sm font-mono">{address?.substring(0, 10)}...</span>
            </div>
            <button
              onClick={() => disconnect()}
              className="px-4 py-2 text-sm text-gray-600 hover:text-gray-800"
            >
              Disconnect
            </button>
          </div>
        )}
      </div>

      {/* USDC Balance (with embedded faucet button) */}
      {isConnected && address && (
        <div className="mb-8">
          <USDCBalance address={address} />
        </div>
      )}

      {/* Faucet Onboarding Modal */}
      {showFaucetOnboarding && faucetInfo?.available && (
        <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
          <div className="bg-white rounded-lg p-8 max-w-lg">
            <h2 className="text-2xl font-bold mb-4">🎁 Welcome!</h2>
            <p className="text-gray-600 mb-6">
              You're in the {faucetInfo.environment} testing environment.
              Would you like to receive 1 million test USDC to get started?
            </p>
            
            <FaucetButton
              onSuccess={handleFaucetSuccess}
              onError={(err) => {
                console.error('Faucet error:', err);
                alert(`Failed to request funds: ${err}`);
              }}
            />

            <button
              onClick={() => setShowFaucetOnboarding(false)}
              className="mt-4 w-full text-sm text-gray-500 hover:text-gray-700"
            >
              Skip for now
            </button>
          </div>
        </div>
      )}

      {/* Rest of dashboard content */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
        {/* Your existing dashboard components */}
      </div>
    </div>
  );
};
```

## TypeScript Declarations

Add these to your global type definitions:

```typescript
// types/ethereum.d.ts
interface Window {
  ethereum?: {
    request: (args: { method: string; params?: any[] }) => Promise<any>;
    on: (event: string, handler: (...args: any[]) => void) => void;
    removeListener: (event: string, handler: (...args: any[]) => void) => void;
    isMetaMask?: boolean;
  };
}
```

## Key Features to Implement

### 1. **Auto-Detection** (Highest Priority)
- Check `GET /api/v1/faucet/info` on app load
- Only show faucet UI if `available: true`
- Display environment badge (local/dev/staging/sandbox/production)

### 2. **Network Switching** (Critical for UX)
- Detect if user is on wrong network
- Prompt to add/switch to Anvil network
- Use MetaMask's `wallet_addEthereumChain` API
- Handle rejection gracefully

### 3. **Smart Onboarding** (Recommended)
- Show faucet modal automatically for new users
- Track in localStorage if user already requested
- Optional skip button
- Show success animation

### 4. **Error Handling** (Important)
- Cooldown errors → Show time remaining
- Insufficient balance errors → "Faucet empty"
- User rejection → "Transaction cancelled"
- Network errors → "Check MetaMask connection"

### 5. **Balance Refresh** (Nice to Have)
- Auto-refresh balance every 10s
- Manual refresh button
- Show pending state during faucet request
- Optimistic UI updates

## Testing Checklist

### Frontend Testing

- [ ] Faucet button appears in local/staging
- [ ] Faucet button hidden in sandbox/production
- [ ] Network switch prompt works
- [ ] MetaMask popup appears on request
- [ ] Balance updates after successful request
- [ ] Cooldown error shows correctly
- [ ] Transaction hash displayed
- [ ] Retry works after error

### Integration Testing

- [ ] Backend API returns correct faucet info
- [ ] Contract addresses match deployment file
- [ ] Transaction succeeds on Anvil
- [ ] Deposit monitor detects faucet transfer (optional)
- [ ] Balance reflected in database (if user linked wallet)

## API Call Examples

### Check Faucet Availability

```typescript
const response = await fetch('/api/v1/faucet/info');
const data = await response.json();

if (data.available) {
  // Show faucet UI
  console.log('Faucet available at:', data.faucet_address);
} else {
  // Hide faucet UI (production environment)
  console.log('Faucet not available in', data.environment);
}
```

### Check User's Cooldown Status

```typescript
const address = '0x...';
const response = await fetch(`/api/v1/faucet/status?address=${address}`);
const status = await response.json();

if (status.can_request_now) {
  // Enable faucet button
} else {
  // Show cooldown message
  const hoursLeft = Math.ceil(status.time_until_next_request / 3600);
  console.log(`Cooldown: ${hoursLeft} hours remaining`);
}
```

## MetaMask Integration Best Practices

### 1. Always Check for MetaMask

```typescript
if (!window.ethereum?.isMetaMask) {
  alert('Please install MetaMask to continue');
  window.open('https://metamask.io/download/', '_blank');
  return;
}
```

### 2. Handle Network Switching

```typescript
try {
  await window.ethereum.request({
    method: 'wallet_switchEthereumChain',
    params: [{ chainId: '0x7a69' }], // 31337 in hex
  });
} catch (switchError: any) {
  // Network not added, add it
  if (switchError.code === 4902) {
    await window.ethereum.request({
      method: 'wallet_addEthereumChain',
      params: [{
        chainId: '0x7a69',
        chainName: 'Local Anvil Devnet',
        rpcUrls: ['http://127.0.0.1:8545'],
        nativeCurrency: {
          name: 'Ether',
          symbol: 'ETH',
          decimals: 18
        }
      }]
    });
  }
}
```

### 3. Listen for Account/Network Changes

```typescript
useEffect(() => {
  if (window.ethereum) {
    const handleAccountsChanged = (accounts: string[]) => {
      console.log('Account changed:', accounts[0]);
      setAddress(accounts[0]);
    };

    const handleChainChanged = (chainId: string) => {
      console.log('Network changed:', chainId);
      // Reload the page (recommended by MetaMask)
      window.location.reload();
    };

    window.ethereum.on('accountsChanged', handleAccountsChanged);
    window.ethereum.on('chainChanged', handleChainChanged);

    return () => {
      window.ethereum?.removeListener('accountsChanged', handleAccountsChanged);
      window.ethereum?.removeListener('chainChanged', handleChainChanged);
    };
  }
}, []);
```

## Common Issues & Solutions

### Issue: "MetaMask not connected to correct network"

**Solution:**
Automatically prompt user to switch:
```typescript
await switchToAnvilNetwork(faucetInfo.chain_id);
```

### Issue: "Transaction fails silently"

**Solution:**
Add proper error handling and user feedback:
```typescript
try {
  const tx = await faucetContract.requestFunds();
  await tx.wait();
  showSuccessToast('Received 1M USDC!');
} catch (err) {
  showErrorToast(parseContractError(err));
}
```

### Issue: "Balance doesn't update immediately"

**Solution:**
Use optimistic UI update or force refresh:
```typescript
// Optimistic update
setBalance((prev) => (parseFloat(prev) + 1000000).toString());

// Or wait for transaction and refresh
await tx.wait();
await fetchBalance();
```

## Summary

**What the Backend Provides:**
- ✅ Environment detection API (`/faucet/info`)
- ✅ Faucet status check API (`/faucet/status`)
- ✅ Configuration API (`/faucet/config`)

**What the Frontend Must Do:**
- 📱 Call backend API to detect environment
- 🔌 Integrate MetaMask for wallet connection
- 🌐 Add/switch to Anvil network
- 💰 Call faucet contract's `requestFunds()` function
- 🎨 Show appropriate UI based on environment
- ⏰ Handle cooldown errors gracefully

**Key Point:** The actual faucet request is a **direct contract call via MetaMask** - the backend just provides information about where the contract is and whether it's available.

This keeps the architecture clean: backend tells frontend what's available, frontend interacts directly with blockchain via user's wallet.

