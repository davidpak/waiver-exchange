# 0004. Self-Match & Cancel-vs-Fill Policy
- Status: Accepted
- Date: 2025-08-15

## Context
Preventing self-trades is a fairness/realism requirement. The doc allows policy variants.

## Decision
- **Self-match policy:** default **Skip** — aggressor never trades against own resting orders; engine scans FIFO and skips equal `account_id`.
  - Alternatives supported via config: `CancelResting` or `CancelAggressor`.
- **Cancel vs fill race in same tick:** earlier `(ts_norm, enq_seq)` wins (from ADR-0001).
- **Complexity bound:** skip scan is bounded by FIFO at current best price; typically small; acceptable for hot path.

## Consequences
- Deterministic outcomes; minimal side effects under default Skip.
- Tests: construct interleavings to assert policy behavior and race resolution.
