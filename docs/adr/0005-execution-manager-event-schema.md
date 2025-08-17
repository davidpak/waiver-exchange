# 0005. ExecutionManager Event Schema & Backpressure
- Status: Accepted
- Date: 2025-08-16

## Context
Downstream consumers (Replay, Analytics, UI) require a stable schema and strict delivery/overflow policy. See spec §2.5.2–§2.5.10.

## Decision
**Event families (per symbol, per tick T, canonical order):**
1) Trades
2) BookDeltas (coalesced per (side, price_idx) to final post-tick state)
3) OrderLifecycle (Accepted | PartiallyFilled | Filled | Cancelled | Rejected)
4) TickComplete (exactly one)

**Trade fields:** {symbol, tick, maker_order_id, taker_order_id, maker_acct, taker_acct, price_idx, qty, aggressor_side, ts_norm, seq_in_tick, (optional price_raw)}  
**BookDelta fields:** {symbol, tick, side, price_idx, new_total_qty}  
**OrderLifecycle fields:** {symbol, tick, order_id, account_id, event, reason?, last_fill_price_idx?, last_fill_qty?, remaining_qty}  
**TickComplete fields:** {symbol, tick}

**Sequencing:** Maintain `seq_in_tick` for Trades and Lifecycle; start=0 each tick; strictly increasing; no gaps.

**Exec IDs:** Default **Sharded**: `exec_id = (tick << exec_shift_bits) | seq_in_tick`. If `exec_id_mode = External`, Whistle sets 0 and ExecutionManager stamps deterministically.

**Backpressure:**
- MPSC from Whistle → ExecutionManager: **non-blocking**.
- WAL sink (Replay) is **lossless**; if it can’t keep up ⇒ **fatal** (simulation halts with diagnostic).
- UI/Analytics sinks may be lossy by policy (never drop Trades/BookDeltas for WAL).

## Consequences
Replay determinism, stable schema for encoding (Flatbuffers), clear failure semantics, and bounded queues consistent with spec.
