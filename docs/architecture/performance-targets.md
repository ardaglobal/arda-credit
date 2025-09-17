# Performance & Capacity Targets (ZK Program)

This document captures initial throughput, batching, and latency targets for the pilot and near-term growth scenarios. It translates business activity into concrete transaction volumes and transactions-per-second (TPS) goals for the zkVM batch processor.

For a high-level overview of the three-component architecture (smart contracts, ZKVM program, and web server), see [`docs/architecture/three-component-overview.md`](../architecture/three-component-overview.md).

## Overview

- New users per day (U): 100
- New loans per day (L): 10
- Transaction mapping per business activity:
  - Per user: `UserRegistration` + `KycSubmission` + `KycVerification` + `UserDeposit` (1 each)
  - Per loan: 2 admin ops (e.g., due diligence + approval), `LoanOrigination`, `UserWithdrawal` (1 each)
  - Repayments: one `LoanPayment` per month for 12 months; each payment paired with a `UserDeposit`; `LoanCloseout` at payoff
- Business-hours constraint: all traffic occurs Mon–Fri, 9am–5pm local (8 hours/day)
- Future system may include more transaction types; pilot planning also considers a 5× complexity multiplier on total tx volume
- Capacity planning includes 10× headroom on sustained and burst rates

Note: Admin ops (due diligence, approval) are counted as 2 tx/loan; whether they live inside the ZK batch or remain off-chain admin events can be revisited. The targets below assume they are included to size conservatively.

## Transaction Volume Formula

Let m be the month index since start (0..12). Daily transactions at month m:

DailyTx(m) = 4U + 4L + 2·m·L + [m ≥ 12 ? L : 0]

- 4U: user reg + KYC submit + KYC verify + deposit
- 4L: loan due diligence + approval + origination + withdrawal
- 2·m·L: monthly payments (m·L) plus payment-paired deposits (m·L)
- +L at m ≥ 12: daily closeouts begin once loans mature (12-month loans)

Plugging U=100, L=10:
- m=0: 440/day
- m=6: 560/day
- m=12: 690/day (steady-state peak)

Average TPS (no business-hours constraint) = DailyTx / 86,400

## Business-Hours, 5× Complexity, and 10× Headroom

For planning, assume:
- 5× multiplier on total transaction count (final app has ~5× the transaction variety/volume)
- All traffic within business window (8 hours/day)
- 10× headroom on sustained and burst traffic

Steady-state (m=12):
- Base daily: 690 → 5× = 3,450 tx per business day
- In-window sustained TPS: 3,450 / 28,800 ≈ 0.12 TPS
- With 10× headroom: target sustained ≥ 1.2 TPS

Interim checkpoints:
- m=0: 440 → 5× = 2,200/day → 2,200 / 28,800 ≈ 0.076 TPS → headroom target ≥ 0.76 TPS
- m=6: 560 → 5× = 2,800/day → 2,800 / 28,800 ≈ 0.097 TPS → headroom target ≥ 0.97 TPS

Weekly capacity (Mon–Fri): 3,450 × 5 = 17,250 tx/week (headroom target ≥ 172,500 tx/week)

## Burst/Peaks (Payment Clustering)

Payments often cluster by day-of-month or hour. At m=12 without the 5× multiplier, daily payments + paired deposits ≈ 240 tx/day; concentrated in 1 hour → ~0.067 TPS. With 5× multiplier:
- Clustered hour: ≈ 1,200 tx/hour ≈ 0.33 TPS
- With 10× headroom: burst capacity target ≥ 12,000 tx/hour ≈ 3.3 TPS

Add baseline traffic and other ops; targets above remain safely conservative.

## Batch Sizing & Latency Targets (ZK Proofs)

Recommended initial SLOs (cover pilot + headroom):

- Throughput
  - Sustained in-window: ≥ 1.2 TPS (10× headroom over 0.12 TPS steady-state)
  - Short burst (≥ 1 hour): ≥ 3.3 TPS (10× headroom over clustered hour)
- Proof capacity
  - Large batch: ≥ 10,000–20,000 tx/proof (absorbs bursty hours with margin)
  - 15‑minute batches: ≥ 1,500–2,500 tx/proof (gives >10× headroom on typical 15‑min windows)
- Latency
  - Proof wall‑clock: ≤ 5 minutes for 10k tx; ≤ 2 minutes for ≤ 2.5k tx
  - End‑to‑end inclusion (ingest → on‑chain): ≤ 30 minutes during business hours
- Operational cadence
  - Default: 15‑minute batch interval during business hours; relax to hourly off‑peak

These SLOs materially exceed pilot needs (and allow growth before re‑tuning).

## How to Recompute for Different U and L

1) Compute DailyTx(m) = 4U + 4L + 2·m·L + [m ≥ 12 ? L : 0]
2) Apply scenario multipliers (e.g., 5× overall)
3) Apply business-hours constraint: TPS = DailyTx / 28,800
4) Size sustained target with 10× headroom
5) Identify likely clustered windows (e.g., monthly payments) and apply 10× headroom to that peak

## Notes & Next Steps

- If admin loan ops remain off-chain and not part of the ZK state, real ZK transaction counts decrease; targets above are thus conservative.
- If deposits/withdrawals are off-chain for custodied flows, adjust the 4U/4L terms accordingly.
- As we benchmark the zkVM and proof system, we’ll refine per‑proof capacity versus wall‑clock; targets above provide ample margin for the pilot.
- Monitoring KPIs to instrument:
  - Ingest queue depth, queue latency
  - Batch assembly time, proof time, on‑chain submit time
  - Inclusion SLA percentiles (P50/P95)
  - Error rate / retries per batch

## Quick Reference (Pilot Targets)

- Sustained TPS (business hours): ≥ 1.2
- 1‑hour burst TPS: ≥ 3.3
- Proof capacity: ≥ 10k–20k tx/proof
- Proof time: ≤ 5 minutes (10k tx)
- Inclusion SLA: ≤ 30 minutes (P95) during business hours
- Weekly capacity (Mon–Fri): ≥ 172,500 tx/week

These targets comfortably cover 100 users/day and 10 loans/day at 5× complexity under business-hours load with 10× headroom.

## Macro-Scale Scenario: US Economy Scale (6 Loans/Second)

For context on long-term scalability, this section models what the system would look like at US economy scale: 6 loans per second sustained, plus 100,000 user onboarding.

### Transaction Volume at Scale

- Loan origination rate (λ): 6 loans/second
- Steady-state lifetime tx per loan: 2 admin ops + 1 origination + 1 withdrawal + 12 payments + 12 payment-paired deposits + 1 closeout = 29 ZK tx/loan lifetime
- Sustained TPS (24/7): 29 × 6 = 174 TPS
- Daily volume: 174 × 86,400 ≈ 15,033,600 tx/day

### Business-Hours Constraint (8 hours/day)

If traffic remains concentrated in business hours despite scale:
- In-window sustained TPS: 15,033,600 / 28,800 ≈ 522 TPS
- Batch sizing (1-minute): ~31,320 tx
- Batch sizing (5-minute): ~156,600 tx
- Batch sizing (15-minute): ~469,800 tx

### 100,000 User Impact

- One-time onboarding: registration + KYC submit + KYC verify + deposit = 4 tx/user
- Total onboarding tx: 400,000
  - Spread over 24h: ~4.6 TPS
  - Spread over 8h: ~13.9 TPS
- Impact: Modest compared to daily loan flows; adds <3% to daily volume at this scale

### Targets to Support Macro Scale

- Sustained throughput:
  - 24/7 scenario: ≥ 200 TPS (10× headroom over 174 TPS)
  - Business-hours scenario: ≥ 600 TPS (10× headroom over 522 TPS)
- Proof capacity/latency:
  - Per-proof capacity: ≥ 50,000–200,000 tx
  - Proof time: ≤ 1–5 minutes for large batches
- Ops cadence:
  - 1-minute micro-batches per shard; aggregate every 5–15 minutes
  - Horizontal scaling: Multiple provers to keep proof times sub-2–5 minutes at peak
  - Batch sharding: Split into multiple ~10k–20k tx proofs/minute and aggregate

This macro-scale analysis shows the system could scale to US economy levels (6 loans/second = ~15M daily tx) with appropriate horizontal scaling and proof time optimization.
