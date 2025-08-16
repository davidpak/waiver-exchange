# 0002. Data Model and Reject Codes
- Status: Accepted
- Date: 2025-08-15

## Context
Admission must be explicit and deterministic. Price arithmetic must be integer-based with tick alignment and bands.

## Decision
- **Price model:** `PriceDomain { floor, ceil, tick }`. `idx(price)` only if aligned and in range.
- **Bands:** `Bands { mode: Abs(Price) | Percent(bps) }`. Compute without floats.
- **Cold start policy:** If no reference price:
  - `MARKET` → Reject(MarketDisallowed)
  - `IOC`    → Reject(IocDisallowed)
  - Only in-band `LIMIT` accepted until first trade sets `ref_price`.
- **POST-ONLY rule:** If it would cross at submitted price → Reject(PostOnlyCross). No slide/price-improve.
- **Admission order (first failure wins):**
  1. Arena capacity
  2. Duplicate order id
  3. Market halted
  4. Tick alignment (`BadTick`)
  5. Price bands (`OutOfBand`)
  6. Type/side constraints (MARKET/IOC semantics)
  7. Max size/exposure
  8. Risk cache verdict present (non-blocking); miss/fail ⇒ rejection
- **Stable reject reasons** (append-only; never renumber):
  - `BadTick`
  - `OutOfBand`
  - `PostOnlyCross`
  - `MarketDisallowed`
  - `IocDisallowed`
  - `RiskUnavailable`
  - `InsufficientFunds`
  - `ExposureExceeded`
  - `ArenaFull`
  - `QueueBackpressure`
  - `Malformed`
  - `UnknownOrder`
  - `SelfMatchBlocked`
  - `MarketHalted`

## Consequences
- Deterministic, analyzable rejections; UI and analytics rely on stable enums.
- Hot path remains allocation-free and non-blocking.
