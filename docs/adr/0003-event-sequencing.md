# 0003. Event Sequencing Contract
- Status: Accepted
- Date: 2025-08-15

## Context
Downstream (ExecutionManager, Replay, UI) needs a fixed, canonical order of events per tick for stable WAL encoding and deterministic replay.

## Decision
Within a single **symbol** and `tick(T)`, Whistle emits to ExecutionManager in this **canonical order**:
1. **Trades** (ordered by `(price_idx, maker.ts_norm, maker.enq_seq)` as produced)
2. **BookDeltas** (**coalesced** per `(side, price_idx)` to the final post-state for the tick)
3. **OrderLifecycle** (`Accepted | PartiallyFilled | Filled | Cancelled | Rejected`)
4. **TickComplete** (exactly one per symbol per tick)

**Sequencing:**
- Maintain `seq_in_tick` counter for **Trades** and **Lifecycle**; starts at 0 each tick; strictly increasing; no gaps.
- BookDeltas are emitted after Trades, in a fixed key order (choose and freeze: e.g., Bid then Ask, each by ascending `price_idx`).

**Backpressure:**
- Whistle never blocks on MPSC to ExecutionManager.
- Replay/WAL sink is lossless; overflow is **fatal** per policy.
- UI/analytics fanouts may be lossy per policy (not part of this contract).

**Exec IDs:**
- If `exec_id_mode = Sharded`, Whistle stamps `exec_id` using `(tick << exec_shift_bits) | seq_in_tick`.
- If `External`, Whistle sets `exec_id = 0` and ExecMgr stamps deterministically.

## Consequences
- Replay can validate per-tick hashes; UI logic can assume event families’ order.
- Any change to ordering is a breaking change and requires a new schema version.
