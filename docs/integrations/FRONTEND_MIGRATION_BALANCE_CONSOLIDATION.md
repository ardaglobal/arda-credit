# Frontend Migration Guide: Account-Centric Balance Tracking

## Overview

The Arda Credit API has migrated from a user-centric financial tracking system to an **account-centric** model. This guide helps frontend developers migrate to the new account-based API structure.

## Summary of Changes

### Old System (DEPRECATED ❌)
- Multiple overlapping balance tables and endpoints
- User-centric and address-centric endpoints mixed
- Separate tracking for USDC deposits/withdrawals
- Manual balance updates via API calls
- No concept of "accounts" - direct address usage

### New System (CURRENT ✅)
- **Account-centric model**: Accounts are the primary entity
- Each account has one Ethereum address (one-to-one)
- Accounts can be owned by users or organizations
- Automatic balance updates via database triggers
- Generic deposit/withdrawal tracking (multi-currency)
- Unified withdrawal lifecycle in one table
- Account status controls operations (ACTIVE, INACTIVE, SUSPENDED)

---

## API Endpoint Migration

### Finding Your Account

#### **Step 1: Lookup Account by Ethereum Address**

When user connects their wallet, lookup their account:

```javascript
GET /api/v1/accounts?address=0x742d35Cc6640CA5AaAaB2AAD9d8e7f2B6E37b5D1

// Response:
{
  "accounts": [
    {
      "account_id": "550e8400-e29b-41d4-a716-446655440000",
      "account_label": "John Doe",
      "account_number": "0x742d35Cc6640CA5AaAaB2AAD9d8e7f2B6E37b5D1",
      "account_type": "OPERATING",
      "currency": "USDC",
      "status": "ACTIVE",
      "user_id": "123e4567-e89b-12d3-a456-426614174000",
      "org_id": null
    }
  ]
}
```

**Usage:**
```typescript
async function getAccountByAddress(address: string) {
  const response = await fetch(`/api/v1/accounts?address=${address}`);
  const data = await response.json();
  return data.accounts[0]; // One-to-one relationship
}

// Store account_id for subsequent API calls
const account = await getAccountByAddress(walletAddress);
localStorage.setItem('account_id', account.account_id);
```

### Balance Queries

#### **Getting Account Balance**

**OLD** ❌
```javascript
// DEPRECATED: Do not use
GET /api/v1/user/{user_id}/financial/account
GET /api/v1/address/{address}/balance
```

**NEW** ✅
```javascript
// Get balance for specific account (primary endpoint)
GET /api/v1/account/{account_id}/balance

// Response:
{
  "account_id": "550e8400-e29b-41d4-a716-446655440000",
  "account_label": "John Doe",
  "account_number": "0x742d35Cc6640CA5AaAaB2AAD9d8e7f2B6E37b5D1",
  "account_status": "ACTIVE",
  "balance": {
    "currency": "USDC",
    "available_balance": "5000000",      // Wei (USDC with 6 decimals)
    "available_balance_usdc": "5.000000", // Human-readable
    "locked_balance": "1000000",          // Locked in active deals
    "locked_balance_usdc": "1.000000",
    "total_deposited": "6000000",
    "total_withdrawn": "0",
    "deposit_count": 3,
    "withdrawal_count": 0
  }
}
```

**Or get all accounts for a user:**
```javascript
GET /api/v1/user/{user_id}/balances

// Response:
{
  "user_id": "123e4567-e89b-12d3-a456-426614174000",
  "accounts": [
    {
      "account_id": "550e8400-e29b-41d4-a716-446655440000",
      "account_label": "Operating USD",
      "account_number": "0x742d...",
      "status": "ACTIVE",
      "balances": [
        {
          "currency": "USDC",
          "available_balance_usdc": "5.000000",
          "locked_balance_usdc": "1.000000",
          ...
        }
      ]
    }
  ],
  "total_available_usdc": "5.000000"
}
```

**Migration Code:**
```javascript
// OLD
async function getUserBalance(userId) {
  const response = await fetch(`/api/v1/user/${userId}/financial/account`);
  const data = await response.json();
  return data.transactions; // Confusing response structure
}

// NEW (Account-Centric)
async function getUserBalances(userId) {
  const response = await fetch(`/api/v1/user/${userId}/balances`);
  const data = await response.json();
  return {
    totalAvailableUSDC: parseFloat(data.total_available_usdc),
    accounts: data.accounts.map(account => ({
      accountId: account.account_id,
      accountLabel: account.account_label,
      address: account.account_number,
      status: account.status,
      availableUSDC: parseFloat(account.balances[0].available_balance_usdc),
      lockedUSDC: parseFloat(account.balances[0].locked_balance_usdc),
      totalDepositedUSDC: parseFloat(account.balances[0].total_deposited_usdc),
      depositCount: account.balances[0].deposit_count
    }))
  };
}
```

#### **Getting Account Balance (Recommended Pattern)**

**NEW** ✅
```javascript
// After wallet connection, get account_id then fetch balance
const account = await getAccountByAddress(walletAddress);
const accountId = account.account_id;

GET /api/v1/account/{accountId}/balance

// Response:
{
  "account_id": "550e8400-e29b-41d4-a716-446655440000",
  "account_label": "John Doe",
  "account_number": "0x742d35Cc6640CA5AaAaB2AAD9d8e7f2B6E37b5D1",
  "account_status": "ACTIVE",
  "balance": {
    "currency": "USDC",
    "available_balance": "5000000",
    "available_balance_usdc": "5.000000",
    "locked_balance": "1000000",
    "locked_balance_usdc": "1.000000",
    "total_deposited": "6000000",
    "total_deposited_usdc": "6.000000",
    "total_withdrawn": "0",
    "total_withdrawn_usdc": "0.000000",
    "deposit_count": 3,
    "withdrawal_count": 0
  }
}
```

**Usage Example:**
```typescript
interface AccountBalance {
  account_id: string;
  account_label: string;
  account_number: string;  // Ethereum address
  account_status: string;
  balance: {
    currency: string;
    available_balance_usdc: string;
    locked_balance_usdc: string;
    total_deposited_usdc: string;
    total_withdrawn_usdc: string;
    deposit_count: number;
    withdrawal_count: number;
  };
}

async function getAccountBalance(accountId: string): Promise<AccountBalance> {
  const response = await fetch(`/api/v1/account/${accountId}/balance`);
  const data = await response.json();
  return data;
}

// Display in UI
function BalanceDisplay({ accountId }: { accountId: string }) {
  const [accountBalance, setAccountBalance] = useState<AccountBalance | null>(null);
  
  useEffect(() => {
    getAccountBalance(accountId).then(setAccountBalance);
  }, [accountId]);
  
  if (!accountBalance) return <div>Loading...</div>;
  
  const { balance, account_label, account_number, account_status } = accountBalance;
  
  return (
    <div>
      <h3>{account_label}</h3>
      <p className="address">{account_number}</p>
      <p className={`status ${account_status.toLowerCase()}`}>Status: {account_status}</p>
      
      <h4>Balance: ${balance.available_balance_usdc} USDC</h4>
      {balance.locked_balance_usdc !== '0' && (
        <p>Locked: ${balance.locked_balance_usdc} USDC</p>
      )}
      <p>Total Deposited: ${balance.total_deposited_usdc} USDC</p>
      <p>Deposits: {balance.deposit_count}</p>
    </div>
  );
}
```

---

### Deposit History

#### **Getting Deposit History**

**OLD** ❌
```javascript
// DEPRECATED: Do not use
GET /api/v1/user/{user_id}/transactions
GET /api/v1/address/{address}/deposits
```

**NEW** ✅
```javascript
// Get deposit history for specific account
GET /api/v1/account/{account_id}/deposits?limit=50&offset=0

// Response:
{
  "account_id": "550e8400-e29b-41d4-a716-446655440000",
  "deposits": [
    {
      "id": 123,
      "account_id": "550e8400-e29b-41d4-a716-446655440000",
      "address": "0x742d35Cc6640CA5AaAaB2AAD9d8e7f2B6E37b5D1",
      "amount": "5000000",          // Wei
      "amount_usdc": "5.000000",    // Human-readable
      "currency": "USDC",
      "transaction_hash": "0xabc...def",
      "block_number": 18234567,
      "status": "confirmed",
      "confirmation_count": 12,
      "created_at": "2025-01-15T10:30:00Z",
      "processed_at": "2025-01-15T10:35:00Z"
    }
  ],
  "total_count": 3,
  "has_more": false
}
```

**Migration Code:**
```javascript
// OLD
async function getDepositHistory(userId) {
  const response = await fetch(`/api/v1/user/${userId}/transactions`);
  const data = await response.json();
  return data.transactions.filter(tx => tx.transaction_type === 'DEPOSIT');
}

// NEW (Account-Centric)
async function getDepositHistory(accountId, limit = 50, offset = 0) {
  const response = await fetch(
    `/api/v1/account/${accountId}/deposits?limit=${limit}&offset=${offset}`
  );
  const data = await response.json();
  return data.deposits;
}

// React Hook Example
function useDepositHistory(accountId: string) {
  const [deposits, setDeposits] = useState([]);
  const [loading, setLoading] = useState(true);
  
  useEffect(() => {
    if (!accountId) return;
    
    getDepositHistory(accountId)
      .then(data => setDeposits(data.deposits))
      .finally(() => setLoading(false));
  }, [accountId]);
  
  return { deposits, loading };
}

// Full pattern with account lookup
function useUserDeposits(walletAddress: string) {
  const [deposits, setDeposits] = useState([]);
  const [account, setAccount] = useState(null);
  
  useEffect(() => {
    if (!walletAddress) return;
    
    // 1. Lookup account by address
    getAccountByAddress(walletAddress)
      .then(acc => {
        setAccount(acc);
        // 2. Get deposits for that account
        return getDepositHistory(acc.account_id);
      })
      .then(data => setDeposits(data.deposits));
  }, [walletAddress]);
  
  return { deposits, account };
}
```

---

### Withdrawal Management

#### **Initiating Withdrawal**

**OLD** ❌
```javascript
// DEPRECATED
POST /api/v1/withdrawals
{
  "user_address": "0x...",
  "recipient_address": "0x...",
  "amount_usdc": "100.50"
}
```

**NEW** ✅ (Similar but with updated response)
```javascript
POST /api/v1/withdrawals
{
  "user_address": "0x742d35Cc6640CA5AaAaB2AAD9d8e7f2B6E37b5D1",
  "recipient_address": "0x1234567890123456789012345678901234567890",
  "amount_usdc": "100.50"
}

// Response:
{
  "request_id": 456,
  "state": "validated",             // NEW: Clearer state names
  "kyc_validated": true,
  "balance_validated": true,
  "amount_usdc": "100.50",
  "error_message": null,
  "validation_errors": null
}
```

**Migration Code:**
```typescript
// OLD
interface WithdrawalRequest {
  request_id: number;
  status: string;  // Ambiguous
  kyc_validated: boolean;
  balance_validated: boolean;
  amount_usdc: string;
}

// NEW
interface WithdrawalResponse {
  request_id: number;
  state: 'pending' | 'validated' | 'approved' | 'submitted' | 'confirmed' | 'failed' | 'cancelled';
  kyc_validated: boolean;
  balance_validated: boolean;
  amount_usdc: string;
  error_message?: string;
  validation_errors?: object;
}

async function initiateWithdrawal(
  userAddress: string,
  recipientAddress: string,
  amountUSDC: string
): Promise<WithdrawalResponse> {
  const response = await fetch('/api/v1/withdrawals', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      user_address: userAddress,
      recipient_address: recipientAddress,
      amount_usdc: amountUSDC
    })
  });
  
  if (!response.ok) {
    throw new Error(`Withdrawal failed: ${response.statusText}`);
  }
  
  return await response.json();
}
```

#### **Checking Withdrawal Status**

**OLD** ❌
```javascript
GET /api/v1/withdrawals/{request_id}
```

**NEW** ✅ (Updated response fields)
```javascript
GET /api/v1/withdrawals/{request_id}

// Response:
{
  "request_id": 456,
  "user_address": "0x742d35Cc6640CA5AaAaB2AAD9d8e7f2B6E37b5D1",
  "recipient_address": "0x1234567890123456789012345678901234567890",
  "amount_usdc": "100.50",
  "state": "submitted",            // Clear lifecycle state
  "kyc_validated": true,
  "balance_validated": true,
  "transaction_hash": "0xabc...def", // Present after submission
  "confirmation_count": 3,
  "required_confirmations": 12,
  "created_at": "2025-01-15T10:00:00Z",
  "validated_at": "2025-01-15T10:00:05Z",
  "approved_at": "2025-01-15T10:01:00Z",
  "submitted_at": "2025-01-15T10:02:00Z",
  "confirmed_at": null,  // Will be set when confirmed
  "processing_history": [  // NEW: Full audit trail
    {
      "event_type": "validation",
      "old_state": "pending",
      "new_state": "validated",
      "success": true,
      "timestamp": "2025-01-15T10:00:05Z"
    }
  ]
}
```

**React Component Example:**
```tsx
interface WithdrawalStatus {
  state: string;
  confirmation_count: number;
  required_confirmations: number;
  transaction_hash?: string;
  error_message?: string;
}

function WithdrawalStatusTracker({ withdrawalId }: { withdrawalId: number }) {
  const [status, setStatus] = useState<WithdrawalStatus | null>(null);
  
  useEffect(() => {
    // Poll every 5 seconds for updates
    const interval = setInterval(async () => {
      const response = await fetch(`/api/v1/withdrawals/${withdrawalId}`);
      const data = await response.json();
      setStatus(data);
      
      // Stop polling when finalized
      if (['confirmed', 'failed', 'cancelled'].includes(data.state)) {
        clearInterval(interval);
      }
    }, 5000);
    
    return () => clearInterval(interval);
  }, [withdrawalId]);
  
  if (!status) return <div>Loading...</div>;
  
  return (
    <div className="withdrawal-tracker">
      <h3>Withdrawal Status: {status.state}</h3>
      {status.transaction_hash && (
        <p>
          Transaction: 
          <a href={`https://etherscan.io/tx/${status.transaction_hash}`}>
            {status.transaction_hash.substring(0, 10)}...
          </a>
        </p>
      )}
      {status.state === 'submitted' && (
        <div className="progress">
          <p>Confirmations: {status.confirmation_count} / {status.required_confirmations}</p>
          <progress value={status.confirmation_count} max={status.required_confirmations} />
        </div>
      )}
      {status.error_message && (
        <div className="error">{status.error_message}</div>
      )}
    </div>
  );
}
```

#### **Cancelling Withdrawal**

**NEW** ✅ (Endpoint unchanged)
```javascript
POST /api/v1/withdrawals/{request_id}/cancel

// Response:
{
  "request_id": 456,
  "state": "cancelled",
  "cancelled_at": "2025-01-15T10:05:00Z",
  ...
}
```

**Usage:**
```typescript
async function cancelWithdrawal(withdrawalId: number): Promise<void> {
  const response = await fetch(`/api/v1/withdrawals/${withdrawalId}/cancel`, {
    method: 'POST'
  });
  
  if (!response.ok) {
    const error = await response.json();
    throw new Error(error.message || 'Cannot cancel withdrawal');
  }
}
```

---

### Withdrawal History

**OLD** ❌
```javascript
GET /api/v1/users/{address}/withdrawals
```

**NEW** ✅
```javascript
GET /api/v1/address/{address}/withdrawals?limit=50&offset=0

// Response:
{
  "withdrawals": [
    {
      "id": 456,
      "address": "0x742d35Cc6640CA5AaAaB2AAD9d8e7f2B6E37b5D1",
      "recipient_address": "0x1234567890123456789012345678901234567890",
      "amount": "100500000",
      "amount_usdc": "100.50",
      "currency": "USDC",
      "state": "confirmed",
      "transaction_hash": "0xabc...def",
      "created_at": "2025-01-15T10:00:00Z",
      "confirmed_at": "2025-01-15T10:10:00Z"
    }
  ],
  "total_count": 5,
  "has_more": false
}
```

---

## Key Behavioral Changes

### 1. **Automatic Balance Updates**

**OLD System:**
- Balances updated manually via API calls
- Required separate API calls to record deposits/withdrawals
- Inconsistent state between tables

**NEW System:**
- Balances updated **automatically** via database triggers
- When deposit status changes to 'confirmed' → balance increases
- When withdrawal state changes to 'confirmed' → balance decreases
- **No manual API calls needed**

### 2. **Withdrawal States**

**NEW Withdrawal Lifecycle:**
```
User Request Flow:
pending → validated → approved → submitted → confirmed

Contract Direct Flow:
submitted → confirmed

Failed/Cancelled:
pending/validated/approved → failed/cancelled
```

**State Meanings:**
- `pending`: Just created, validation in progress
- `validated`: KYC & balance checks passed
- `approved`: Admin approved (if required)
- `submitted`: Sent to blockchain
- `confirmed`: Blockchain confirmation received
- `failed`: Failed validation or execution
- `cancelled`: User or admin cancelled

**Frontend State Handling:**
```typescript
function getWithdrawalStatusColor(state: string): string {
  switch (state) {
    case 'pending': return 'yellow';
    case 'validated': return 'blue';
    case 'approved': return 'blue';
    case 'submitted': return 'orange';
    case 'confirmed': return 'green';
    case 'failed': return 'red';
    case 'cancelled': return 'gray';
    default: return 'gray';
  }
}

function getWithdrawalStatusLabel(state: string): string {
  switch (state) {
    case 'pending': return 'Validating...';
    case 'validated': return 'Awaiting Approval';
    case 'approved': return 'Approved - Pending Submission';
    case 'submitted': return 'Processing on Blockchain';
    case 'confirmed': return 'Completed';
    case 'failed': return 'Failed';
    case 'cancelled': return 'Cancelled';
    default: return 'Unknown';
  }
}
```

### 3. **Address-First Design**

**OLD System:**
- Operations keyed by user_id
- Required user-to-address lookup

**NEW System:**
- Operations keyed by Ethereum address
- Optional user/company linking for aggregation
- **Use wallet address directly in API calls**

**Example:**
```typescript
// OLD: Required user ID
const balance = await getUserFinancialSummary(userId);

// NEW: Use wallet address directly
const balance = await getAddressBalance(walletAddress);

// Or get all addresses for a user
const userBalances = await getUserBalances(userId);
// userBalances.balances = array of balances for each linked address
```

---

## Common Migration Patterns

### Pattern 1: Dashboard Balance Display

**OLD** ❌
```jsx
function UserDashboard({ userId }) {
  const [summary, setSummary] = useState(null);
  
  useEffect(() => {
    fetch(`/api/v1/user/${userId}/financial/summary`)
      .then(res => res.json())
      .then(setSummary);
  }, [userId]);
  
  return (
    <div>
      <h2>Balance: ${summary?.net_account_flow_usdc}</h2>
      <p>Deposits: ${summary?.total_deposits_usdc}</p>
      <p>Withdrawals: ${summary?.total_withdrawals_usdc}</p>
    </div>
  );
}
```

**NEW** ✅
```jsx
function UserDashboard({ userId }) {
  const [balances, setBalances] = useState(null);
  
  useEffect(() => {
    fetch(`/api/v1/user/${userId}/balance`)
      .then(res => res.json())
      .then(setBalances);
  }, [userId]);
  
  if (!balances) return <div>Loading...</div>;
  
  return (
    <div>
      <h2>Available Balance: ${balances.total_available_balance_usdc} USDC</h2>
      <p>Total Deposited: ${balances.balances[0]?.total_deposited_usdc}</p>
      <p>Total Withdrawn: ${balances.balances[0]?.total_withdrawn_usdc}</p>
      
      {/* Show all linked addresses */}
      {balances.balances.map(balance => (
        <div key={balance.address}>
          <p>{balance.address}: ${balance.available_balance_usdc} USDC</p>
        </div>
      ))}
    </div>
  );
}
```

### Pattern 2: Transaction History

**OLD** ❌
```jsx
function TransactionHistory({ userId }) {
  const [transactions, setTransactions] = useState([]);
  
  useEffect(() => {
    fetch(`/api/v1/user/${userId}/financial/transactions`)
      .then(res => res.json())
      .then(data => setTransactions(data.transactions));
  }, [userId]);
  
  return transactions.map(tx => (
    <div key={tx.id}>
      {tx.transaction_type}: ${tx.amount_usdc}
    </div>
  ));
}
```

**NEW** ✅
```jsx
function TransactionHistory({ address }) {
  const [deposits, setDeposits] = useState([]);
  const [withdrawals, setWithdrawals] = useState([]);
  
  useEffect(() => {
    // Fetch deposits and withdrawals separately
    Promise.all([
      fetch(`/api/v1/address/${address}/deposits?limit=50`).then(r => r.json()),
      fetch(`/api/v1/address/${address}/withdrawals?limit=50`).then(r => r.json())
    ]).then(([depositsData, withdrawalsData]) => {
      setDeposits(depositsData.deposits);
      setWithdrawals(withdrawalsData.withdrawals);
    });
  }, [address]);
  
  // Combine and sort by date
  const allTransactions = [
    ...deposits.map(d => ({ type: 'deposit', ...d })),
    ...withdrawals.map(w => ({ type: 'withdrawal', ...w }))
  ].sort((a, b) => new Date(b.created_at) - new Date(a.created_at));
  
  return allTransactions.map(tx => (
    <div key={`${tx.type}-${tx.id}`} className={tx.type}>
      {tx.type === 'deposit' ? '+' : '-'}${tx.amount_usdc} USDC
      <span className={`status-${tx.status || tx.state}`}>
        {tx.status || tx.state}
      </span>
    </div>
  ));
}
```

### Pattern 3: Withdrawal Form with Validation

```tsx
function WithdrawalForm({ userAddress }: { userAddress: string }) {
  const [amount, setAmount] = useState('');
  const [recipient, setRecipient] = useState('');
  const [balance, setBalance] = useState<AddressBalance | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState('');
  
  // Fetch balance on mount
  useEffect(() => {
    fetch(`/api/v1/address/${userAddress}/balance`)
      .then(res => res.json())
      .then(data => setBalance(data.balance));
  }, [userAddress]);
  
  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setSubmitting(true);
    setError('');
    
    try {
      // Validate amount
      const amountNum = parseFloat(amount);
      const availableNum = parseFloat(balance?.available_balance_usdc || '0');
      
      if (amountNum > availableNum) {
        setError(`Insufficient balance. Available: $${availableNum} USDC`);
        setSubmitting(false);
        return;
      }
      
      // Submit withdrawal
      const response = await fetch('/api/v1/withdrawals', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          user_address: userAddress,
          recipient_address: recipient,
          amount_usdc: amount
        })
      });
      
      if (!response.ok) {
        const errorData = await response.json();
        throw new Error(errorData.message || 'Withdrawal failed');
      }
      
      const result = await response.json();
      
      // Check validation results
      if (!result.kyc_validated) {
        setError('KYC validation required');
      } else if (!result.balance_validated) {
        setError('Insufficient balance');
      } else {
        // Success - navigate to status page
        window.location.href = `/withdrawals/${result.request_id}`;
      }
    } catch (err) {
      setError(err.message);
    } finally {
      setSubmitting(false);
    }
  };
  
  return (
    <form onSubmit={handleSubmit}>
      <h3>Withdraw USDC</h3>
      
      {balance && (
        <p className="balance-display">
          Available: ${balance.available_balance_usdc} USDC
          {balance.locked_balance_usdc && (
            <span className="locked">
              (${balance.locked_balance_usdc} locked in deals)
            </span>
          )}
        </p>
      )}
      
      <input
        type="text"
        placeholder="Amount (USDC)"
        value={amount}
        onChange={(e) => setAmount(e.target.value)}
        disabled={submitting}
      />
      
      <input
        type="text"
        placeholder="Recipient Address (0x...)"
        value={recipient}
        onChange={(e) => setRecipient(e.target.value)}
        disabled={submitting}
      />
      
      {error && <div className="error">{error}</div>}
      
      <button type="submit" disabled={submitting || !balance}>
        {submitting ? 'Processing...' : 'Withdraw'}
      </button>
    </form>
  );
}
```

---

## Complete Example: Wallet Integration

```typescript
// wallet.service.ts
export class WalletService {
  private baseUrl = 'https://api.arda.credit/api/v1';
  
  /**
   * Get complete balance information for an address
   */
  async getBalance(address: string): Promise<AddressBalance> {
    const response = await fetch(`${this.baseUrl}/address/${address}/balance`);
    const data = await response.json();
    return data.balance;
  }
  
  /**
   * Get deposit history for an address
   */
  async getDeposits(address: string, limit = 50, offset = 0) {
    const response = await fetch(
      `${this.baseUrl}/address/${address}/deposits?limit=${limit}&offset=${offset}`
    );
    const data = await response.json();
    return data.deposits;
  }
  
  /**
   * Get withdrawal history for an address
   */
  async getWithdrawals(address: string, limit = 50, offset = 0) {
    const response = await fetch(
      `${this.baseUrl}/address/${address}/withdrawals?limit=${limit}&offset=${offset}`
    );
    const data = await response.json();
    return data.withdrawals;
  }
  
  /**
   * Initiate withdrawal request
   */
  async initiateWithdrawal(
    userAddress: string,
    recipientAddress: string,
    amountUSDC: string
  ) {
    const response = await fetch(`${this.baseUrl}/withdrawals`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        user_address: userAddress,
        recipient_address: recipientAddress,
        amount_usdc: amountUSDC
      })
    });
    
    if (!response.ok) {
      throw new Error('Withdrawal failed');
    }
    
    return await response.json();
  }
  
  /**
   * Check withdrawal status
   */
  async getWithdrawalStatus(withdrawalId: number) {
    const response = await fetch(`${this.baseUrl}/withdrawals/${withdrawalId}`);
    return await response.json();
  }
  
  /**
   * Cancel pending withdrawal
   */
  async cancelWithdrawal(withdrawalId: number) {
    const response = await fetch(`${this.baseUrl}/withdrawals/${withdrawalId}/cancel`, {
      method: 'POST'
    });
    
    if (!response.ok) {
      throw new Error('Cannot cancel withdrawal');
    }
    
    return await response.json();
  }
}
```

---

## Testing Your Migration

### Checklist

- [ ] Update all balance fetching to use `/address/{address}/balance` or `/user/{user_id}/balance`
- [ ] Remove all calls to `/financial/*` endpoints
- [ ] Update withdrawal initiation to use new response format
- [ ] Handle new withdrawal states in UI (pending → validated → approved → submitted → confirmed)
- [ ] Update deposit history queries to use `/address/{address}/deposits`
- [ ] Update withdrawal history queries to use `/address/{address}/withdrawals`
- [ ] Test balance display after deposits
- [ ] Test balance locking during withdrawal approval
- [ ] Test balance release on withdrawal cancellation
- [ ] Test multi-address scenarios (if user has multiple linked addresses)

### Test Scenarios

#### Scenario 1: Fresh Deposit
```
1. Deposit USDC to contract
2. Wait for confirmations
3. Check balance → should show updated available_balance
4. Check deposits list → should show new deposit with 'confirmed' status
```

#### Scenario 2: Withdrawal Request
```
1. Get current balance
2. Initiate withdrawal (amount < available_balance)
3. Check balance → locked_balance should increase
4. Wait for approval/submission
5. Check balance → available_balance should decrease when confirmed
```

#### Scenario 3: Withdrawal Cancellation
```
1. Initiate withdrawal
2. Withdrawal goes to 'approved' state
3. Cancel withdrawal
4. Check balance → locked_balance should decrease back to available_balance
```

---

## Migration Script Template

Use this script to update your frontend codebase:

```bash
#!/bin/bash
# migrate_balance_endpoints.sh

echo "Migrating balance tracking endpoints..."

# Find and list all old endpoint usages
echo "\n🔍 Finding deprecated endpoints..."
grep -r "/financial/" src/ --include="*.ts" --include="*.tsx" --include="*.js" --include="*.jsx"

echo "\n⚠️  Please manually review and update the above files"
echo "\nOld patterns to replace:"
echo "  /user/{id}/financial/account → /user/{id}/balance"
echo "  /user/{id}/financial/transactions → /address/{addr}/deposits + /address/{addr}/withdrawals"
echo "  /user/{id}/financial/summary → /user/{id}/balance (calculate from returned data)"

# Suggest automated replacements (review before applying)
echo "\n📝 Suggested replacements (REVIEW BEFORE APPLYING):"
echo "  sed -i '' 's|/api/v1/user/\${userId}/financial/account|/api/v1/user/\${userId}/balance|g' src/**/*.{ts,tsx,js,jsx}"
```

---

## Troubleshooting

### "Balance is zero after deposit"
- **Check**: Is deposit status 'confirmed'? Balance only updates on confirmation.
- **Solution**: Poll `/address/{address}/deposits` to check deposit status

### "Withdrawal blocked with insufficient balance"
- **Check**: Is some balance locked? Check `locked_balance` field
- **Solution**: Wait for active deals to close, or adjust withdrawal amount

### "User has multiple addresses"
- **Check**: `/user/{user_id}/balance` returns array of balances
- **Solution**: Sum `balances[].available_balance_usdc` for total

### "Cannot find deposit history"
- **Check**: Are you querying by correct address?
- **Solution**: Use `/address/{address}/deposits` not `/user/{user_id}/transactions`

---

## Support

For questions or issues during migration:
1. Review this guide thoroughly
2. Check API documentation at `http://your-api/swagger-ui/`
3. Review backend changes in `BALANCE_CONSOLIDATION_GUIDE.md`
4. Contact backend team for clarifications

---

## Quick Reference Card

| **Operation** | **Old Endpoint** | **New Endpoint (Account-Centric)** |
|---------------|------------------|------------------|
| Lookup account | N/A | `GET /accounts?address={addr}` |
| Get account balance | `/user/{id}/financial/account`<br/>`/address/{addr}/balance` | `GET /account/{account_id}/balance` |
| Get user balances | `/user/{id}/financial/summary` | `GET /user/{id}/balances` |
| Get org balances | N/A | `GET /org/{org_id}/balances` |
| Deposit history | `/user/{id}/financial/transactions` | `GET /account/{account_id}/deposits` |
| Withdrawal history | `/users/{addr}/withdrawals` | `GET /account/{account_id}/withdrawals` |
| Initiate withdrawal | `POST /withdrawals` | `POST /account/{account_id}/withdrawals` |
| Withdrawal status | `GET /withdrawals/{id}` | `GET /withdrawals/{id}` (updated fields) |
| Cancel withdrawal | `POST /withdrawals/{id}/cancel` | `POST /withdrawals/{id}/cancel` |
| List user accounts | N/A (NEW) | `GET /user/{id}/accounts` |
| List org accounts | N/A (NEW) | `GET /org/{org_id}/accounts` |

### Typical Workflow

1. **User Connects Wallet** → `GET /accounts?address={walletAddress}` → Get `account_id`
2. **Display Balance** → `GET /account/{account_id}/balance`
3. **Show Transactions** → `GET /account/{account_id}/deposits` + `GET /account/{account_id}/withdrawals`
4. **Initiate Withdrawal** → `POST /account/{account_id}/withdrawals`
5. **Track Status** → `GET /withdrawals/{withdrawal_id}`

---

**Last Updated:** 2025-01-15  
**Backend Version:** Post-consolidation (migration 003+ with Account-Centric Model)  
**Breaking Changes:** Yes - requires frontend updates to use account_id instead of addresses  
**See Also:** `ACCOUNT_CENTRIC_MODEL.md` for detailed architecture documentation

