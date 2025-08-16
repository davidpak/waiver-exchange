# 0001. Determinism Model
- Status: Accepted
- Date: 2025-08-15

## Context
Replay must be bitwise-stable. Matching is tick-bounded. Any hashing/ordering must be reproducible across runs and machines (with pinned toolchain/flags).

## Decision
- **Tick-bounded:** All state mutations occur only inside `tick(T)`. No effects outside.
- **Priority key:** `(ts_norm, enq_seq)`; earlier wins. `enq_seq` is per-symbol, per-tick.
- **Cancel vs fill (same tick):** Earlier `(ts_norm, enq_seq)` wins. No heuristics.
- **Execution IDs:** default **Sharded**:
`exec_id = (tick << exec_shift_bits) | local_seq_in_tick`
`local_seq_in_tick` increments per Trade/Lifecycle emission and resets each tick.
External/global stamping by ExecutionManager is allowed when `exec_id_mode = External`.
- **Integer-only decisioning:** Prices/qty/times are integers. No floats on the hot path.
- **No wall-clock dependence:** Only logical tick/time participates in decisions.
- **Stable hashing:** Prefer arrays/indices. If hashing is used, use a fixed seed.
- **Thread/NUMA:** One engine/thread per symbol; memory allocated on same NUMA node; engine does not migrate during lifetime.

## Consequences
- Replay equality is testable and enforceable.
- Builds must be pinned (compiler + flags) for reproducibility in replay CI.