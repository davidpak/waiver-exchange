# The Waiver Exchange - System Architecture and Design Overview (v0.2)

Owner: David Pak

Status: Draft → Accept upon merge

Scope: End-to-end architecture, invariants, and interfaces for the Waiver Exchange

Audience: Engineers, reviewers, curious learners. This is the source of truth.

Normative language: MUST / SHOULD / MAY

## 1. Project Overview

**The Waiver Exchange** is a fully self-contained, high-performance, deterministic, **tick-bounded** trading simulator where fantasy football players are traded like financial instruments. Prices are determined purely by supply and demand within the system (no scoring or real-world P&L influence), but real-world player data is surfaced to inform decisions. 

This system is designed for **performance, determinism, observability, and extensibility**. It aims to uphold the highest standard for building trading systems, system design, and market simulation.

At the core is `Whistle`, the per-symbol matching engine that executes trades using **strict price-time priority**. Around it, a set of well-bounded services handle ingress, orchestration, fan-out, persistence, analytics, and risk—without touching the hot path.

### Role in the System

`Whistle` is a **leaf-level execution core**—one instance per symbol—driven by a deterministic `SimulationClock`. Orders enter through `OrderGateway`, are routed by `OrderRouter` to the correct symbol’s **SPSC** queue, matched inside Whistle, and then emitted to `ExecutionManager` as canonical engine events. `ExecutionManager` fans out to **Replay (WAL)**, **Analytics**, and **WebUI**. `AccountService` provides **read-only** risk cache verdicts for admission. `SymbolCoordinator` owns engine lifecycle and placement.

### Core Responsibilities (system-level)

- **Order ingress & normalization** (OrderGateway): parse, validate API payloads, apply latency profiles.
- **Routing & fan-in** (OrderRouter): per-symbol SPSC enqueue; reject on backpressure—never block.
- **Per-symbol execution** (Whistle): validate, admit, match by **price-time**, maintain book state, emit canonical events.
- **Time & scheduling** (SimulationClock): drive `tick(now)` for active symbols in a deterministic order.
- **Lifecycle & placement** (SymbolCoordinator): spawn/pin/evict engines; wire queues; manage cold/warm start.
- **Fan-out & IDs** (ExecutionManager): enforce emission order to sinks; stamp global `execution_id` if centralized.
- **Persistence & replay** (ReplayEngine): lossless WAL of inputs/outputs for **byte-identical** replay.
- **Observability** (AnalyticsEngine): metrics and aggregates; optional lossy policy that never affects WAL.
- **Risk & balances** (AccountService): admission verdicts via local cache; authoritative ledger **off hot path**.
- **Presentation** (WebUI): visualize depth/trades; strictly read-only.

### Out of Scope (for the hot path)

- Networking and auth semantics (edge).
- Cross-symbol ordering, global coordination, or UI transport guarantees.
- Database I/O, formatting, or logging in match loops.
- Real-world data ingestion affecting price formation.

### Invariants (system-level, non-negotiable)

1. **One engine per symbol.** It never migrates threads during its lifetime.
2. **Tick-bounded execution.** All state changes happen inside `tick(now)`; every output is attributable to a tick.
3. **Price-time fairness.** Priority key = `(normalized_timestamp, enqueue_sequence)`.
4. **Canon:** Per-tick emission order is **Trades → BookDeltas → OrderLifecycle → TickComplete**.
5. **No hot-path heap/locks/syscalls.** Arenas/queues preallocated; maintenance only at **boundaries**.
6. **Backpressure never blocks ingress.** SPSC full ⇒ **Reject(Backpressure)**; WAL overflow ⇒ **fatal**; Analytics/UI may drop by policy.
7. **Deterministic IDs.** Either `(tick << SHIFT) | local_seq` (sharded) or centralized—but always replay-stable.
8. **External data is advisory.** It informs users/bots/UI, not engine decisions.

| Constraint | Implication |
| --- | --- |
| **Determinism** | Same inputs + config + tick schedule ⇒ byte-identical outputs (pre `execution_id`). |
| **Isolation** | Per-symbol state; no global locks; pinned threads; NUMA-local memory. |
| **Low latency** | Decision p50 ≤ 1.5 µs, p99 ≤ 3.0 µs (dequeue → decision, no I/O). |
| **Bounded queues** | SPSC inbound / MPSC outbound fixed-capacity; behavior on overflow is explicit. |
| **Cache-aware layout** | Flat ladder + arena + bitset; minimal pointer chasing and false sharing. |
| **Schema/version stability** | WAL/Snapshot/Event schemas versioned; changes gated by ADRs. |
| **Replayability** | Snapshots only at boundaries; resume at `T+1` and WAL replay to match hashes. |
| **Operational clarity** | Explicit failure policies; counters/timers emitted without perturbing the hot path. |

---

## 2. High-Level Component Breakdown

### Matching Core

| **Component** | **Description** | **Tech Stack** |
| --- | --- | --- |
| `Whistle` (Matching Engine) | Per-symbol, high-performance price-time matcher; deterministic, tick-bounded execution | **Rust** (safety, perf, deterministic concurrency) |
| `OrderBook` | Bid/ask ladders with intrusive FIFO queues, bitset navigation, O(1) cancels | **Rust** (tight integration with `Whistle`) |
| `SymbolCoordinator` | Owns engine lifecycle: spawn/evict, thread/NUMA placement, queue wiring | **Rust** (task runtime, resource control) |

### 2. 1 Ingestion & Routing

| **Component** | **Description** | **Tech Stack** |
| --- | --- | --- |
| `OrderGateway` | External ingress (API or sim); validates envelope, rate-limits, authenticates | **C++** (low-latency I/O) *(Rust variant acceptable)* |
| `OrderRouter` | Routes to per-symbol SPSC queue; stamps `enq_seq`; handles backpressure | **C++** (lightweight fan-in/out) *(Rust variant acceptable)* |
| `LatencyModel` | Applies deterministic synthetic latency per account/strategy before enqueue | **Rust** (precision, fairness enforcement) |

### 2.2 Post-Match & Observability

| **Component** | **Description** | **Tech Stack** |
| --- | --- | --- |
| `ExecutionManager` | Receives canonical events; assigns `execution_id` (default centralized); fans out to sinks | **C++** (batching, zero-copy encode: Flatbuffers/Cap’n Proto) |
| `AnalyticsEngine` | Aggregates metrics (latency, volumes, rejects); writes columnar stores | **Rust** *(or C++ if shared with ExecMgr pipeline)* |
| `AccountService` | Read-only risk cache for admission; authoritative balances/P&L updates out of band | **Rust** (ownership model, SIMD where useful) |
| `PersistenceLayer` | WAL segments, snapshots, and metrics storage | **Rust/C++** (Flatbuffers/Parquet/ClickHouse/RocksDB) |
| `ReplayEngine` | Deterministic replayer: WAL → engine inputs; validates event/book hashes | **Rust** (snapshotting, file I/O) |

### 2.3 Control & Simulation

| **Component** | **Description** | **Tech Stack** |
| --- | --- | --- |
| `SimulationClock` | Drives logical ticks; enforces per-tick work caps; orders symbol iteration | **Rust** (pure, testable) |
| `AdminShell` | Operational CLI: start/stop symbols, load snapshots, inspect queues | **C++** *(or Rust for tighter integration)* |

### 2.4 UI, Bots & External Data

| **Component** | **Description** | **Tech Stack** |
| --- | --- | --- |
| `StrategyEngine` | Runs sandboxed bots (market-making, momentum, hype); SMP-aware; deterministic | **Rust** core + **WASM/Python** plug-ins |
| `MarketDataViewer` | Surfaces real-world player signals (injuries, rankings) for human/bot context | **Rust/C++** (API adapters) |
| `DataIngestion` | Pulls/normalizes external data; never drives price, only informs | **Rust** *(or Python for ETL jobs)* |
| `WebUI` | Real-time depth/trades, account view, controls for sim/bots | **TypeScript + React** (WebSockets) |

*Notes:*  Execution IDs are centralized by default for cross-symbol monotonicity; sharded mode is available when configured. All per-symbol hot paths remain allocation-free and tick-bounded; serialization, persistence, and analytics are downstream.

---

## 3.3 System Flow (High-Level)

1. **Submit** — A user or bot submits an order (`LIMIT`, `MARKET`, `IOC`, `POST-ONLY`).
2. **Ingress** — `OrderGateway` validates shape and forwards to `OrderRouter`.
3. **Placement** — `OrderRouter` looks up or activates the symbol’s `Whistle` via `SymbolCoordinator` (one engine per symbol).
4. **Normalize Time** — If enabled, `LatencyModel` stamps a **normalized timestamp (TsNorm)**; `OrderRouter` attaches **Enqueue Sequence (EnqSeq)**.
5. **Enqueue** — The message is written to the symbol’s **SPSC ring** (single producer = router, single consumer = `Whistle`). Backpressure policy: **full ⇒ Reject(Backpressure)**, never block.
6. **Tick** — On the next logical tick `T`, `SimulationClock` calls `Whistle.tick(T)` for each active symbol.
7. **Ingest & Validate** — `Whistle` drains up to `batch_max` messages and runs deterministic checks:
    - Tick-size alignment
    - Price bands vs. reference price (cold/warm rules)
    - Type semantics (`POST-ONLY` must not cross; `MARKET/IOC` never rest)
    - Risk cache verdict (non-blocking; miss ⇒ reject)
    - Structural limits (arena capacity, duplicate order IDs)
8. **Match** — Valid orders are processed under **strict price-time priority**:
    - Crossed liquidity trades first; partials retain priority
    - Self-match prevention policy enforced (default: skip)
    - Remainders: `LIMIT/POST-ONLY` may rest; `MARKET/IOC` cancel remainder
9. **Book Maintenance** — Resting orders are stored in an intrusive FIFO per price level; level totals and best pointers update; empty/non-empty levels tracked via bitset.
10. **Emit (Canonical Order)** — After matching completes for tick `T`, `Whistle` emits:
    1. **Trades**
    2. **BookDeltas** (coalesced end-of-tick quantities per level)
    3. **OrderLifecycle** (Accepted/Partial/Filled/Cancelled/Rejected)
    4. *TickComplete { symbol, T }`
11. **Fan-out & Persist** — `ExecutionManager` stamps global `execution_id` (if centralized), then fans out to:
    - **ReplayEngine** (lossless WAL, snapshots)
    - **AnalyticsEngine** (metrics, aggregates)
    - **WebUI** (real-time depth, trades; may drop frames by policy)
12. **Advance** — `SimulationClock` advances to `T+1`. Coordinator may spawn/evict engines at **tick boundaries** only.

**Notes**

- All effects are **tick-bounded**; nothing mutates between `tick(T)` calls.
- Ingress and egress queues are **bounded**; backpressure outcomes are explicit and deterministic.
- Cold starts reject `MARKET/IOC` until a first trade establishes a reference price.

---

## 4. Core Event Queues & Data Flow

| Source → Destination | Queue Type | Contract & Purpose |
| --- | --- | --- |
| `OrderRouter` → `Whistle` (per symbol) | **SPSC ring** | Deterministic, low-latency ingress; single producer/consumer; **full ⇒ Reject(Backpressure)**; preserves `(TsNorm, EnqSeq)` order. |
| `Whistle` → `ExecutionManager` | **MPSC queue** | Canonical per-tick event sequence (**Trades → BookDeltas → OrderLifecycle → TickComplete**). No drops allowed for Trades/BookDeltas. |
| `ExecutionManager` → `ReplayEngine` / `AnalyticsEngine` / `WebUI` | Internal fan-out | WAL is lossless and ordering-stable; analytics/UI can be lossy by policy (never Trades/BookDeltas for replay). |

## Supported Order Types

Each order type allows traders (human or bot) to define **intent -** whether they want to add liquidity, sweep it, or do so only if the market conditions are ideal.

| Type | Description |
| --- | --- |
| `LIMIT` | Submit at a specified price or better; can rest in the book |
| `MARKET` | Take best available liquidity immediately (slippage possible) |
| `IOC` | Fill any available amount immediately; cancel remainder |
| `POST-ONLY` | Add liquidity only — cancels if it would cross the spread |

## 5. Event Model & Canonical Sequencing

This section defines **what gets emitted**, **in what order**, and **who stamps which fields**. It is the contract that keeps **Whistle** hot, **ExecutionManager** authoritative, and **Replay** byte-identical.

### 3.1 Event families (exactly four)

1. **Trade** — a single maker/taker match at a specific price/qty.
    
    Fields: `symbol`, `tick`, `taker_side`, `price_idx` (and/or `price_raw`), `qty`, `maker_order_id`, `taker_order_id`, `maker_acct`, `taker_acct`, `ts_norm` (aggressor), `seq_in_tick` (**Whistle** stamps), optional `exec_id` (see 2.2).
    
2. **BookDelta** — level totals after all updates for that level **within the tick**.
    
    Fields: `symbol`, `tick`, `side`, `price_idx`, `new_total_qty`.
    
3. **OrderLifecycle** — state transitions for individual orders.
    
    Kinds: `Accepted | PartiallyFilled | Filled | Cancelled | Rejected`.
    
    Fields: `symbol`, `tick`, `order_id`, `account_id`, `event`, `reason?` (for `Rejected`), `last_fill_price_idx?`, `last_fill_qty?`, `remaining_qty`, `seq_in_tick`.
    
4. **TickComplete** — end-of-batch marker, one per symbol per tick.
    
    Fields: `symbol`, `tick` (plus optional diagnostics behind a feature flag).
    

**Design choice:** four families only. No ad-hoc events in the hot path. This guarantees stable ordering and compact schemas.

---

### 3.2 Stamping authority (who writes which fields)

| Field | Stamped by | Rationale |
| --- | --- | --- |
| `symbol`, `tick`, `price_idx`, `qty`, `taker_side` | **Whistle** | Known in the match loop; zero latency. |
| `maker_order_id`, `taker_order_id`, accounts | **Whistle** | Immediate from arena; needed for replay & risk. |
| `seq_in_tick` | **Whistle** | Deterministic per-tick counter for total ordering. |
| `execution_id` (global) | **ExecutionManager** (default) | Ensures cross-symbol monotonicity without coupling engines. Sharded mode available when configured. |
| Final encoding (Flatbuffers/Cap’n Proto) | **ExecutionManager** | Keep serialization off the hot path. |

**Design choice:** centralize `execution_id` by default for cross-symbol monotonicity; allow a **sharded** `(tick << SHIFT) | local_seq` mode when we need zero fan-out work. Both are **replay-stable**.

---

### 3.3 Canonical per-tick order (non-negotiable)

Within a single `tick()` per symbol, events **must** be emitted in this sequence:

1. **Trades** → 2) **BookDeltas** → 3) **OrderLifecycle** → 4) **TickComplete**

**Why:**

- Trades define economics.
- BookDeltas reflect the **post-trade** level state.
- Lifecycle events communicate admission/results.
- TickComplete is the stable barrier for Replay & Analytics.

---

### 3.4 Sequencing & determinism

- `seq_in_tick` starts at **0** at tick entry.
- Increment on every **Trade** and every **OrderLifecycle** emission **in the order those facts become true**.
- `BookDelta` entries do **not** consume `seq_in_tick` (they’re coalesced summaries; see 2.5).
- When two outcomes are possible in the same tick (e.g., **cancel vs. fill race**), the tie breaks by the engine’s priority key:
    
    `(ts_norm, enq_seq)` — **earlier wins deterministically**.
    

**Design choice:** keep `BookDelta` outside the sequence to avoid artificial gaps and allow coalescing without perturbing ordering.

---

### 3.5 Coalescing rules for BookDeltas

- Track touched levels in a tiny per-tick map keyed by `(side, price_idx)`.
- Store the **final post-tick** `new_total_qty`.
- Emit once per key after Trades, in a fixed order:
    - **Option A (default):** `Bid` then `Ask`, each **ascending `price_idx`** (consistent for UI & hashing).
    - **Option B:** `Ask` ascending, `Bid` **descending** (classic market view). Pick one and **freeze** it.

**Design choice:** single emission per level per tick keeps UI/live readers simple and Replay hashes stable.

---

### 3.6 Delivery semantics & backpressure

- Whistle enqueues events to ExecutionManager via a **bounded MPSC** **without blocking**.
- On push failure:
    - **Replay sink (WAL)** cannot drop: policy is **fatal**. Set `fatal_flag`, finish the tick (still emit `TickComplete`), coordinator evicts at boundary.
    - **Analytics/UI** may be **lossy** under an explicit feature flag (never drop Trades/BookDeltas; only non-critical Lifecycle if configured).

**Design choice:** losslessness where it matters (Replay), optional lossy where it doesn’t (UI), never stall the engine.

---

### 3.7 Schema & versioning notes

- Every event carries a **schema version** in the envelope.
- Breaking changes require an ADR and bump; ExecMgr must be able to **route/upgrade** or **reject** mixed versions.
- WAL stores **pre-`execution_id`** bytes for replay; ExecMgr may persist a **post-ID** stream for analytics. Both are versioned.

**Design choice:** record the engine’s native stream to guarantee replay; derived streams are convenience, not source of truth.

---

### 3.8 Invariants & validation (cheap to assert)

- Exactly **one** `TickComplete` per symbol per tick.
- No `Trade` without corresponding qty updates in book state.
- `BookDelta(new_total_qty)` equals the ladder’s computed total at end of tick.
- `seq_in_tick` is **strictly increasing**, starts at 0, no gaps for Trade/Lifecycle.
- Replaying identical inputs yields byte-identical **pre-ID** event streams (hash match).

Optional cheap hashes:

- **Event hash (per tick):** rolling xxh3 over emitted bytes (pre-ID).
- **Book hash (post-tick):** xxh3 over `(side, price_idx, level_qty, head_id?, tail_id?)`.

---

### 3.9 Tiny example (one symbol, one tick)

1. Aggressor BUY LIMIT crosses two resting SELL orders at `p=120`:
    - Emit `Trade(seq=0, maker=O1, taker=A, qty=5)`
    - Emit `Trade(seq=1, maker=O2, taker=A, qty=3)`
2. Level 120 qty went from 10 → 2, level 121 unchanged:
    - Emit `BookDelta(Ask, 120, new_total_qty=2)`
3. Order lifecycle:
    - `Accepted(A)` (if it wasn’t resting) → `Filled(A, remaining=0, seq=2)`
    - `Filled(O1, remaining=0, seq=3)`; `PartiallyFilled(O2, remaining=2, seq=4)`
4. Emit `TickComplete`.

(If centralized IDs are enabled, ExecMgr stamps `execution_id` for both Trades after receipt.)

---

### 3.10 “Done when” (acceptance for this section)

- Unit tests assert the **exact** family order and `seq_in_tick` monotonicity.
- Property tests cover cancel/fill races (earlier `(ts_norm, enq_seq)` always wins).
- Replay harness verifies **byte-identical** pre-ID event streams and matching **book hash** per tick.
- Backpressure tests show: WAL overflow ⇒ **fatal**; Analytics overflow ⇒ **drops allowed** (when flag enabled) with no perturbation of Trade/BookDelta.

---

## 6. System Time, Ticks, & Determinism

This system runs on **logical time**. All market effects are **tick-bounded**: nothing changes between `tick(T)` calls, and every output is attributable to a specific tick.

### 6.1 Time Sources

- **`SimulationClock` (authoritative):** Drives ticks for all active symbols. Calls `Whistle.tick(T)` once per symbol per tick.
- **`TsNorm` (normalized timestamp):** Per-order logical timestamp applied *before* enqueue by `LatencyModel`. Used for price-time priority within a tick.
- **`EnqSeq` (enqueue sequence):** Monotonic tie-breaker stamped by `OrderRouter` as the order enters the SPSC. Unique within `(symbol, T)`.

### 6.2 Invariants (non-negotiable)

1. **Tick-bounded execution:** All admission, matching, and emissions happen inside `tick(T)`. No state mutation occurs outside.
2. **Deterministic priority:** Within a price level, ordering is by `(TsNorm, EnqSeq)`; partial fills **retain** their original priority.
3. **One engine per symbol:** A `Whistle` instance never migrates threads during its lifetime.
4. **Canonical per-tick emission:** **Trades → BookDeltas → OrderLifecycle → TickComplete** exactly once per symbol per tick.
5. **No hot-path allocation or syscalls:** Preallocated arenas/queues; diagnostics are async.
6. **Bounded queues, explicit backpressure:**
    - Ingress SPSC full ⇒ `Reject(Backpressure)` (router sees it); never block.
    - Egress to `ExecutionManager` overflow ⇒ **fatal for replay** (symbol evicted at boundary); UI/analytics may be lossy by policy, never Trades/BookDeltas.

### 6.3 Tick Cadence & Scheduling

- **Cadence:** Fixed-rate ticks (e.g., 1–10 kHz) or step-driven by tests/CLI; pick once per run for determinism.
- **Fairness:** `SimulationClock` iterates symbols in a **stable order** each tick. Optional `batch_max` caps per-tick work per symbol to prevent starvation.
- **Boundary work only:** Any compaction (e.g., `OrderIndex` tombstone rebuild), snapshotting, or spawn/evict occurs **after** `TickComplete(T)` and before `tick(T+1)`.

### 6.4 Admission Order (within `tick(T)`)

When `Whistle.tick(T)` runs, it drains up to `batch_max` messages from the symbol’s SPSC **in producer order** and applies **fail-fast validation**:

1. **Capacity/structure:** Arena space, duplicate ID, message shape.
2. **Market rules:** Tick-size alignment; price bands vs reference price.
3. **Type semantics:** `POST-ONLY` must not cross; `MARKET/IOC` never rest.
4. **Risk cache:** Non-blocking read; miss ⇒ reject (determinism > liveness).

Accepted orders are immediately matched if crossing; otherwise they rest.

### 6.5 Priority & Tie-Breaking

- **Primary:** `TsNorm` (normalized time from `LatencyModel`).
- **Secondary:** `EnqSeq` (stamped at ingress; monotonic within the tick).
- **Cancel vs Fill in same tick:** Earlier `(TsNorm, EnqSeq)` wins—consistent with SPSC order.

### 6.6 Cold Start & Halts

- **Cold start:** Until first trade sets a reference price, `MARKET/IOC` are rejected. Only **in-band `LIMIT`** orders may rest.
- **Halts:** While halted, all orders reject with `MarketHalted` except policy-allowed admin cancels.

### 6.7 Clock/Coordinator Contracts

- **Registration:** `SymbolCoordinator` wires queues and registers a symbol with `SimulationClock` **before** its first tick.
- **Lifecycle:** Stop/evict requests take effect at the **next tick boundary**. A draining engine still emits its final `TickComplete` before stopping.

### 6.8 Observability (without perturbation)

- Per-tick counters (queue depth, rejects by reason, arena occupancy, best-price churn) recorded to an async diagnostics ring.
- Optional profiling flags (validation/match cycle counts) are boundary-flushed and never in the match loop.

### 6.9 “Done When” (acceptance for this section)

- Running two identical sessions (same inputs, config, and tick schedule) yields **byte-identical** event streams pre-`execution_id`.
- Every `tick(T)` for a symbol produces **exactly one** `TickComplete`.
- Backpressure behavior is test-covered: SPSC full ⇒ reject; ExecMgr overflow ⇒ symbol marks fatal, evicted at boundary.
- Cold-start behavior is enforced by tests: `MARKET/IOC` reject; first `LIMIT` trade establishes reference price.

## Matching & Book Updates (Master-Level)

`Whistle` executes matching **only inside `tick(T)`**, using **strict price-time priority** and FIFO per price level. The loop is allocation-free and bounded by config (`batch_max`), updating the book and emitting events deterministically.

### Priority model

- **Price first, then time.** Better price wins; ties break by `(ts_norm, enq_seq)` captured at admission.
- **FIFO within level.** Resting orders at the same price are served in arrival order; partials **retain** their original place.

### Order-type semantics

- **LIMIT:** Match while crossing; any remainder **rests** at its price (tail of the FIFO).
- **MARKET:** Match best prices until filled or book exhausted; **never rests**.
- **IOC:** Match immediately up to its limit; **cancel** any remainder.
- **POST-ONLY:** Must add liquidity at its submit price; if it would cross, it’s **rejected at admission** (no slide/price improvement).

### Self-match prevention (SMP)

- Default policy: **skip** own resting orders when aggressing (no auto-cancel, no self-fill).
- Alternative policies (configurable per symbol): **cancel resting** or **cancel aggressor**.
- Behavior is deterministic and recorded in events for replay.

### Book maintenance (what changes)

- **Level totals**: Updated after each fill/partial; coalesced for emission later in the tick.
- **Intrusive FIFO links**: Fully filled makers are unlinked in O(1); partials stay in place.
- **Top-of-book pointers**: `best_bid`/`best_ask` updated when a level becomes empty/non-empty.
- **Non-empty index**: Maintained to jump to the next price efficiently (no full scans).

### Cancels & races

- Cancels are part of the same `tick(T)` batch. If a cancel and a potential fill compete:
    - Earlier `(ts_norm, enq_seq)` **wins**.
    - Outcome is stable in replay and reflected in lifecycle events.

### Determinism & replay stance

- Matching order and outcomes depend **only** on: admitted order payloads, book state at `tick` entry, and fixed policies.
- Emitted events are buffered and later ordered **canonically** within the tick (Trades → BookDeltas → OrderLifecycle → TickComplete).
- Replaying the same inputs yields byte-identical event streams (pre global execution IDs).

### Interfaces touched

- **Reads/Writes:** `OrderBook` (levels, best pointers), arena entries (qty, links).
- **Outbound:** Trade records and coalesced book deltas staged for `ExecutionManager`.
- **Indexing:** OrderId→handle map updated on fills/cancels for O(1) maintenance.

### Observability & safety

- Counters for matches, partials, depth traversed, SMP skips, and book-churn per tick.
- No hot-path logging or allocation; diagnostics are buffered and emitted off the match loop.

---

## ExecutionManager & Event Pipeline

`ExecutionManager` is the **single intake** for all per-symbol events emitted by `Whistle`. It stamps (or validates) execution IDs, preserves the **canonical per-tick ordering**, and fans out to **Replay**, **Analytics**, and **UI** without perturbing engine latency or determinism.

### Role in the System

- **Ingest:** Consume `EngineEvent` batches from all `Whistle` instances via an MPSC queue.
- **Order:** Maintain canonical ordering **within each symbol/tick**: Trades → BookDeltas → OrderLifecycle → TickComplete.
- **Stamp IDs:** Assign **global execution IDs** (if centralized mode) or validate sharded IDs.
- **Fan-out:** Deliver a **lossless** stream to `ReplayEngine`, a **structured** stream to `AnalyticsEngine`, and an **event stream** to the Web UI (lossy allowed).
- **Boundary:** Align all work to tick boundaries; no mid-tick reordering of a symbol’s batch.

### Contracts & Guarantees

**Input (from Whistle)**

- Each `tick(symbol, T)` yields exactly one `TickComplete(T)`.
- Within a symbol/tick, events are already grouped and ordered: Trades → BookDeltas → OrderLifecycle → TickComplete.
- Trades/Lifecycle carry a **per-symbol, per-tick** `seq_in_tick` that is strictly increasing, gap-free from 0.

**Output (downstream)**

- **Replay stream:** **Lossless**, byte-stable per run. If the sink backpressures beyond policy, simulation halts (fatal).
- **Analytics stream:** Structured metrics/logs; allowed to drop under pressure without affecting determinism.
- **UI stream:** Real-time updates; lossy by policy.

**Never violates:**

- Per-symbol event order.
- One `TickComplete` per symbol per tick.
- Idempotency of global IDs (no duplicates, no rewrites once assigned).

### Execution ID Policy

Two supported modes:

| Mode | Behavior | When to use |
| --- | --- | --- |
| **Sharded (engine-local)** | `Whistle` stamps `exec_id = (tick << SHIFT) | local_trade_seq`. ExecMgr **validates** only. |
| **Centralized (global)** | ExecMgr assigns a **monotonic global ID** as events arrive. Deterministic merge order: `(tick, symbol_id, group_order, seq_in_tick)` where `group_order = Trades(0) < BookDeltas(1) < OrderLifecycle(2) < TickComplete(3)`. | If a single global sequence is required for external consumers. |

> Centralized mode never reorders within a symbol; it merges already-ordered symbol batches by a stable key so replays are deterministic.
> 

### Backpressure & Failure Policy

- **Ingress (Whistle → ExecMgr MPSC):** Must be sized so producers **never block**. If overflow is observed, policy is **fatal** (configuration error) after the current tick boundary is emitted.
- **Replay sink:** **Lossless**. On persistent backpressure, ExecMgr signals **fatal** and stops the simulation cleanly at the next boundary.
- **Analytics/UI sinks:** May degrade (drop/coalesce) but **must not** feed back into engine timing or event order.

### Batching & Flushing

- **Per-symbol batch:** Process symbol events atomically per tick; flush to sinks immediately after `TickComplete(symbol, T)`.
- **Global pacing:** Optional micro-batch window (e.g., sub-millisecond) to coalesce cross-symbol writes—**does not** cross tick or reorder a symbol.

### Schema & Compatibility

- Event families are versioned. Adding fields is **backward compatible** (reserved ranges); removing or changing semantics requires a **major** bump and explicit migration notes.
- Wire format: FlatBuffers or Cap’n Proto (implementation doc picks one). The master doc’s rule: **no schema churn without ADR**; WAL remains readable across minor versions.

### Idempotency & Replay

- Downstream writes are idempotent using `(symbol_id, tick, seq_in_tick, kind, ordinal)` or the assigned `exec_id`.
- Replaying the same inputs (events as emitted by Whistle) yields **byte-identical** Replay output and identical Analytics aggregates.

### Observability (without perturbation)

- Counters: events/sec by family, queue depths, drops by sink, fatal reasons.
- Timers: ingest-to-flush latency per family, tick batch processing time.
- All metrics are emitted **off-path**; no formatting or syscalls in the ingest loop.

### Configuration Knobs

- `mode = {sharded, centralized}`
- `replay_sink_policy = fatal|block` (we default to **fatal**; block is for dev only)
- `ui_sink_policy = drop_on_backpressure` (default true)
- `micro_batch_window_us` (0 = disabled)
- `mpsc_capacity` (sized for peak T x symbols)

### Failure Modes & Expected Behavior

| Scenario | Behavior | Notes |
| --- | --- | --- |
| Ingress MPSC overflow | **Fatal after boundary** | Mis-sized queue; isolate & stop |
| Replay sink backpressure | **Fatal**, emit diagnostic | Lossless guarantee preserved |
| Analytics/UI sink backpressure | Drop/coalesce, never blocks | Determinism unaffected |
| Invalid `seq_in_tick` from Whistle | Flag symbol as **faulted**, stop after boundary | Invariant breach upstream |
| Duplicate exec_id (centralized) | Prevent assignment; log and fault | Indicates non-deterministic merge or bug |

### Invariants

1. For each `(symbol, tick)`, emitted order is **Trades → BookDeltas → OrderLifecycle → TickComplete**.
2. No cross-symbol merge can reorder a symbol’s internal sequence.
3. Replay stream is complete and in the same order the engines produced.
4. If centralized, `exec_id` is strictly increasing over the whole run.

---

## AccountService

`AccountService` is the **source of truth** for balances, positions, and limits. It enables **deterministic admission** (read-only risk checks on the hot path) and **authoritative settlement** (applying fills to cash/positions). It never blocks `Whistle`; it operates on **snapshotted, tick-consistent state** and reconciles changes at tick boundaries.

### Role in the System

- **Balances & Positions**
    - Track per-account **cash**, **per-symbol position** (qty, avg entry), and **P&L** (realized/unrealized; informational).
- **Risk & Eligibility**
    - Enforce configurable limits before admission: **cash sufficiency**, **notional / quantity caps**, **per-symbol exposure caps**, **shorting permissions** (off by default).
- **Settlement**
    - Apply trades (debits/credits) and update positions **exactly once** based on `ExecutionManager`’s event stream.
- **Snapshots & Replay**
    - Publish a **read-only risk cache** to `Whistle` (epoched), persist authoritative state, and restore deterministically.

### Design Principles

1. **Non-blocking admission.** `Whistle` only does **read** lookups against a local, epoch-tagged risk cache. Cache miss or stale epoch ⇒ **Reject(RiskUnavailable)**.
2. **Deterministic timing.** Risk state visible to `Whistle` changes **only** at tick boundaries (cache swap), never mid-tick.
3. **Exactly-once settlement.** Trades are applied idempotently keyed by `(symbol, tick, seq_in_tick)` (or global `execution_id` if present).
4. **Conservative reservation.** Buys reserve notional on **admission**; sells reserve **inventory** on admission. Reservations release or convert to settled deltas on fill/cancel.
5. **Clear failure modes.** No “soft” declines—every denial is explicit (`InsufficientFunds`, `ExposureExceeded`, `RiskUnavailable`, `ShortingDisabled`, etc.).

### What `Whistle` Reads vs. What AccountService Writes

| Contract | Direction | Purpose |
| --- | --- | --- |
| `RiskCache{epoch, per-account caps, available_cash, available_qty(symbol), outstanding_notional}` | AS → Whistle | Admission checks (pure reads, NUMA-local) |
| `EngineEvent::Trade / OrderLifecycle` | Whistle → ExecMgr → AS | Authoritative settlement & reservation release |
| `Snapshot{balances, positions, reservations, epoch}` | AS ↔ Persistence | Warm start & replay |
| `NewEpoch{epoch_id, cache_blob}` | AS → Whistle (boundary) | Swap-in next tick’s read-only cache |

### Reservation & Exposure Model (admission-time)

- **Buy LIMIT/POST-ONLY/IOC:** reserve `qty * limit_price`.
- **Buy MARKET:** reserve `qty * ref_price_with_band` (configurable: last trade + band or best_ask snapshot).
- **Sell LIMIT/POST-ONLY/IOC:** reserve `qty` of **inventory**.
- **Sell MARKET:** reserve `qty` of inventory as above (no naked sells unless **Shorting=true** with borrow cap).
- **Caps:** configurable per account: `max_per_order_qty`, `max_open_notional`, `max_per_symbol_exposure`, `max_total_exposure`.

> Admission uses available = balance − active_reservations (for cash) and position_available − active_reservations (for inventory). No blocking, no RPC.
> 

### Settlement Model (fill-time)

- On **trade**:
    - **Buyer:** `cash -= price*qty`, `position[symbol] += qty`, `avg_entry` updated by standard weighted-average.
    - **Seller:** `cash += price*qty`, `position[symbol] -= qty` (short if allowed).
    - **Reservations:** reduced by the exact filled amount; remainders persist until order completes/cancels.
    - **Realized P&L:** updated on closing trades; **unrealized** derived from last trade (for analytics; no admission impact).
- On **cancel / IOC remainder / MARKET remainder**: release corresponding reservations immediately upon lifecycle event.

### Shorting Policy (config)

- **Default:** **disabled** (sell qty must be ≤ on-hand position).
- **If enabled:** per-account **borrow cap** (qty or notional), with optional **borrow fee** accrual off-path. Admission rejects if a sell would breach borrow cap.

### Determinism & Replay

- **Epoching:** Each published risk cache carries an `epoch` id. `Whistle` reads only the current epoch during `tick(T)`. Next epoch becomes visible **after `TickComplete(T)`**.
- **Idempotency:** Settlement applies each trade once keyed by tick+seq (or global ID). Replays re-apply safely and match persisted totals.
- **Warm start:** Snapshot includes balances, positions, outstanding reservations, and `epoch`. After restore, the next cache publish resumes at `resume_tick`.

### Configuration Knobs

- **Admission:**
    - `max_per_order_qty`, `max_open_orders`, `max_open_notional`, `max_per_symbol_exposure`, `allow_shorting`, `borrow_cap_{qty|notional}`.
    - MARKET reservation basis: `BestOf{best_quote, last_trade}`, plus `band_padding_bp`.
- **Settlement:**
    - Realized P&L method (`FIFO` default for simulation clarity; `AVG` optional), rounding mode (banker’s or floor).
- **Publishing:**
    - Cache publish cadence (per tick, or every N ticks), and **atomic swap** mechanism.
- **Replay:**
    - Mode A: **risk snapshot required** (preferred, smaller WAL).
    - Mode B: record **admission verdicts** in WAL and bypass live checks on replay.

### Failure Modes & Expected Behavior

| Scenario | Admission | Settlement | Notes |
| --- | --- | --- | --- |
| Cache miss/stale epoch | **Reject(RiskUnavailable)** | — | Non-blocking, deterministic |
| Insufficient funds/inventory | **Reject(InsufficientFunds)** | — | Uses reservation-inclusive available |
| Exposure cap breach | **Reject(ExposureExceeded)** | — | Per-symbol or global caps |
| Shorting disabled | **Reject(ShortingDisabled)** | — | Unless policy enables with cap |
| Duplicate trade during replay | — | **Ignored (idempotent)** | Keyed by tick/seq or exec_id |
| Out-of-order events | — | Applied by **canonical order** (tick, seq) | ExecMgr preserves order |

### Interfaces (high-level)

- **To `Whistle`**: `RiskCache` (immutable during tick), versioned; memory-local for speed.
- **From `ExecutionManager`**: Ordered event stream (trades, lifecycle).
- **To `PersistenceLayer`**: Periodic state snapshots + streaming deltas for audit.
- **To `AnalyticsEngine`**: P&L snapshots, exposure metrics, admission reject stats.

### Observability (no hot-path perturbation)

- Counters: rejects by reason, reservation notional by account/symbol, cache publish latency, idempotent trade drops.
- Gauges: total open notional, free cash per account, aggregate long/short exposure.
- All published **after** tick; no logging in `Whistle`’s match loop.

### Invariants

1. `available_cash = cash − Σ(active_cash_reservations) ≥ 0`
2. `available_qty[symbol] = position_qty − Σ(active_qty_reservations[symbol]) ≥ 0`
3. Sum of reservations equals the sum of admissible open quantities * prices under current policy.
4. Applying the same `Trade` twice does not change balances/positions (idempotent).
5. Snapshot → restore → replay produces **identical** balances/positions/P&L.

---

## Persistence & Replay (WAL + Snapshots)

`PersistenceLayer` gives us **lossless history** and **bitwise replay** without touching the hot path. It is split into two cooperating parts:

- **WAL (Write-Ahead Log):** append-only, schema-versioned record of inputs and outputs.
- **Snapshots:** point-in-time captures of per-symbol state taken **only at tick boundaries**.

Together they allow **resume at `T+1`**, deterministic audits, and performance forensics—without perturbing `Whistle`.

### Responsibilities

- **Record** all inputs (orders/cancels/config) and all engine outputs (event families) in a versioned WAL.
- **Rotate & compress** segments safely; never block engines.
- **Snapshot** each symbol after `TickComplete(T)` per policy.
- **Restore** a symbol to an identical state and **replay** forward to reproduce bytes.
- **Verify** integrity with cheap rolling hashes (events/book).

### What We Record (authoritative)

**Inputs (immutable):**

- Enqueued orders/cancels with `symbol, tick_enqueued, ts_norm, enq_seq, account, side, type, price?, qty`.
- Session config (price bands, tick size, policies, `batch_max`, etc).
- Optional: **admission verdicts** if we do not snapshot risk (see Determinism Modes).

**Outputs (immutable, from ExecutionManager):**

- Canonical event families per symbol/tick: **Trades → BookDeltas → OrderLifecycle → TickComplete**.
- Per-tick **event hash** and **book hash** (post-tick).

**Never recorded in hot path:** debug logs, UI-only artifacts, formatting.

### Snapshot Boundaries & Contents

**When:** Only **after** `TickComplete(T)`; resume at `T+1`. No mid-tick snapshots.

**What per symbol:**

- Order arena (live orders) + free list.
- OrderIndex (including tombstones).
- Book levels (head/tail/total), non-empty bitset, best bid/ask.
- Reference price/last trade, deterministic counters.
- Config hash and risk epoch (if using risk snapshot mode).

**Why:** Boundary snapshots keep hot-path logic simple and guarantee replay alignment.

### Determinism Modes (AccountService)

To keep admission deterministic you must choose one:

1. **Risk snapshot mode (preferred):** persist the **read-only risk cache** at snapshot time (limits + utilization per account). Replay loads it verbatim.
2. **Admission-verdict mode:** record an **Accepted/Rejected(+code)** verdict for each order in the WAL and bypass live risk on replay.

If neither is provided, the run is **not replayable** and will be rejected in replay mode.

### File Layout (per session)

- `metadata.json` — session id, schema versions, config hash, start/end ticks.
- `wal/` — rotated segments (e.g., time- or size-based), compressed.
- `snapshots/<symbol>/` — most recent point-in-time image per symbol (policy-driven cadence).
- `metrics/` — perf aggregates (latency histograms, arena occupancy), optional.

Rotation is atomic (write temp → fsync → rename). Compression is typically Zstandard. Segment headers carry **schema + CRC**.

### Replay Procedure (symbol-local then global)

1. **Load snapshot** for the symbol (or start cold if none for early ticks).
2. **Resume at `T+1`**; drive the same tick schedule.
3. **Feed inputs** from WAL into the symbol’s SPSC in original enqueue order (`tick_enqueued, enq_seq`).
4. **Capture events** from `ExecutionManager` and compare **byte-for-byte** (or hash-for-hash) with recorded outputs.
5. **Cross-check book hash** after each `TickComplete`.

At system scope, repeat for all symbols; centralized Execution IDs (if enabled) reproduce identically because merge order is defined.

### Integrity & Guarantees

- **Bitwise replay:** same inputs + same config + same tick schedule ⇒ identical outputs.
- **Hash checks:** per-tick event hash and post-tick book hash detect drift instantly.
- **Schema evolution:** all records are versioned. We allow **additive** changes; breaking changes require an ADR, a new major version, and migration tooling.

### Backpressure & Failure Policy

- **Replay sink must be lossless.** If it backpressures beyond policy, the simulation marks **fatal** and stops **after the current boundary** (never mid-tick).
- **WAL rotation failure:** fail fast, surface a single fatal diagnostic, and halt at boundary.
- **Disk full:** same as above; engines don’t block—`ExecutionManager` elevates to fatal at boundary.

### Observability (non-perturbing)

- Counters: WAL bytes written, segment rotations, snapshot time, compression ratio.
- Timers: ingest→persist latency per family, snapshot serialization time.
- All metrics emitted off-path; no formatting or syscalls in `Whistle`.

### Configuration Knobs

- `wal_segment_max_mb`, `wal_rotate_interval_s`
- `compression = zstd | none`, `compression_level`
- `snapshot_every_n_ticks` (0 = disabled), `snapshot_retention` (count/time)
- `determinism_mode = risk_snapshot | admission_verdict`
- `replay_sink_policy = fatal|block` (default **fatal**)
- `verify_hashes = on|off` (on in CI)

### Invariants (must hold)

1. Snapshots exist **only** at tick boundaries and load to a consistent state.
2. Every `(symbol, tick)` in WAL has exactly one `TickComplete`.
3. Event order inside a symbol/tick is canonical; cross-symbol merge never reorders a symbol batch.
4. Book hashes computed from replay match recorded hashes post-tick.

---

## SymbolCoordinator & Placement

`SymbolCoordinator` is the system’s **orchestrator** for per-symbol engines. It owns when engines exist, where they run, and how they’re wired. Its job is to keep thousands of independent `Whistle` instances **placed, pinned, observable, and fault-isolated**—without ever touching the hot path.

### Role (at a glance)

- **Lifecycle:** create, register, pause, evict, and restore engines.
- **Placement:** choose thread/core/NUMA for each engine; keep it there.
- **Wiring:** allocate and connect per-symbol queues (SPSC in, MPSC out).
- **Backpressure & faults:** detect hot-path pressure via downstream signals; enforce boundary-safe shutdowns.
- **Capacity control:** bound memory/CPU per engine and across the fleet.
- **Handoff to Clock:** ensure engines are added/removed from the `SimulationClock` participant set only at tick boundaries.

---

### Responsibilities

- **Admission of symbols**
    - Cold start a symbol on first order or on explicit “prewarm” command.
    - Deny admits if capacity policy would be violated (clear reason surfaced upstream).
- **Placement policy**
    - Pin each engine to a **stable OS thread**; prefer NUMA-local memory.
    - **Hot symbols** (top N by msg/s) get dedicated threads; **warm/cold symbols** share a worker pool with bounded concurrency.
    - Optional **isolation pools** (e.g., “premier players”) for predictable latency.
- **Lifecycle states**
    - `Idle → Booting → Running → Draining → Stopped` (Faulted side path).
    - All transitions happen **only** at tick boundaries; no mid-tick churn.
- **Queue wiring**
    - Create per-symbol **SPSC<OrderMsg>** (router → engine).
    - Register engine’s **MPSC<EngineEvent>** with `ExecutionManager`.
    - Size queues from config; never resize at runtime.
- **Eviction & reclamation**
    - Idle timeout or memory pressure ⇒ request stop at next boundary.
    - On eviction: drain to final `TickComplete`, close queues, recycle buffers.
- **Fault isolation**
    - If an engine raises a **fatal flag** (e.g., replay sink policy), quarantine and evict at boundary without affecting others.
    - Optionally auto-restart from most recent snapshot (policy-gated).

---

### Placement & Scheduling (policy overview)

| Class | When applied | Execution | Goal |
| --- | --- | --- | --- |
| **Dedicated** | Top-K hot symbols | One engine ↔ one pinned thread | Minimum jitter, cache locality |
| **Pooled** | Long tail | Fixed-size async pool (work stealing off) | Fairness across many symbols |
| **Quarantine** | Faulted | Isolated thread, limited runtime | Forensics, safe teardown |

**NUMA**: allocate engine memory on the node of its thread; prefer keeping router/engine/ExecMgr on the same node for that symbol shard.

---

### Interaction with SimulationClock

- **Registration:** `SymbolCoordinator` registers an engine **before** the first tick after `boot()` succeeds.
- **Tick cadence:** Clock drives one `tick(T)` per registered engine; the coordinator never calls engine methods itself in the hot path.
- **Unregistration:** On eviction, remove from the participant set **after** the engine emits `TickComplete(T)`.

---

### Capacity & Resource Model

- **Per-engine caps:** arena size, index size, outbound queue depth.
- **Fleet caps:** max running engines, max dedicated threads per NUMA node, total memory envelope.
- **Admission decisions:** if adding an engine would exceed any cap → reject symbol start or spill it to pooled tier (configurable).
- **Hysteresis:** promote/demote between Dedicated ↔ Pooled using smoothed msg/s and CPU time.

---

### Backpressure & Fault Policy

- **Inbound SPSC full:** upstream rejects with `Reject(Backpressure)`; coordinator records metric (no blocking).
- **Outbound MPSC failure (lossless path):** engine marks **fatal**; coordinator evicts at boundary (policy: `fatal` or `block`). Default: **fatal**.
- **Crash/Invariant breach:** mark symbol Faulted, stop intake, drain if possible, snapshot for forensics, then evict.

---

### Cold/Warm Start

- **Cold start:** new arena/book; best pointers unset; MARKET/IOC policy follows cold-start rules.
- **Warm start:** load snapshot; verify invariants; resume at `T+1`. Failsafe: if snapshot invalid → refuse warm boot and fall back to cold only if policy allows (never silently).

---

### Observability (coordinator scope)

- **Per-symbol:** state, placement tier, thread id/core, NUMA node, queue depths, idle time, eviction reason.
- **Fleet:** engines by tier, memory in use, snapshot cadence & duration, hot-symbol list.
- **Events:** Start/Stop/Evict/Restart with reason codes (schema-stable).

All counters/timers are emitted off the hot path; no formatting in `Whistle`.

---

### Configuration Knobs

- `max_running_engines`, `max_dedicated_threads_per_numa`
- `dedicated_threshold_msgs_per_sec`, `demote_threshold`
- `engine_idle_timeout_ticks`
- `spsc_depth`, `mpsc_depth`
- `eviction_policy = idle|memory|manual`
- `fault_policy = quarantine_then_evict | immediate_evict`
- `warm_restart = on|off`
- `numa_policy = prefer_local | ignore`

---

### Invariants

1. **One engine per symbol**, pinned; no migration during lifetime.
2. **No lifecycle changes mid-tick.** Start/stop only at boundaries.
3. **Exactly one** `TickComplete(T)` per registered engine per tick.
4. Queue sizes are fixed; any overflow is handled via explicit policy, never blocking `Whistle`.
5. Warm resumes load a state that passes structural checks before rejoining the clock.

---

## Latency Model & Fairness

**Goal:** make timing **deterministic and comparable** across humans and bots without hiding edge or introducing randomness. Latency is simulated, **stamped once** before enqueue, and never altered inside `Whistle`.

### What it does

- Converts wall-clock arrival into a **normalized timestamp (`ts_norm`)** using a per-actor **latency profile**.
- Ensures strict **price-time** fairness: tie-break = `(ts_norm, enq_seq)`.
- Lets you model strategy edges (e.g., “fast market maker”) without violating determinism.

### Inputs & profiles

- **Actor profile:** `{ base_us, jitter_us=0, distribution=None, clamp_min_us, clamp_max_us }`
    - Default: `jitter_us=0` (no randomness). If jitter is enabled for experiments, it must be seeded and **held constant** for replay builds.
- **System knobs:** `max_normalized_skew`, `drop_if_skew_exceeds` (optional hard guard).

### Normalization pipeline (OrderGateway → OrderRouter)

1. Read wall-clock arrival (ingress).
2. Compute simulated latency `L` from actor profile.
3. Set `ts_norm = logical_tick_start_time + L`.
4. Stamp **once** onto the order; attach **monotonic `enq_seq`** before SPSC enqueue.

### Invariants (non-negotiable)

1. `ts_norm` is **write-once** upstream; `Whistle` treats it as data, not a signal.
2. Gate fairness by `(ts_norm, enq_seq)` only; no hidden clocks.
3. Profiles are **static within a run** (can change only at tick boundaries with a control event).
4. If a profile would produce negative/NaN/overflow: reject admission with explicit reason.
5. Replays that reapply the same profiles yield **byte-identical** results.

### Observability (off hot path)

- Per-actor: p50/p95/p99 normalized latency, drops by reason.
- Per-symbol: inter-arrival variance, queue depth at tick start.
- Fleet: distribution of profiles; slowest actors.

### Configuration (safe defaults)

- `default_profile = base_us=500, jitter_us=0`
- `max_normalized_skew = 5_000us` (example)
- `drop_if_skew_exceeds = true` → `Reject(LatencyOutOfBounds)`

---

## StrategyEngine & Bots

**Goal:** provide realistic, programmable market participants that compete under the **same rules and latencies** as humans—without compromising determinism, safety, or isolation.

### Role in the system

- Runs **user-supplied or built-in** strategies.
- Consumes **public** data only (top of book, trades, deltas, session metadata, and *informative* real-world signals).
- Submits actions via the **same `OrderGateway`** as any actor; no privileged path, no special fees/priority.

### Contract (what a bot can see & do)

**Inputs (read-only):**

- Book snapshots/deltas (side, price, level qty).
- Trades (price, qty, aggressor side).
- Own order acks/fills/cancels/rejects.
- Time: logical tick `T`; no wall-clock.
- Optional external signals (injuries, rankings, projections) flagged **informative-only**.

**Actions (write):**

`SubmitLimit`, `SubmitMarket`, `SubmitIoc`, `SubmitPostOnly`, `Cancel(order_id)` — identical message shapes to human flow. No amendments; cancel+replace only.

**Fairness:**

Actions are stamped with that bot’s **latency profile** and ordered by `(ts_norm, enq_seq)` like everyone else.

---

### Runtime model & sandboxing

| Aspect | Decision |
| --- | --- |
| Execution model | **Step-per-tick**: `on_tick(T, inputs) -> actions[]` |
| Default language | **Rust** SDK (native), plus **WASM** plug-ins for extensibility |
| Optional scripting | **Python** workers behind a deterministic RPC shim (no network, seeded RNG) |
| Isolation | WASM sandbox or subprocess; no file/network; memory cap; CPU time slice per tick |
| Determinism | RNG seeded from `(session_id, symbol_id, bot_id)`; seeds logged in WAL |
| Failure policy | Bot panic/timeout ⇒ drop actions for this tick, emit diagnostic; engine unaffected |

**Quotas (per bot, per tick):**

- **Time budget:** e.g., 100–500 µs wall per tick (configurable).
- **Action budget:** max N submits + M cancels (admission limits still apply at the gateway).
- **Outstanding orders:** per-symbol cap (defends against book bloat).

---

### Data & policy boundaries

- **No private state leaks:** bots cannot see other actors’ identities or hidden queues.
- **No real-world P&L coupling:** external data is **advisory**, never drives prices directly.
- **Self-match prevention:** enforced by Whistle; bots can’t opt out.
- **Halt/limits:** if a symbol is paused or price-banded, bot actions are rejected like any other.

---

### Bot lifecycle

1. **Register** bot spec: `id`, `symbol(s)`, latency profile, quotas, seed.
2. **Activate** at a tick boundary; StrategyEngine subscribes to required feeds.
3. **Run step** each `T`: collect inputs → call bot → validate actions → forward to `OrderGateway`.
4. **Deactivate/Evict** at boundary; outstanding orders cancelled by policy or left resting if configured.

---

### Observability (off hot path)

- Per-bot: step time p50/p95/p99, actions per tick, rejects by reason, hit rate (fills/submits), inventory/PNL (if enabled in analytics).
- Per-symbol: bot market share, quote stability, spread contribution.
- Audit: logged **seed**, version hash of bot binary/WASM, and profile.

---

### Configuration (safe defaults)

- `default_latency_profile = base_us=500, jitter=0`
- `max_actions_per_tick = 64`, `max_outstanding = 256`
- `cpu_budget_us = 200`, `mem_limit_mb = 64`
- `rng_mode = deterministic(seed = H(session_id, symbol_id, bot_id))`
- `allow_python = false` (opt-in); default **WASM** or Rust.

---

### Interfaces (summary)

**Bot SDK (conceptual):**

- `fn init(ctx: InitCtx) -> BotState`
- `fn on_tick(state: &mut BotState, view: MarketView<'_>) -> SmallVec<Action>`
- `fn on_lifecycle(state, ev: LifecycleEvent)` (optional)

**StrategyEngine → OrderGateway:**

Same `InboundMsg` types as human flow; stamped with bot actor id and latency profile before SPSC enqueue.

---

### Determinism & replay

- Given the same **inputs, seed, config, and tick schedule**, bot outputs are **byte-identical**.
- Bot versions are content-addressed; the exact artifact hash is stored in WAL/session metadata.
- Any non-deterministic feature (e.g., jitter) is **disallowed** in replay mode.

---

## Core Concurrency & Execution Model

Order flow and matching are optimized through **structured concurrency** and **buffered isolation per symbol**.

### Per-Symbol Order Buffers

Each active symbol (`Whistle` instance) maintains its own **lock-free, single-producer-single-consumer (SPSC) queue** for inbound order traffic. This ensures:

- **Thread safety**: Producers and consumers never share mutable state
- **Deterministic ordering**: Orders are matched in exact price-time priority
- **Non-blocking routing**: `OrderRouter` pushes orders without waiting for a match cycle

### Threading Model

- **Hot symbols** (frequently traded players) are assigned **dedicated threads** for minimal context switching and maximum CPU cache locality
- **Less active symbols** are managed by an **async task pool**, ideal for bursty or occasional activity
- CPU affinity can be pinned for top-tier markets to maximize throughput

```jsx
		    SimulationClock triggers Tick (T)
		
		            ┌────────────────┐
		            │ Tick Start (T) │
		            └────────────────┘
		                     │
		       ┌─────────────┼──────────────┐
		       ▼             ▼              ▼
		  [Whistle]     [LatencyModel]   [StrategyEngine]
		(order matching)   (timestamp adj)   (bot logic)
		                     ▼
		            ┌────────────────┐
		            │ Tick Complete  │
		            └────────────────┘
		                     │
		                     ▼
		            [ExecutionManager]
		      (flushes all buffered events)
		                     │
		                     ▼
		            [AnalyticsEngine + ReplayEngine]
		       (emit logs, metrics, WALs, snapshots)
```

### Cold Start Behavior

If an order arrives for a player whose `Whistle` engine is inactive:

- `SymbolCoordinator` spins up the engine in a background thread
- The order is held until the engine’s buffer becomes available (typically sub-ms)
- Engines are evicted after configurable inactivity windows to free up system resources

---

## Market Integrity and Fairness

Each component contributes to ensuring orderly, fair, and stable simulated markets.

| Mechanism | Enforced By | Description |
| --- | --- | --- |
| Price Band Limits | `Whistle` | Orders moving price beyond ±X% of last trade are rejected |
| Max Order Size | `OrderGateway`, `Whistle` | Capped per submission and per account exposure |
| Order Rate Throttling | `OrderGateway` | Token-bucket per trader to limit flood risk |
| Minimum Tick Size | `OrderGateway` | Ensures discrete, clean pricing per asset class |
| Circuit Breakers | `SymbolCoordinator` | Pauses symbols with extreme price velocity |
| Fair Queue Handling | `LatencyModel`, `SimulationClock` | Normalized timestamps and configurable delay logic |
| Exposure Tracking | `AccountService` | Per-user caps by symbol or total net exposure |
| Quote Lifespan Rules | `Whistle` | Optional time-in-force enforcement or cancel delay penalties |

---

## Testing Philosophy

A core design principle of the Waiver Exchange is that every testable unit **must be tested, deterministically and repeatedly**. Testing is not an afterthought — it is embedded in every component’s interface, lifecycle, and data flow.

We support multiple levels of testing:

| **Type** | **Purpose** |
| --- | --- |
| Unit Tests | Validate core algorithms and decision logic (e.g., matching, risk checks) |
| Integration Tests | Ensure component interoperability (e.g., `Whistle` + `AccountService`) |
| Property-Based Tests | Use randomized inputs to validate invariants and edge cases (via `proptest`) |
| Simulation Tests | Run full market flows with bots and variable latency |
| Replay Tests | Guarantee full determinism under replay of input logs |
| Performance Tests | Benchmark latency, throughput, and resource usage |
| End-to-End Tests | From frontend/API → matching → account system → analytics |

All components support isolated and replayable evaluation. Tests are tracked and organized in a dedicated structure (`/tests`) and are run automatically via CI.

Testing is built on:

- **Rust**'s `cargo test`, `proptest`, `criterion` for most core logic
- **C++** via `Catch2` or `GoogleTest` for auxiliary or interop layers
- **Full snapshot replay** testing for historical or regression analysis

---

## Implementation Guardrails

Critical violations that break system invariants and must be prevented during implementation.

**Canonical event order** — The per-tick emission sequence Trades → BookDeltas → OrderLifecycle → TickComplete is non-negotiable. Reordering breaks replay determinism.

**Tick-bounded execution** — All state changes must occur inside `tick(T)`. Mid-tick mutations violate the execution model and break determinism.

**Hot-path constraints** — No heap allocation, locks, or syscalls in match loops. Use preallocated buffers and async diagnostics.

**Backpressure policies** — SPSC full ⇒ Reject(Backpressure); WAL overflow ⇒ fatal; UI/analytics may drop by policy. Never block on overflow.

**Determinism requirements** — Same inputs + config + tick schedule = byte-identical outputs. No wall-clock time or random behavior in hot paths.

**Price-time fairness** — Priority key = `(ts_norm, enq_seq)`. Partial fills retain original priority. Self-match prevention skips same-account orders.

**Cold-start rules** — MARKET/IOC rejected until first trade establishes reference price. Only in-band LIMIT orders accepted.

**Sequence numbering** — `seq_in_tick` increments for each Trade and OrderLifecycle event, starting at 0 each tick. No gaps allowed.

**POST-ONLY validation** — Reject orders that would cross the spread at submit price. No slide, no price improvement.

**Invariant preservation** — All non-negotiable rules must be enforced. Violations break correctness, performance, or replayability.

Use compile-time checks, debug assertions, and property tests to catch violations early. Code reviews must verify invariant compliance.

---

## Project Goals

| **Category** | **Goal** |
| --- | --- |
| Performance | < 2 μs match latency per order |
| Determinism | Every run with same inputs = same output |
| Throughput | 100k + orders/sec simulated across symbols |
| Extensibility | Easy to add new symbols, players, or bots |
| Observability | Fully loggable + visualizable with zero overhead |
| Testability | Unit tests, property-based tests, replay integration |
| Modularity | Clean, typed interfaces; one responsibility per component |
| Concurrency | Lock-free buffers + safe, isolated execution per engine |
| Scalability | Dynamic symbol activation and memory-efficient engine management |