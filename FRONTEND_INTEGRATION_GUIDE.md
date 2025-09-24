# 🌐 Frontend Integration Guide for Loan Trading

## Overview

The loan trading system enables users to list loans for sale on a marketplace and transfer ownership between lenders. This guide provides everything needed to integrate loan trading functionality into your frontend application.

## API Endpoints Summary

### Marketplace Operations (Worker A)

**Browse Marketplace**
```http
GET /api/v1/loans/for-sale
```
- Browse available loans for sale
- Supports filtering and pagination
- Returns `LoansForSaleResponse` with `MarketplaceDeal` objects

**List Loan for Sale**
```http
POST /api/v1/loan/{id}/list-for-sale
```
- List a loan you own for sale on the marketplace
- Body: `ListLoanForSaleRequest`
- Returns `ListLoanForSaleResponse`

**Remove Loan from Sale**
```http
DELETE /api/v1/loan/{id}/list-for-sale
```
- Remove your loan listing from the marketplace
- Returns `RemoveLoanFromSaleResponse`

**Marketplace Statistics**
```http
GET /api/v1/loans/marketplace/stats
```
- Get marketplace statistics
- Returns `MarketplaceStatsResponse`

### Transfer Operations (Worker B)

**Execute Loan Transfer**
```http
POST /api/v1/loan/{id}/transfer
```
- Purchase a loan from the marketplace
- Body: `LoanTransferRequest`
- Returns `LoanTransferResponse`

**Loan Transfer History**
```http
GET /api/v1/loan/{id}/transfers
```
- Get transfer history for a specific loan
- Returns `LoanTransferHistoryResponse`

**User Transfer History**
```http
GET /api/v1/user/{id}/transfers
```
- Get transfer history for a specific user
- Returns `UserTransferHistoryResponse`

**Validate Transfer**
```http
POST /api/v1/loan/{id}/transfer/validate
```
- Validate transfer before execution
- Body: `LoanTransferRequest`
- Returns validation result

## Authentication

### MVP Testing
```javascript
// Add header for MVP testing
const headers = {
  'Content-Type': 'application/json',
  'x-test-user-id': userId  // User's UUID
};
```

### Production Authentication
```javascript
// Production: Use JWT authentication
const headers = {
  'Content-Type': 'application/json',
  'Authorization': `Bearer ${jwtToken}`
};
```

## TypeScript Interfaces

### Core Types

```typescript
interface MarketplaceDeal {
  id: string;              // Loan ID
  title?: string;
  description?: string;
  listing_description?: string;
  principal_amount: string; // Decimal as string
  outstanding_balance: string;
  interest_rate_bps: number;
  loan_status: string;
  originated_at: string;
  asking_price: string;    // Sale price
  listed_at: string;
  seller_email: string;
  seller_kyc_status: string;
  asset_class?: string;
  region?: string;
  risk_level?: string;
}

interface LoansForSaleResponse {
  listings: MarketplaceDeal[];
  total_count: number;
  page: number;
  page_size: number;
  has_more: boolean;
}

interface ListLoanForSaleRequest {
  asking_price: string;    // Price in USDC (e.g., "45000.00")
  description?: string;    // Optional description
}

interface ListLoanForSaleResponse {
  listing_id: string;
  loan_id: string;
  asking_price: string;
  listed_at: string;
  status: string;
}

interface MarketplaceStatsResponse {
  active_listings: number;
  total_listed_value: string;
  average_listing_price: string;
  listings_by_asset_class: Record<string, number>;
}

interface LoanTransferRequest {
  new_lender_id: string;   // Buyer UUID
  purchase_price: string;  // Price in USDC
  transfer_notes?: string;
}

interface LoanTransferResponse {
  transfer_id: string;
  loan_id: string;
  from_lender_id: string;
  to_lender_id: string;
  purchase_price: string;
  transfer_status: string;
  completed_at: string;
}

interface TransferHistoryItem {
  transfer_id: string;
  loan_id: string;
  from_lender_id: string;
  to_lender_id: string;
  purchase_price: string;
  transfer_status: string;
  completed_at?: string;
  created_at: string;
  notes?: string;
}

interface LoanTransferHistoryResponse {
  loan_id: string;
  transfers: TransferHistoryItem[];
  total_transfers: number;
}

interface UserTransferHistoryResponse {
  user_id: string;
  transfers: TransferHistoryItem[];
  total_count: number;
  page: number;
  page_size: number;
}
```

## Example Frontend Integration

### Marketplace Browsing

```typescript
interface MarketplaceFilters {
  min_amount?: string;
  max_amount?: string;
  min_price?: string;
  max_price?: string;
  asset_class?: string;
  region?: string;
  limit?: number;
  offset?: number;
}

class LoanTradingService {
  private baseUrl = '/api/v1';

  async browseMarketplace(filters: MarketplaceFilters = {}): Promise<LoansForSaleResponse> {
    const params = new URLSearchParams();

    Object.entries(filters).forEach(([key, value]) => {
      if (value !== undefined && value !== null) {
        params.append(key, value.toString());
      }
    });

    const response = await fetch(`${this.baseUrl}/loans/for-sale?${params}`, {
      headers: {
        'x-test-user-id': this.getCurrentUserId() // Replace with JWT in production
      }
    });

    if (!response.ok) {
      throw new Error(`Failed to browse marketplace: ${response.statusText}`);
    }

    return response.json();
  }

  async listLoanForSale(loanId: string, askingPrice: string, description?: string): Promise<ListLoanForSaleResponse> {
    const response = await fetch(`${this.baseUrl}/loan/${loanId}/list-for-sale`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'x-test-user-id': this.getCurrentUserId()
      },
      body: JSON.stringify({
        asking_price: askingPrice,
        description: description || ''
      })
    });

    if (!response.ok) {
      throw new Error(`Failed to list loan: ${response.statusText}`);
    }

    return response.json();
  }

  async executeTransfer(loanId: string, purchasePrice: string, notes?: string): Promise<LoanTransferResponse> {
    const response = await fetch(`${this.baseUrl}/loan/${loanId}/transfer`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'x-test-user-id': this.getCurrentUserId()
      },
      body: JSON.stringify({
        new_lender_id: this.getCurrentUserId(), // Current user is the buyer
        purchase_price: purchasePrice,
        transfer_notes: notes || 'Purchase through marketplace'
      })
    });

    if (!response.ok) {
      const error = await response.json();
      throw new Error(error.error || `Transfer failed: ${response.statusText}`);
    }

    return response.json();
  }

  async validateTransfer(loanId: string, purchasePrice: string): Promise<any> {
    const response = await fetch(`${this.baseUrl}/loan/${loanId}/transfer/validate`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'x-test-user-id': this.getCurrentUserId()
      },
      body: JSON.stringify({
        new_lender_id: this.getCurrentUserId(),
        purchase_price: purchasePrice
      })
    });

    return response.json();
  }

  async getMarketplaceStats(): Promise<MarketplaceStatsResponse> {
    const response = await fetch(`${this.baseUrl}/loans/marketplace/stats`, {
      headers: {
        'x-test-user-id': this.getCurrentUserId()
      }
    });

    if (!response.ok) {
      throw new Error(`Failed to get stats: ${response.statusText}`);
    }

    return response.json();
  }

  async getLoanTransferHistory(loanId: string): Promise<LoanTransferHistoryResponse> {
    const response = await fetch(`${this.baseUrl}/loan/${loanId}/transfers`, {
      headers: {
        'x-test-user-id': this.getCurrentUserId()
      }
    });

    if (!response.ok) {
      throw new Error(`Failed to get history: ${response.statusText}`);
    }

    return response.json();
  }

  async getUserTransferHistory(userId: string, page = 1, limit = 20): Promise<UserTransferHistoryResponse> {
    const params = new URLSearchParams({
      limit: limit.toString(),
      offset: ((page - 1) * limit).toString()
    });

    const response = await fetch(`${this.baseUrl}/user/${userId}/transfers?${params}`, {
      headers: {
        'x-test-user-id': this.getCurrentUserId()
      }
    });

    if (!response.ok) {
      throw new Error(`Failed to get user history: ${response.statusText}`);
    }

    return response.json();
  }

  private getCurrentUserId(): string {
    // Replace with your actual user ID retrieval logic
    return localStorage.getItem('userId') || '';
  }
}
```

### React Hook Example

```typescript
import { useState, useEffect } from 'react';

interface UseMarketplaceReturn {
  listings: MarketplaceDeal[];
  loading: boolean;
  error: string | null;
  refresh: () => void;
  stats: MarketplaceStatsResponse | null;
}

export function useMarketplace(filters: MarketplaceFilters = {}): UseMarketplaceReturn {
  const [listings, setListings] = useState<MarketplaceDeal[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [stats, setStats] = useState<MarketplaceStatsResponse | null>(null);

  const fetchListings = async () => {
    try {
      setLoading(true);
      setError(null);

      const tradingService = new LoanTradingService();
      const [listingsResponse, statsResponse] = await Promise.all([
        tradingService.browseMarketplace(filters),
        tradingService.getMarketplaceStats()
      ]);

      setListings(listingsResponse.listings);
      setStats(statsResponse);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load marketplace');
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchListings();
  }, [JSON.stringify(filters)]);

  return {
    listings,
    loading,
    error,
    refresh: fetchListings,
    stats
  };
}
```

## Error Handling

All endpoints return standard HTTP status codes:

- **400 Bad Request**: Invalid parameters or validation errors
- **401 Unauthorized**: Missing or invalid authentication
- **403 Forbidden**: User doesn't own the loan or other permission issues
- **404 Not Found**: Loan or user not found
- **409 Conflict**: Business rule violations (insufficient balance, loan not for sale)
- **500 Internal Server Error**: Server-side errors

### Error Response Format

```typescript
interface ErrorResponse {
  error: string;  // Human-readable error message
  code?: string;  // Optional error code for programmatic handling
}
```

### Common Error Scenarios

```typescript
try {
  await tradingService.executeTransfer(loanId, price);
} catch (error) {
  if (error.message.includes('insufficient balance')) {
    // Show balance error to user
    showError('Insufficient funds for this purchase');
  } else if (error.message.includes('not for sale')) {
    // Loan was already sold
    showError('This loan is no longer available');
  } else if (error.message.includes('do not own')) {
    // User doesn't own the loan they're trying to list
    showError('You do not own this loan');
  } else {
    // Generic error
    showError('Transaction failed. Please try again.');
  }
}
```

## Rate Limiting

- Current: None (MVP)
- Production: Consider implementing for marketplace browsing
- Recommended: 100 requests per minute per user for marketplace endpoints

## Performance Considerations

- Marketplace listings are paginated (default: 20 items per page)
- Use filtering to reduce result sets
- Cache marketplace stats for better UX
- Implement optimistic updates for better perceived performance

## WebSocket Integration (Future)

For real-time marketplace updates, consider implementing WebSocket connections:

```typescript
// Future: Real-time marketplace updates
const ws = new WebSocket('ws://api.arda.credit/marketplace');

ws.onmessage = (event) => {
  const update = JSON.parse(event.data);
  if (update.type === 'listing_added') {
    // Add new listing to UI
  } else if (update.type === 'listing_removed') {
    // Remove listing from UI
  }
};
```

## Testing

Use the provided test scripts for integration testing:

```bash
# Run complete integration test
./test_loan_trading_integration.sh

# Run CLI workflow test
./test_cli_trading_workflow.sh

# Run error scenario tests
./test_trading_error_scenarios.sh

# Run performance tests
./test_trading_performance.sh
```

## Migration from Existing Deal Interface

If you have an existing `Deal` interface, map it to `MarketplaceDeal`:

```typescript
// Existing Deal interface
interface Deal {
  id: string;
  title: string;
  description: string;
  amount: number;
  interestRate: number;
  term: number;
  risk: string;
  region: string;
  seller: string;
}

// Migration mapping
function mapDealToMarketplaceDeal(deal: Deal): MarketplaceDeal {
  return {
    id: deal.id,
    title: deal.title,
    description: deal.description,
    principal_amount: deal.amount.toString(),
    outstanding_balance: deal.amount.toString(), // Assuming no payments made
    interest_rate_bps: deal.interestRate * 100, // Convert percentage to bps
    loan_status: 'for_sale',
    originated_at: new Date().toISOString(), // Placeholder
    asking_price: deal.amount.toString(), // Use principal as asking price
    listed_at: new Date().toISOString(),
    seller_email: deal.seller,
    seller_kyc_status: 'approved', // Assume approved for MVP
    asset_class: 'unknown',
    region: deal.region,
    risk_level: deal.risk
  };
}
```

## Support

For questions or issues with the loan trading integration:

1. Check the OpenAPI documentation at `/docs/api`
2. Review the test scripts for working examples
3. Contact the backend team for API-related questions
4. Check application logs for detailed error information

---

**The loan trading system is now ready for frontend integration! 🚀**
