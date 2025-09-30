# Faucet Integration Summary

## Your Questions Answered

### Q: "Do we need to implement any additional functionality for users to request funds?"

**Answer:** ✅ **Yes, I've created it!**

I've implemented:
1. ✅ **Backend API endpoints** (`api/src/handlers/faucet.rs`)
   - `GET /api/v1/faucet/info` - Environment detection & contract addresses
   - `GET /api/v1/faucet/status?address=0x...` - Check cooldown status
   - `GET /api/v1/faucet/config` - Get faucet configuration

2. ✅ **Frontend implementation guide** (`docs/integrations/frontend-faucet-integration.md`)
   - Complete React/TypeScript code examples
   - MetaMask integration patterns
   - Auto-onboarding flow
   - Error handling strategies

### Q: "Where is the eth private key in the makefile coming from? Is this a setting in anvil that we can reset if needed?"

**Answer:** It's the **standard Anvil test account private key**.

**Key Facts:**
- 🔑 **Private Key**: `0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80`
- 👤 **Address**: `0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266`
- 💰 **Balance**: 10,000 ETH on every Anvil instance
- 🌍 **Source**: Anvil's default test mnemonic: `"test test test test test test test test test test test junk"`

**Is it safe?**
- ✅ **YES** for local development (only controls test funds)
- ❌ **NEVER** use on real networks (publicly known)
- ✅ **Standard practice** in Foundry ecosystem
- ✅ **Team consistency** - everyone has same test accounts

**Can you change it?**
- Yes! Modify `docker-compose.yml`:
  ```yaml
  anvil:
    command:
      - anvil
      - --mnemonic
      - "your custom twelve word mnemonic here"
  ```
- **Recommendation**: Keep the default for team consistency

**Where it's used:**
- `Makefile` line 418-419: For automated contract deployment
- `Makefile` line 317: For withdrawal executor default key
- **Only in local/staging** - sandbox/production use your real `.env` key

### Q: "Is that something that is a backend thing or a frontend thing?"

**Answer:** It's **mostly a FRONTEND thing** with backend support.

**Backend's Role (✅ Done):**
1. Provide faucet information via API
2. Tell frontend if faucet is available
3. Return contract addresses
4. Return network configuration

**Frontend's Role (📋 To Implement):**
1. Call backend API to detect environment
2. Show "Request Test USDC" button (local/staging only)
3. Prompt user to connect MetaMask
4. Add/switch to Anvil network in MetaMask
5. Call faucet contract via MetaMask
6. Handle transaction signing (MetaMask popup)
7. Show success/error messages
8. Refresh USDC balance

**Why frontend?**
- Faucet is a smart contract (requires user signature)
- Users must sign transaction via MetaMask
- Backend can't sign on user's behalf
- Direct wallet-to-contract interaction

## What You Need to Do

### Option 1: Give Guide to Frontend Team

**Action:** Share this file with frontend developers:
```
docs/integrations/frontend-faucet-integration.md
```

**What they'll get:**
- ✅ Complete React/TypeScript implementation
- ✅ All necessary hooks (`useFaucetInfo`, `useAnvilNetwork`, `useFaucetRequest`)
- ✅ Ready-to-use components (`FaucetButton`, `USDCBalance`)
- ✅ Full onboarding flow example
- ✅ MetaMask integration best practices
- ✅ Error handling strategies
- ✅ Testing checklist

**Estimated implementation time:** 4-6 hours for frontend developer

### Option 2: Test Backend API Now

You can test the new endpoints immediately:

```bash
# Start server
make server

# In another terminal, test faucet info endpoint
curl http://localhost:8080/api/v1/faucet/info | jq '.'

# Expected response:
# {
#   "available": true,
#   "environment": "local",
#   "faucet_address": "0xDc64a140Aa3E981100a9becA4E685f962f0cF6C9",
#   "usdc_address": "0x5FbDB2315678afecb367f032d93F642f64180aa3",
#   "amount_per_request": "1000000",
#   "cooldown_seconds": 3600,
#   ...
# }
```

## Implementation Flow

### User Onboarding Journey (Automated)

```
1. User visits app
   ↓
2. Frontend calls GET /api/v1/faucet/info
   ↓
3. If faucet.available === true:
   → Show "You're in test environment" banner
   → Show "Request Test USDC" button
   ↓
4. User clicks "Connect Wallet"
   → MetaMask popup appears
   → User approves connection
   ↓
5. Frontend detects wrong network (if connected to Sepolia)
   → Prompts: "Switch to Local Anvil Devnet?"
   → User clicks "Add Network" in MetaMask
   → Auto-switches to Anvil (localhost:8545)
   ↓
6. User clicks "Request Test USDC"
   → Frontend calls faucet.requestFunds() via ethers.js
   → MetaMask popup: "Confirm transaction"
   → User signs transaction
   ↓
7. Transaction confirmed on Anvil
   → User receives 1M USDC instantly (2-second blocks)
   → Balance updates automatically
   → Success message: "✅ Received 1M test USDC!"
   ↓
8. User can now:
   → Make deposits to ARDA contract
   → Create deals
   → Test full workflow
```

## Quick Reference

### Backend API Endpoints (Already Built)

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/api/v1/faucet/info` | GET | Get faucet availability & contract addresses |
| `/api/v1/faucet/status?address=0x...` | GET | Check if address can request funds |
| `/api/v1/faucet/config` | GET | Get faucet configuration parameters |

### Smart Contract Functions (For Frontend)

| Function | Purpose |
|----------|---------|
| `requestFunds()` | Request 1M USDC (user pays gas) |
| `canRequestNow(address)` | Check if address can request |
| `getTimeUntilNextRequest(address)` | Get cooldown remaining |
| `getFaucetBalance()` | Get total faucet balance |

### Frontend Implementation Files

| File | Purpose |
|------|---------|
| `hooks/useFaucetInfo.ts` | Fetch faucet info from backend |
| `hooks/useAnvilNetwork.ts` | Add/switch MetaMask network |
| `hooks/useFaucetRequest.ts` | Call faucet contract |
| `components/FaucetButton.tsx` | Reusable faucet UI component |
| `components/USDCBalance.tsx` | Balance display with faucet |

All complete code provided in: `docs/integrations/frontend-faucet-integration.md`

## Testing the Backend

```bash
# 1. Start services
make server

# 2. Test faucet info endpoint
curl http://localhost:8080/api/v1/faucet/info | jq '.'

# 3. Test faucet status endpoint
curl "http://localhost:8080/api/v1/faucet/status?address=0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266" | jq '.'

# 4. Test faucet config endpoint
curl http://localhost:8080/api/v1/faucet/config | jq '.'
```

## Key Architectural Decisions

### Why Frontend Calls Contract Directly?

**Pros:**
- ✅ User signs with their own wallet (non-custodial)
- ✅ No backend secrets needed
- ✅ Transaction shows in MetaMask history
- ✅ User has full control
- ✅ Matches production deposit/withdrawal flow

**Cons:**
- ❌ Requires MetaMask integration
- ❌ User must approve network switch
- ❌ User must sign transaction

**Alternative Considered:**
Backend could call faucet on user's behalf, but this would require:
- Backend to hold private keys for faucet admin
- User to trust backend with fund distribution
- More complex permission management
- Not reflective of production workflow

**Decision:** Frontend calls contract directly (chosen approach)

### Why Three API Endpoints?

1. **`/faucet/info`** - Primary endpoint
   - Called once on app load
   - Determines if faucet UI should appear
   - Provides all contract addresses
   - Includes MetaMask network config

2. **`/faucet/status`** - User-specific
   - Check cooldown before showing button
   - Validate user can request
   - Show time remaining

3. **`/faucet/config`** - Configuration
   - Simple config for caching
   - UI customization parameters
   - Display text and limits

## Environment Behavior Matrix

| Environment | Faucet API Returns | Frontend Shows | User Action |
|------------|-------------------|----------------|-------------|
| **local** | `available: true` | "Request Test USDC" button | Calls contract via MetaMask |
| **staging** | `available: true` | "Request Test USDC" button | Calls contract via MetaMask |
| **sandbox** | `available: false` | Nothing (no faucet UI) | Uses Sepolia test faucets |
| **production** | `available: false` | Nothing (no faucet UI) | Uses real USDC |

## Next Steps for Frontend Team

### Phase 1: Basic Integration (Required)
1. ✅ Review `docs/integrations/frontend-faucet-integration.md`
2. Create `useFaucetInfo` hook (calls `/api/v1/faucet/info`)
3. Create `FaucetButton` component (calls contract via MetaMask)
4. Add faucet button to dashboard/onboarding
5. Test with local Anvil

### Phase 2: Enhanced UX (Recommended)
6. Add network detection & auto-switch
7. Create `useAnvilNetwork` hook
8. Show cooldown timer
9. Add success animations
10. Optimistic balance updates

### Phase 3: Auto-Onboarding (Nice to Have)
11. Auto-show faucet modal for new users
12. Track in localStorage if user already requested
13. Skip button for returning users
14. Integration with user registration flow

## Summary

✅ **Backend is DONE:**
- API endpoints created and tested
- Routes registered in router
- Paths defined
- Documentation complete

📋 **Frontend TODO:**
- Implement using guide: `docs/integrations/frontend-faucet-integration.md`
- Call backend API to detect environment
- Call faucet contract via MetaMask when user requests
- Handle network switching
- Show appropriate UI based on environment

🔑 **Private Key:**
- It's the standard Anvil test key (safe for local development)
- Publicly known, only controls test funds
- Can be changed via docker-compose if needed
- Never used on real networks (sandbox/production use your `.env` key)

**Estimated frontend implementation time:** 4-6 hours

