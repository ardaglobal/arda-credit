# zkVM Integration: Implementation Plan & Work Streams

**Project:** zkVM Full Integration for MVP  
**Timeline:** 17-18 days (3.5-4 weeks)  
**Team Size:** 3-4 engineers recommended  
**Last Updated:** December 12, 2024  

---

## Executive Summary

Based on the ZK Circuit Audit and Architecture documentation, we need to implement the complete integration between our API layer and zkVM proof system. The work is broken into 6 parallel work streams that can be staffed independently.

### Critical Path

```
Week 1: Foundation (WS-1) → BLOCKS EVERYTHING
        ↓
Week 2: Core Deal Ops (WS-2) || Collateral zkVM (WS-3)  ← PARALLEL
        ↓                               ↓
Week 3: API Integration (WS-2)   || Collateral API (WS-4)  ← PARALLEL
        ↓                               ↓
Week 4: Testing (WS-5) + Test Scripts (WS-6)  ← PARALLEL
```

### Success Metrics

- ✅ 10/10 transaction types integrated with zkVM
- ✅ All API endpoints create deal_transactions records
- ✅ Collateral operations ZK-proven (prevents double-pledging cryptographically)
- ✅ End-to-end tests passing (API → zkVM → Contract)
- ✅ Test script creates 100% transaction coverage
- ✅ < 5 minute latency from user action to on-chain proof

---

## Work Stream Breakdown

### WS-1: Transaction Recording Infrastructure
**Owner:** Backend/Infrastructure Engineer  
**Timeline:** Week 1 (1 day)  
**Dependencies:** None - MUST START FIRST  
**Blocks:** All other work streams  

### WS-2: Core Deal Operations Integration
**Owner:** Backend Engineer #1  
**Timeline:** Weeks 1-3 (3.5 days)  
**Dependencies:** WS-1 must complete first  
**Blocks:** WS-5 (testing)  

### WS-3: Collateral zkVM Implementation
**Owner:** zkVM/Cryptography Engineer  
**Timeline:** Weeks 2-3 (5.5 days)  
**Dependencies:** WS-1 complete, can run parallel with WS-2  
**Blocks:** WS-4 (collateral API)  

### WS-4: Collateral API Integration
**Owner:** Backend Engineer #2  
**Timeline:** Week 3 (4.5 days)  
**Dependencies:** WS-3 must complete first  
**Blocks:** WS-5 (collateral testing)  

### WS-5: Testing & Validation
**Owner:** QA/Test Engineer  
**Timeline:** Weeks 2-4 (4.5 days, incremental)  
**Dependencies:** Can start as WS-2/WS-3/WS-4 complete  
**Blocks:** Production release  

### WS-6: Test Data & Scripts
**Owner:** DevOps/Backend Engineer #3  
**Timeline:** Weeks 1-4 (1 day total, spread across weeks)  
**Dependencies:** Can start immediately, update as other WS complete  
**Blocks:** QA validation  

---

## Epic Issues

## EPIC-1: Transaction Recording Infrastructure

**Work Stream:** WS-1  
**Priority:** CRITICAL - Blocks all other work  
**Effort:** 1 day  
**Owner:** Backend/Infrastructure Engineer  

### Description

Create the foundational `TransactionRecordingService` that bridges the gap between API endpoints and zkVM batch processing. This service will be used by all API endpoints to create `deal_transactions` records that feed into the ZK proof system.

### Acceptance Criteria

- [ ] `TransactionRecordingService` created in `api/src/services/transaction_recording.rs`
- [ ] Service can record any transaction type with proper UUID hashing (SHA256)
- [ ] Inserts into `deal_transactions` table with correct schema
- [ ] Unit tests cover all transaction types
- [ ] Error handling for duplicate transactions
- [ ] Logging and metrics instrumentation
- [ ] Documentation with usage examples

### Technical Scope

**New File:** `api/src/services/transaction_recording.rs`

**Key Functions:**
```rust
pub struct TransactionRecordingService {
    pool: PgPool,
}

impl TransactionRecordingService {
    pub async fn record_transaction(
        &self,
        transaction_type: TransactionType,
        user_id: Uuid,
        deal_id: Option<Uuid>,
        amount: Decimal,
        metadata: Option<serde_json::Value>,
    ) -> Result<Uuid, DbError>;
    
    fn hash_uuid(uuid: &Uuid) -> Vec<u8>;
}
```

**Database Table:** `deal_transactions` (already exists)

### Sub-Issues

#### EPIC-1.1: Create TransactionRecordingService Struct
- Create service file with basic structure
- Add PgPool dependency
- Define public API surface
- **Effort:** 2 hours

#### EPIC-1.2: Implement UUID Hashing
- Implement SHA256 hashing for UUIDs
- Ensure consistency with existing test script
- Add helper functions for byte conversion
- **Effort:** 1 hour

#### EPIC-1.3: Implement record_transaction Method
- Build SQL query for deal_transactions insert
- Handle all transaction type variants
- Add proper error handling
- **Effort:** 3 hours

#### EPIC-1.4: Add Unit Tests
- Test all transaction types
- Test UUID hashing consistency
- Test error cases (duplicates, invalid data)
- Mock database for testing
- **Effort:** 2 hours

#### EPIC-1.5: Add Logging & Metrics
- Instrument with tracing
- Add Prometheus metrics counters
- Log transaction IDs for debugging
- **Effort:** 1 hour (can be parallel with 1.3)

---

## EPIC-2: Core Deal Operations Integration

**Work Stream:** WS-2  
**Priority:** HIGH  
**Effort:** 3.5 days (spread across weeks 1-3)  
**Owner:** Backend Engineer #1  
**Dependencies:** EPIC-1 complete  

### Description

Integrate the four core deal/user operations with the zkVM by adding `TransactionRecordingService` calls to existing/new API endpoints. This enables user registration, deal creation, payments, and closeouts to flow through the ZK proof system.

### Acceptance Criteria

- [ ] UserRegistration integrated with existing user creation endpoint
- [ ] DealOrigination integrated with existing deal creation endpoint
- [ ] New payment endpoint created and integrated
- [ ] New closeout endpoint created and integrated
- [ ] All endpoints create deal_transactions records
- [ ] Integration tests pass for all operations
- [ ] API documentation updated (Swagger)

### Technical Scope

**Files to Modify:**
- `api/src/handlers/users/users.rs` - Add recording to user creation
- `api/src/handlers/deals/deals.rs` - Add recording to deal creation, add new endpoints

**New Endpoints:**
- `POST /api/v1/deal/{id}/payment` - Make payment on deal
- `POST /api/v1/deal/{id}/closeout` - Close completed deal

### Sub-Issues

#### EPIC-2.1: Integrate UserRegistration
- Modify existing user creation endpoint
- Add TransactionRecordingService call
- Handle errors and edge cases
- Update API documentation
- **Effort:** 0.5 days (4 hours)
- **Dependencies:** EPIC-1 complete

#### EPIC-2.2: Integrate DealOrigination  
- Modify existing deal creation endpoint
- Add TransactionRecordingService call
- Format metadata properly (originator + investor IDs in first 64 bytes)
- Handle errors
- **Effort:** 0.5 days (4 hours)
- **Dependencies:** EPIC-1 complete

#### EPIC-2.3: Create Payment Endpoint
- Create new `make_payment_endpoint` function
- Validate deal exists and is active
- Check user permissions
- Create payment record in database
- Create deal_transactions record via service
- Add OpenAPI documentation
- **Effort:** 1 day (8 hours)
- **Dependencies:** EPIC-1 complete

#### EPIC-2.4: Create Closeout Endpoint
- Create new `closeout_deal_endpoint` function
- Validate deal is paid_off
- Check user is investor
- Update deal status to closed
- Create deal_transactions record via service
- Add OpenAPI documentation
- **Effort:** 0.5 days (4 hours)
- **Dependencies:** EPIC-1 complete

#### EPIC-2.5: Integration Tests for Core Operations
- Test UserRegistration flow (API → deal_transactions → batch)
- Test DealOrigination flow
- Test Payment flow
- Test Closeout flow
- Verify transaction records created correctly
- **Effort:** 1 day (8 hours)
- **Dependencies:** EPIC-2.1-2.4 complete
- **Can be parallel with other work**

---

## EPIC-3: Collateral zkVM Implementation

**Work Stream:** WS-3  
**Priority:** HIGH  
**Effort:** 5.5 days  
**Owner:** zkVM/Cryptography Engineer  
**Dependencies:** EPIC-1 complete, can run parallel with EPIC-2  

### Description

Implement collateral state management and processing logic within the zkVM to enable cryptographic proof of collateral operations, including the critical double-pledge prevention. This involves creating a new `lib/src/collateral.rs` module and integrating it with the existing zkVM state.

### Acceptance Criteria

- [ ] `lib/src/collateral.rs` module created
- [ ] Collateral and associations added to GlobalLoanState
- [ ] All 6 collateral transaction types processed in zkVM (no more NO-OPs)
- [ ] Double-pledge prevention validated in ZK
- [ ] Collateral state included in Merkle tree computation
- [ ] Unit tests for all collateral operations
- [ ] zkVM tests pass with collateral transactions

### Technical Scope

**New File:** `lib/src/collateral.rs`

**Modifications:**
- `lib/src/lib.rs` - Add collateral module export
- `lib/src/deal_processor.rs` - Replace NO-OPs with actual processing (lines 272-280)

**Key Structures:**
```rust
pub struct Collateral {
    pub collateral_id: [u8; 32],
    pub collateral_type: String,
    pub created_at: u64,
    pub updated_at: u64,
}

pub struct CollateralState {
    pub collateral: HashMap<[u8; 32], Collateral>,
    pub associations: HashMap<[u8; 32], Vec<[u8; 32]>>,  // deal_id -> collateral_ids
}
```

### Sub-Issues

#### EPIC-3.1: Create Collateral Module Structure
- Create `lib/src/collateral.rs`
- Define Collateral struct (zkVM-compatible)
- Define CollateralState struct
- Define CollateralError enum
- Add module exports to lib.rs
- **Effort:** 0.5 days (4 hours)
- **Dependencies:** None - can start immediately after EPIC-1

#### EPIC-3.2: Update GlobalLoanState
- Add collateral HashMap to GlobalLoanState
- Add collateral_associations HashMap
- Update Default implementation
- Update serialization/deserialization
- **Effort:** 0.5 days (4 hours)
- **Dependencies:** EPIC-3.1 complete

#### EPIC-3.3: Implement CollateralCreation Processor
- Implement `process_collateral_creation()` in CollateralState
- Validate collateral_id uniqueness
- Extract metadata (name, type, etc.)
- Insert into collateral HashMap
- Add unit tests
- **Effort:** 0.5 days (4 hours)
- **Dependencies:** EPIC-3.2 complete

#### EPIC-3.4: Implement CollateralUpdate Processor
- Implement `process_collateral_update()` in CollateralState
- Validate collateral exists
- Update properties from metadata
- Add unit tests
- **Effort:** 0.5 days (4 hours)
- **Dependencies:** EPIC-3.2 complete
- **Can be parallel with EPIC-3.3**

#### EPIC-3.5: Implement Association/Disassociation Processors
- Implement `process_collateral_association()`
- **CRITICAL:** Add double-pledge detection (check if collateral already associated)
- Implement `process_collateral_disassociation()`
- Validate deal exists before association
- Add unit tests for both
- Test double-pledge prevention
- **Effort:** 1 day (8 hours)
- **Dependencies:** EPIC-3.2 complete
- **Can be parallel with EPIC-3.3/3.4**

#### EPIC-3.6: Implement Detection Processors
- Implement `process_collateral_detection_request()`
- Implement `process_collateral_detection_update()`
- Handle metadata for detection results
- Add unit tests
- **Effort:** 1 day (8 hours)
- **Dependencies:** EPIC-3.2 complete
- **Can be parallel with EPIC-3.3/3.4/3.5**

#### EPIC-3.7: Integrate with deal_processor.rs
- Replace NO-OP code (lines 272-280) with actual processing
- Wire up all 6 collateral processors
- Add error handling
- Update process_transaction match arms
- **Effort:** 0.5 days (4 hours)
- **Dependencies:** EPIC-3.3-3.6 all complete

#### EPIC-3.8: zkVM Testing for Collateral
- Test collateral creation in zkVM
- Test double-pledge prevention (critical!)
- Test association/disassociation
- Test detection flows
- Integration tests with deals
- **Effort:** 1 day (8 hours)
- **Dependencies:** EPIC-3.7 complete

---

## EPIC-4: Collateral API Integration

**Work Stream:** WS-4  
**Priority:** HIGH  
**Effort:** 4.5 days  
**Owner:** Backend Engineer #2  
**Dependencies:** EPIC-1 complete, EPIC-3 complete  

### Description

Integrate collateral API endpoints with the TransactionRecordingService to create deal_transactions records for all collateral operations. This includes verifying/updating existing endpoints and creating webhook handlers for collateral-intel service events.

### Acceptance Criteria

- [ ] All collateral CRUD endpoints create deal_transactions records
- [ ] Collateral-deal association endpoints integrated
- [ ] Webhook handlers for collateral-intel detection events
- [ ] All 6 collateral transaction types can be triggered via API
- [ ] Integration tests pass
- [ ] API documentation updated

### Technical Scope

**Files to Verify/Modify:**
- `api/src/handlers/taxonomy/collateral.rs` - Verify endpoints exist, add recording
- `api/src/events/` - Webhook handlers for detection events

**Endpoints to Integrate:**
- POST /api/v1/collateral - Create collateral
- PUT /api/v1/collateral/{id} - Update collateral
- POST /api/v1/collateral/{id}/associate - Associate with deal
- DELETE /api/v1/collateral/{id}/disassociate - Remove from deal
- Webhook for detection requests (from collateral-intel)
- Webhook for detection updates (from collateral-intel)

### Sub-Issues

#### EPIC-4.1: Audit Existing Collateral Endpoints
- Review `api/src/handlers/taxonomy/collateral.rs`
- Document which endpoints exist
- Identify which need to be created
- Create issue list for missing endpoints
- **Effort:** 0.5 days (4 hours)
- **Dependencies:** None - can start while EPIC-3 is in progress

#### EPIC-4.2: Integrate CollateralCreation Endpoint
- Add TransactionRecordingService call to creation endpoint
- Format metadata for zkVM
- Handle errors
- Test integration
- **Effort:** 0.5 days (4 hours)
- **Dependencies:** EPIC-1, EPIC-3.3 complete

#### EPIC-4.3: Integrate CollateralUpdate Endpoint
- Add TransactionRecordingService call to update endpoint
- Format metadata for zkVM
- Handle errors
- Test integration
- **Effort:** 0.5 days (4 hours)
- **Dependencies:** EPIC-1, EPIC-3.4 complete
- **Can be parallel with EPIC-4.2**

#### EPIC-4.4: Integrate Association/Disassociation Endpoints
- Add recording to association endpoint
- Add recording to disassociation endpoint
- Validate deal exists
- Handle edge cases
- Test integration
- **Effort:** 1 day (8 hours)
- **Dependencies:** EPIC-1, EPIC-3.5 complete
- **Can be parallel with EPIC-4.2/4.3**

#### EPIC-4.5: Create Detection Event Webhooks
- Create webhook handler for detection requests
- Create webhook handler for detection updates
- Parse collateral-intel service payloads
- Create deal_transactions records
- Add authentication/security
- Test with mock events
- **Effort:** 1.5 days (12 hours)
- **Dependencies:** EPIC-1, EPIC-3.6 complete
- **Can be parallel with EPIC-4.2/4.3/4.4**

#### EPIC-4.6: Collateral API Integration Tests
- Test all collateral endpoints create transactions
- Test webhook handlers
- Verify transaction records format
- End-to-end collateral flow testing
- **Effort:** 1 day (8 hours)
- **Dependencies:** EPIC-4.2-4.5 all complete

---

## EPIC-5: Testing & Validation

**Work Stream:** WS-5  
**Priority:** HIGH  
**Effort:** 4.5 days (incremental across weeks)  
**Owner:** QA/Test Engineer  
**Dependencies:** Incremental - starts as work streams complete  

### Description

Comprehensive testing of the zkVM integration including unit tests, integration tests, and end-to-end tests. Validates that transactions flow correctly from API through zkVM to smart contract, and that all business logic and cryptographic validations work as expected.

### Acceptance Criteria

- [ ] Unit tests for all new services (>80% coverage)
- [ ] Integration tests for all 10 transaction types
- [ ] End-to-end tests from API to contract
- [ ] Performance tests (batching, proof generation)
- [ ] Error case tests (zkVM rejections, failures)
- [ ] Load tests (concurrent transactions)
- [ ] Documentation of test strategy

### Technical Scope

**Test Files:**
- `api/tests/transaction_recording_tests.rs` - Service tests
- `api/tests/deal_zkvm_integration_tests.rs` - Deal operation tests
- `api/tests/collateral_zkvm_integration_tests.rs` - Collateral tests
- `api/tests/e2e_zkvm_flow_tests.rs` - End-to-end tests

### Sub-Issues

#### EPIC-5.1: Unit Tests for TransactionRecordingService
- Test record_transaction() for all types
- Test UUID hashing
- Test error handling
- Mock database interactions
- **Effort:** 0.5 days (4 hours)
- **Dependencies:** EPIC-1 complete

#### EPIC-5.2: Integration Tests for Deal Operations
- Test UserRegistration API → zkVM flow
- Test DealOrigination API → zkVM flow
- Test DealPayment API → zkVM flow
- Test DealCloseout API → zkVM flow
- Verify batch processing picks up transactions
- **Effort:** 1 day (8 hours)
- **Dependencies:** EPIC-2 complete

#### EPIC-5.3: Integration Tests for Collateral Operations
- Test CollateralCreation API → zkVM flow
- Test CollateralUpdate API → zkVM flow
- Test Association/Disassociation API → zkVM flow
- Test Detection webhooks → zkVM flow
- Verify double-pledge prevention
- **Effort:** 1 day (8 hours)
- **Dependencies:** EPIC-3, EPIC-4 complete

#### EPIC-5.4: End-to-End Flow Tests
- Test complete user registration → deal creation → payment → closeout flow
- Test collateral creation → association → detection flow
- Verify proof generation via Sindri
- Verify contract posting
- Verify on-chain state roots match zkVM output
- **Effort:** 1 day (8 hours)
- **Dependencies:** EPIC-2, EPIC-4 complete

#### EPIC-5.5: Error & Edge Case Tests
- Test zkVM rejection scenarios
- Test concurrent transaction conflicts
- Test invalid state transitions
- Test rollback scenarios
- Test batch processing failures
- **Effort:** 0.5 days (4 hours)
- **Dependencies:** EPIC-2, EPIC-4 complete
- **Can be parallel with EPIC-5.4**

#### EPIC-5.6: Performance & Load Tests
- Test batch processing performance
- Test proof generation latency
- Test concurrent user operations
- Validate <5 minute end-to-end latency
- Identify bottlenecks
- **Effort:** 0.5 days (4 hours)
- **Dependencies:** EPIC-5.4 complete
- **Can be parallel with EPIC-5.5**

---

## EPIC-6: Test Data & Scripts

**Work Stream:** WS-6  
**Priority:** MEDIUM  
**Effort:** 1 day (spread across weeks)  
**Owner:** DevOps/Backend Engineer #3  
**Dependencies:** None - can start immediately  

### Description

Update `populate_deal_data.rs` test script to create transaction records for all 10 in-scope transaction types. This enables realistic testing of the zkVM with comprehensive transaction coverage.

### Acceptance Criteria

- [ ] Script creates UserRegistration transactions
- [ ] Script creates CollateralCreation transactions
- [ ] Script creates CollateralDealAssociation transactions
- [ ] Script creates CollateralUpdate transactions (optional)
- [ ] Script creates CollateralDealDisassociation transactions (optional)
- [ ] Script creates Detection event transactions (optional)
- [ ] Script achieves 60%+ coverage (required) or 100% (ideal)
- [ ] Script documentation updated

### Technical Scope

**File to Modify:** `scripts/populate_deal_data.rs`

**Functions to Update:**
- `ensure_user_companies()` - Add user registration transaction
- `generate_user_deals()` - Add user registration transaction
- `create_random_collateral()` - Add collateral creation transaction
- `create_realistic_deal()` - Add collateral association transaction

**New Functions (Optional):**
- `update_random_collateral()` - Update collateral + create transaction
- `disassociate_collateral()` - Remove association + create transaction
- `simulate_detection_event()` - Simulate detection + create transaction

### Sub-Issues

#### EPIC-6.1: Add UserRegistration Transactions
- Update ensure_user_companies() (line ~1700)
- Update generate_user_deals() (line ~1359)
- Test script generates user registration records
- **Effort:** 2-3 hours
- **Dependencies:** None - can start immediately
- **Priority:** HIGH

#### EPIC-6.2: Add CollateralCreation Transactions
- Update create_random_collateral() (line ~1843)
- Test script generates collateral creation records
- **Effort:** 1-2 hours
- **Dependencies:** None
- **Priority:** HIGH

#### EPIC-6.3: Add CollateralDealAssociation Transactions
- Update create_realistic_deal() (line ~1949)
- Test script generates association records
- **Effort:** 1-2 hours
- **Dependencies:** None
- **Priority:** HIGH

#### EPIC-6.4: Add CollateralUpdate Function (Optional)
- Create update_random_collateral() function
- Call from script to update existing collateral
- Create transaction records
- **Effort:** 2-3 hours
- **Dependencies:** EPIC-6.2 complete
- **Priority:** MEDIUM

#### EPIC-6.5: Add Disassociation Function (Optional)
- Create disassociate_collateral() function
- Test removing collateral from deals
- Create transaction records
- **Effort:** 2-3 hours
- **Dependencies:** EPIC-6.3 complete
- **Priority:** MEDIUM

#### EPIC-6.6: Add Detection Event Simulation (Optional)
- Create simulate_detection_request() function
- Create simulate_detection_update() function
- Call from script
- **Effort:** 4-6 hours
- **Dependencies:** EPIC-6.2 complete
- **Priority:** LOW

---

## Parallelization Strategy

### Week 1: Foundation (All hands on deck)

**Critical Path:**
```
EPIC-1 (1 day) → BLOCKS EVERYTHING ELSE
```

**Team Assignment:**
- Engineer #1: EPIC-1 (Transaction Recording Infrastructure)
- Engineer #2: Code review, documentation prep
- Engineer #3: EPIC-6.1-6.3 (Test script - can start immediately)
- QA: Test plan preparation

### Week 2: Parallel Implementation

**Parallel Work Streams:**
```
EPIC-2.1-2.2 (Deal Integration)  ||  EPIC-3.1-3.6 (Collateral zkVM)
     Engineer #1                  ||       Engineer #2
     1 day                        ||       5 days
```

**Team Assignment:**
- Engineer #1: EPIC-2.1, 2.2 (UserReg + DealOrigin integration)
- Engineer #2: EPIC-3.1-3.6 (Collateral zkVM implementation)
- Engineer #3: EPIC-6.4-6.5 (Optional test script updates)
- QA: EPIC-5.1 (Unit tests for recording service)

### Week 3: API Integration & Payment Endpoints

**Parallel Work Streams:**
```
EPIC-2.3-2.4 (Payment/Closeout)  ||  EPIC-4.1-4.5 (Collateral API)
     Engineer #1                  ||       Engineer #2
     1.5 days                     ||       4.5 days
```

**Team Assignment:**
- Engineer #1: EPIC-2.3, 2.4 (Payment + Closeout endpoints)
- Engineer #2: EPIC-4.1-4.5 (Collateral API integration)
- Engineer #3: EPIC-6.6 (Detection simulation) or help with EPIC-4.5
- QA: EPIC-5.2 (Integration tests for deal operations)

### Week 4: Testing & Polish

**Parallel Work Streams:**
```
EPIC-2.5 (Deal Tests)  ||  EPIC-4.6 (Collateral Tests)  ||  EPIC-5.4-5.6 (E2E Tests)
   Engineer #1         ||       Engineer #2              ||    QA Engineer
   1 day               ||       1 day                    ||    2.5 days
```

**Team Assignment:**
- Engineer #1: EPIC-2.5 (Deal integration tests) + code review
- Engineer #2: EPIC-4.6 (Collateral integration tests) + code review
- Engineer #3: Documentation updates, deployment prep
- QA: EPIC-5.4-5.6 (E2E, error cases, performance tests)

---

## Risk Management

### Critical Risks

**Risk 1: Collateral zkVM Complexity**
- **Impact:** Could take longer than 5.5 days
- **Mitigation:** Start early (Week 2), allocate experienced zkVM engineer
- **Contingency:** Simplify collateral model if needed (e.g., defer detection events)

**Risk 2: API Integration Breaks Existing Functionality**
- **Impact:** Could introduce regressions in deal creation
- **Mitigation:** Comprehensive integration tests, feature flags for rollback
- **Contingency:** Incremental rollout, disable transaction recording if issues arise

**Risk 3: zkVM Proof Generation Failures**
- **Impact:** Batches fail to prove with collateral transactions
- **Mitigation:** Extensive unit testing in lib/, validate with local SP1 prover
- **Contingency:** Debug with SP1 local prover, check metadata encoding

**Risk 4: Test Script Updates Incomplete**
- **Impact:** Can't fully validate zkVM implementation
- **Mitigation:** Implement high-priority updates first (EPIC-6.1-6.3)
- **Contingency:** Manual testing if script incomplete

### Dependencies Map

```
EPIC-1 (Foundation)
  ├─→ EPIC-2 (Deal Ops)
  │     └─→ EPIC-5.2 (Deal Tests)
  ├─→ EPIC-3 (Collateral zkVM)
  │     ├─→ EPIC-4 (Collateral API)
  │     │     └─→ EPIC-5.3 (Collateral Tests)
  │     └─→ EPIC-5.4 (E2E Tests)
  └─→ EPIC-6 (Test Scripts - independent)
        └─→ EPIC-5 (Validation)
```

---

## Staffing Recommendations

### Ideal Team Composition

**Backend/Infrastructure Engineer (Engineer #1)**
- Owns: WS-1, WS-2
- Skills: Rust, Axum, Database design
- Time: 4.5 days (Week 1-3)

**zkVM/Cryptography Engineer (Engineer #2)**
- Owns: WS-3, assists with WS-4
- Skills: Zero-knowledge proofs, SP1, Cryptography
- Time: 10 days (Week 2-4)

**Backend Engineer (Engineer #3)**
- Owns: WS-4, WS-6
- Skills: Rust, API design, Integration
- Time: 5.5 days (Week 1, 3-4)

**QA/Test Engineer**
- Owns: WS-5
- Skills: Integration testing, E2E testing, Performance testing
- Time: 4.5 days (Week 2-4)

**Total Team:** 4 engineers, 24.5 engineer-days over 4 weeks

### Minimum Team (2-3 Engineers)

If limited staff, prioritize:
- 1 Senior Full-Stack Engineer: EPIC-1, EPIC-2, EPIC-4
- 1 zkVM Specialist: EPIC-3
- 1 QA/DevOps: EPIC-5, EPIC-6

Timeline extends to 5-6 weeks with reduced parallelization.

---

## Success Criteria

### Week 1 Completion

- ✅ TransactionRecordingService complete and tested
- ✅ UserRegistration integrated
- ✅ DealOrigination integrated
- ✅ Test script updated for user registration
- ✅ First integration test passing

### Week 2 Completion

- ✅ Payment + Closeout endpoints created
- ✅ Collateral zkVM module 80% complete
- ✅ Deal operations integration tests passing
- ✅ Test script updated for collateral creation/association

### Week 3 Completion

- ✅ All collateral processors implemented
- ✅ Collateral API fully integrated
- ✅ Collateral integration tests passing
- ✅ Double-pledge prevention working in ZK

### Week 4 Completion

- ✅ All E2E tests passing
- ✅ Performance validated (<5 min latency)
- ✅ Test script achieves 60%+ coverage (100% ideal)
- ✅ Documentation complete
- ✅ Ready for staging deployment

---

## Next Steps

1. **Review this implementation plan** with engineering team
2. **Create Linear/Jira epics** from this document
3. **Assign owners** to each work stream
4. **Schedule kickoff** with all team members
5. **Set up daily standups** for synchronization
6. **Create #zkvm-integration** Slack channel for coordination

---

## Appendix: Epic Template for Issue Tracking

### Epic Template

```markdown
# EPIC-X: [Title]

**Work Stream:** WS-X
**Priority:** HIGH/MEDIUM/LOW
**Effort:** X days
**Owner:** [Name]
**Dependencies:** [List of blocking epics]

## Description
[What this epic accomplishes]

## Acceptance Criteria
- [ ] Criterion 1
- [ ] Criterion 2

## Technical Scope
- Files to create/modify
- Key functions to implement

## Sub-Issues
- EPIC-X.1: [Title] (X hours)
- EPIC-X.2: [Title] (X hours)

## Testing Strategy
[How to validate completion]

## Documentation Updates
[What docs need updating]
```

### Sub-Issue Template

```markdown
# EPIC-X.Y: [Title]

**Parent Epic:** EPIC-X
**Effort:** X hours
**Dependencies:** [Blocking sub-issues]

## Description
[Specific task to complete]

## Implementation Details
- File: [path]
- Function: [name]
- Changes needed: [list]

## Acceptance Criteria
- [ ] Code complete
- [ ] Tests passing
- [ ] Code reviewed
- [ ] Documentation updated

## Code References
[Line numbers, existing examples]
```

---

**End of Implementation Plan**
