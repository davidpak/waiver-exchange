## 1. Overview

`Whistle` is the high-performance, deterministic, **centralized order** matching engine at the core of The Waiver Exchange. It is responsible for processing and matching all order flow within a single player market (symbol), executing trades based on **strict price-time priority**, and enforcing key market rules that preserve fairness and stability.

Each symbol in the system (e.g., a fantasy football player) is backed by its **own dedicated instance of** `Whistle`. This ensures that order matching remains fully isolated per asset, allowing for parallelism, predictable performance, and fault containment. Each instance of `Whistle` is responsible for:

- **Maintaining the order book** for that symbol (bid/ask/levels)
- **Matching incoming orders** using strict price-time priority
- **Enforcing trade rules** (e.g., tick size, order types, exposure controls)
- **Generating execution reports** for downstream consumers (UI, bots, logic)

It is optimized for **low-latency, high-throughput, and replayable simulation,** and model key princes from modern electronic trading systems.

### Role in the System

In the broader architecture, `Whistle` is a leaf-level, per-symbol execution core.

Orders arrive into the system via the `OrderGateway`, are routed by the `OrderRouter` to the appropriate `Whitle` instance. Each maintains a price-ordered order book, and emits execution events via the `ExecutionManager`.

Each symbol has exactly one `Whistle` instance active at a time, managed by the **`SymbolCoordinator` .** These intances are **isolated**, making them safe to scale across threads and cores with no global locks or shared state.

### Core Responsibilities

- Maintain an accurate bid/ask order book sorted by price and time
- Match incoming orders against the book using price-time priority
- Enforce market rules like price bands, tick sizes, and quote integrity
- Emit trade events and order state transitions (filled, canceled, rejected)
- Interact cleanly with other core components:
    - Accept new orders from the `OrderRouter`
    - Emit executions to the `ExecutionManager`
    - Query/account for trader constraints from the `AccountService`
    - Operate on ticks from the `SimulationClock`
    - Respect synthetic latency rules from the `LatencyModel`

### Out of Scope

`Whistle` does not own the following responsibilities:

- Networking or API semantics (OrderGateway does).
- Multi‑symbol coordination or thread placement (SymbolCoordinator).
- Fanout ordering across symbols, persistence, or UI transport (ExecutionManager/ReplayEngine/WebUI).
- Account balance mutation (AccountService).
- Real‑world data ingestion (DataIngestion/MarketDataViewer).

### Invariants (system-level, non-negotiable)

1. **One engine per symbol.** It never migrates threads during its lifetime.
2. **Tick-bounded execution.** All matching occurs inside `tick(now)` . Every output is attributable to a tick.
3. **Price-time fairness.** Priority key = `(normalized_timestamp, enqueue_sequence)` .
4. **POST-ONLY never crosses.** If it would remove liquidity at submit price → reject (no slide, no price improvement).
5. **No heap in the hot path.** Preallocated arenas/queues; any growth only at tick **boundaries** (if enabled).
6. **Canonical event order per symbol & tick: Trades → BookDeltas → OrderLifecycle → TickComplete.**
7. **Deterministic IDs. `exeuction_id = (tick << SHIFT) | local_seq` (**layout documented; resumes across snapshot).
8. **Backpressure does not block.**
    1. Inbound SPSC full → **reject** (router sees `Reject(Backpressure)` ).
    2. Replay sink overflow → **fatal** (simulation halts); UI/analytics may be lossy by policy.

### Design Constraints

| Constraint | Implication |
| --- | --- |
| **Determinism** | Identical inputs must produce identical outputs, enabling replay and testability |
| **Tick-bounded execution** | All matching occurs only insde `tick(now)` ; no state changes between ticks |
| **Strict price-time priority** | Priority key is `(normalized_timestamp, enqueue_sequence)` : partials retain original priority. |
| **POST-ONLY never crosses** | If entry would remove liquidity at submit price, reject (no slide / no price improvement) |
| **Low latency** | Decision p50 ≤ 1.0 μs, p99 ≤ 3.0 μs (dequeue → decision, no I/O) |
| **Isolation** | No global locks or shared mutable state; engine never migrates threads during its lifetime. |
| **Bounded queues, non-blocking backpressure** | SPSC inbound and MPSC outbound are fixed-capacity; inbound full ⇒ reject; Replay egress overflow ⇒ fatal; UI/Analytics may drop. |
| **No hot-path allocation** | Orders/levels/events are preallocated; any growth happens only at tick boundaries (if enabled) |
| **No hot-path syscalls/formatting** | Logging/metrics are async via diagnostic rings; never in the match loop. |
| **Cache-locality aware layout** | Flat price ladder + arena + bitset; structs packed/aligned to minimize pointer chasing and false sharing. |
| **Thread/NUMA affinity enforced** | Each engine is pinned; memory allocated on the same NUMA node as the thread. |
| **Canonical per‑tick event order** | Outbound events per symbol, per tick: **Trades → BookDeltas → OrderLifecycle → TickComplete**. |
| **Deterministic ID layout** | `exeuction_id = (tick << SHIFT)` |
| **Cold‑start reference price** | Bands reference `snapshot.last_trade`; if absent, reject MARKET/IOC and accept only in‑band LIMITs until first trade. |
| **Schema/version stability** | Snapshot/WAL/event schemas are versioned; changes are explicit and backward‑compatible or gated. |
| **Scalability** | Thousands of active symbols with predictable latency; per‑symbol memory ≈ ≤ ~2 MB typical. |
| **Robustness** | Invalid inputs reject with explicit reason; engine faults are isolated and handled at tick boundaries. |
| **Observability without perturbation** | Metrics/counters emitted asynchronously; enabling observability does not affect ordering or latency. |

By isolating one matching engine per player and relying on per-thread execution and buffered ingestion, `Whistle` delivers both speed and clarity — enabling a true simulation-grade trading environment with realistic microstructure behavior.

To meet low-latency goals, `Whistle` avoids dynamic memory allocation, system calls, or locks during runtime operation. It relies on preallocated buffers, aligned memory structures, and strict thread affinity. These design rules are inspired by ultralow-latency financial systems and are enforced at compile time, initialization, or via runtime profiling hooks.

---

## 2 Functional Requirements

### 2.1 Order Acceptance & Message Shape

- **Ingress:** Per‑symbol **SPSC** ring (single producer: `OrderRouter`; single consumer: Whistle).
- **Required fields:**
    
    `order_id (u64)`, `account_id (u64)`, `side (Buy|Sell)`, `type (LIMIT|MARKET|IOC|POST_ONLY)`, `price (Option<Price>)`, `quantity (u32)`, `timestamp_normalized (u64)`, `metadata (optional)`.
    
- **No heap alloc** on accept path or match path.
- **Pre‑boot orders:** buffered upstream by `SymbolCoordinator`; not visible until engine Active.

### 2.2 Validation & Admission

- **Tick size:** `price % tick_size == 0`, else `Reject(BadTick)`.
- **Price bands:** price within ±X% (or ±abs) of **reference price**:
    - Cold start reference = snapshot.last_trade; if none, **MARKET/IOC rejected**; only inside‑band **LIMIT** accepted until first trade.
- **POST‑ONLY:** if it would cross at submitted price → **Reject(PostOnlyCross)**. No slide; no price improvement.
- **Risk/admission:** non‑blocking cache check from `AccountService`. Cache miss ⇒ `Reject(RiskUnavailable)`. (Hot path never blocks.)
- **Max order size / exposure caps:** enforced pre‑admission; violations rejected with reason.
- **SPSC full:** `Reject(Backpressure)` (never block).
- **Malformed:** any missing/invalid field ⇒ reject with explicit reason code.

### 2.3 Matching Rules

- **Priority:** strict **price‑time**. Key = `(timestamp_normalized, enqueue_sequence)`.
- **LIMIT:** match while crossing; remainder rests (FIFO at price level).
- **MARKET:** consume best prices until filled or book exhausted; never rests.
- **IOC:** like MARKET but price‑capped to submitted price; remainder cancels.
- **POST‑ONLY:** never matches on entry; if matching would occur at submit price, reject.
- **Partial fills:** remainder **retains original priority**.
- **Cancel vs fill race (same tick):** resolve by `(timestamp_normalized, enqueue_sequence)`; earlier event wins.
- **Self‑match policy:** default **prevent**; deterministically skip same‑account counterparties at that level (bounded scan). Configurable allow mode.

### 2.4 Order Book & State

- Two ladders (bid/ask), FIFO queues per level.
- **O(1) cancel** via stable handle (implementation detail covered in Bundle 2).
- Queries for best bid/ask and level qty totals (for downstream/UI).

### 2.5 Event Emission (to ExecutionManager)

- **Canonical per‑tick order:** **Trades → BookDeltas → OrderLifecycle → TickComplete**.
- **Trades:** include taker side, maker/taker order IDs, price, qty, logical tick.
- **BookDeltas:** level qty after update.
- **OrderLifecycle:** Accepted / Rejected (with reason) / Cancelled.
- **TickComplete:** per symbol, per tick.
- **Execution IDs:** deterministic layout (`(tick << SHIFT) | local_seq`) or delegated to ExecMgr’s allocator; must be **replay‑stable**.

### 2.6 Lifecycle & Coordination

- **Spawn:** only via `SymbolCoordinator`; queues pre‑allocated; thread pinned per placement policy.
- **Active participation:** added to `SimulationClock` participant set **before** next tick.
- **Eviction:** stop intake; drain to final `TickComplete`; deregister; free queues; emit control events.
- **Shutdown:** cancels all resting; emits lifecycle + final tick.

### 2.7 Replay & Snapshot

- **Inputs recorded:** orders (with normalized timestamps), cancels, config.
- **Outputs recorded:** all events in canonical order with tick.
- **Snapshot contains:** order book, open orders, reference price, exec‑ID local counter, and any knobs required to resume deterministically.
- **Recovery:** `snapshot → resume tick N+1 → WAL replay`, bitwise‑identical outputs.

### 2.8 Observability

- **Hot path:** no syscalls or formatting; metrics/logs go to an async diagnostics ring.
- **Counters/timers:** per‑tick latency, queue depths, rejects by reason, arena occupancy, best‑price churn, self‑match skips.
- **Debug mode:** opt‑in cycle counters; tombstones for freed orders.

### 2.9 Configuration (per symbol/class)

- `tick_size`, `price_floor/ceiling`, `price_band_{abs|percent}`, `arena_capacity`, `batch_max`, `exec_shift_bits`, `self_match_policy`, `elastic_arena` (growth only at tick boundaries), `reference_price_source`.

**Done when (FR):**

- All accept/validate/match behaviors above are test‑covered.
- A single simulated tick with mixed order types produces events in canonical order with stable IDs.
- Snapshot/restore → bitwise identical continuation.

---

## 3 Non‑Functional Requirements

### 3.1 Performance Targets

| Metric | Target | Notes |
| --- | --- | --- |
| Match decision p50 | ≤ **1.5 µs** | Dequeue → decision; no I/O |
| Match decision p99 | ≤ **3.0 µs** | Under peak simulated load |
| Throughput | ≥ **100k orders/s/core** | Sustained |
| Tick flush to egress | ≤ **10 µs** | To `ExecutionManager` ring |
| Cold start | ≤ **500 µs** | Spawn → Active |
| Max tick freq | 10,000 ticks/s | Stress mode |

### 3.2 Determinism & Replayability

- **Bitwise replay**: identical inputs → identical outputs (events, order).
- **Tick‑deterministic**: all effects happen in `tick(T)`, never between ticks.
- **Exec‑ID continuity**: resumes across snapshot; layout documented and versioned.
- **No wall‑clock dependence** for decisions or hashes.

### 3.3 Resource Usage & Isolation

| Resource | Constraint |
| --- | --- |
| Memory | Pre‑allocated; no heap in hot path; typical ≤ ~2 MB/symbol |
| Concurrency | One engine = one pinned thread/task; no global locks |
| NUMA | Engine memory allocated on same node as its thread (if applicable) |
| Queues | Bounded; never resized during tick; SPSC inbound, MPSC outbound |
| Backpressure | Inbound full ⇒ reject; Replay egress overflow ⇒ **fatal**; UI/analytics may drop per policy |

### 3.4 Failure Modes & Recovery

| Scenario | Behavior |
| --- | --- |
| Arena/queue full | Reject with explicit reason; metrics++ |
| Price band/tick violations | Reject; lifecycle event |
| Risk service cache miss | Reject; never block |
| Engine panic (debug) | Fail fast; emit diagnostic |
| Engine panic (prod sim) | Isolate via `SymbolCoordinator`; evict or snapshot‑restart depending on policy |
| Mid‑tick crash | Recover from last completed `TickComplete`; WAL is per‑tick flushable |

### 3.5 Security/Abuse Guardrails (sim integrity)

- **Token‑bucket admission limits** per account at `OrderGateway` (prevents SPSC flood).
- **Per‑symbol outstanding order caps** per account (defense against book bloat).
- **POST‑ONLY spam** counted; optional rate caps.
- **Deterministic drop** rules on overload (never random).

### 3.6 Compatibility & Build Constraints

- **Target:** 64‑bit little‑endian; fixed compiler versions for replay builds.
- **Determinism mode:** disable CPU‑specific reordering features; integer‑only arithmetic.
- **Schema versioning:** Snapshot/WAL/Events carry version/tag; changes documented in changelog.

### 3.7 Testability & Coverage

- **Unit:** ladder navigation, FIFO invariants, cancel O(1), partial fill priority.
- **Property:** price‑time never violated; POST‑ONLY never crosses; market never rests; cancel/fill race.
- **Replay:** run twice → bitwise‑equal; power‑fail between event families; recovery matches baseline.
- **Perf:** microbench on `match_one_tick` across depth profiles; arena exhaustion tests; p50/p99 targets asserted in CI (sim time).

**Done when (NFR):**

- CI asserts latency/throughput SLOs in perf harness.
- Replay tests pass (event hashes, book hashes, exec‑ID continuity).
- Failure injection suite proves isolation and deterministic recovery.

---

## 4. High-Level Execution Flow (per tick)

1. **Clock Trigger** – `SimulationClock` calls `Whistle.tick(T)`, passing the current logical tick ID.
2. **Drain Inbound Queue** – Dequeue up to `batch_max` messages from the **per-symbol SPSC<OrderMsg>**.
    - Messages may be new orders, cancels, or control signals.
    - If queue is empty, no state changes occur and Whistle immediately emits `TickComplete(T)`.
3. **Validate & Admit** – For each inbound message:
    - **Market Rules**: Tick size alignment, price band compliance.
    - **Order Type Rules**: POST-ONLY rejection if crossing, MARKET never rests, IOC cancels remainder.
    - **Risk Checks**: Admission verdict from `AccountService` risk cache. Cache miss ⇒ reject.
    - **Structural Checks**: Bounded arena capacity, handle validity.
    - All rejections produce explicit lifecycle events.
4. **Match Orders** –
    - Apply **strict price-time priority** to match resting orders in the book.
    - Partial fills retain original timestamp and sequence number.
    - Cancel vs. fill races are resolved by `(timestamp_normalized, enqueue_sequence)`.
    - Self-match prevention (if enabled) deterministically skips same-account counterparties.
5. **Emit Canonical Event Sequence** to `ExecutionManager`:
    
    **Trades → BookDeltas → OrderLifecycle → TickComplete(T)**.
    
    - Events are batched per tick for ordering and replay stability.
6. **Return Control** – `ExecutionManager` fans out events:
    - **ReplayEngine** persists events to WAL.
    - **AnalyticsEngine** consumes for metrics.
    - **WebUI** receives book/trade updates.
    - `SimulationClock` advances to next tick.
    

---

## 5. Interface Summary (high-level contracts)

### Inbound Interfaces

- **OrderRouter → SPSC<OrderMsg>**
    - One queue per symbol.
    - Single producer (router), single consumer (Whistle).
    - Never blocks; full ⇒ reject with `Reject(Backpressure)`.
- **SimulationClock → tick(now: TickId)**
    - Drives batch execution.
    - No matching or state changes occur outside this call.
- **LatencyModel → Normalized Timestamp**
    - Applied to inbound orders before enqueue.
    - Provides deterministic timing offsets for simulation.
- **AccountService → Admission Verdict**
    - Non-blocking lookup from risk cache.
    - Miss ⇒ reject without blocking match loop.
- **SymbolCoordinator → Lifecycle Control**
    - Spawn/evict Whistle instances.
    - Wire SPSC/MPSC queues.
    - Assign and pin OS thread / NUMA node.

### Outbound Interfaces

- **ExecutionManager → MPSC<EngineEvent>**
    - Whistle emits events in canonical per-tick order.
    - `ExecutionManager` assigns final `execution_id` if using centralized allocator, or accepts Whistle’s sharded deterministic ID layout.
    - Backpressure on this queue is handled per policy (lossless for replay, lossy for UI).
- **ReplayEngine / AnalyticsEngine / WebUI** (via ExecutionManager)
    - **ReplayEngine**: Lossless WAL persistence; fatal if overflow.
    - **AnalyticsEngine**: Metrics and aggregated book data.
    - **WebUI**: Real-time view of book and trades; allowed to drop frames if behind.

---

## 2.1 Core Types & Price Model

### 2.1.1 Numeric Model

- **No floats anywhere.** Prices and quantities are scaled integers.
- **Tick size** is a positive integer `tick` (e.g., 1 = cent if you choose cents).
- **Price** is an integer count of ticks relative to zero; validation ensures inputs align to `tick`

```rust
pub type TickId     = u64;  // logical time
pub type OrderId    = u64;
pub type AccountId  = u64;
pub type Qty        = u32;  // whole units only
pub type Price      = u32;  // scaled integer
pub type PriceIdx   = u32;  // index into flat ladder
pub type TsNorm     = u64;  // normalized timestamp from LatencyModel
pub type EnqSeq     = u32;  // per-tick, per-symbol enqueue sequence
```

### Price range & Indexing

We predefine a **valid price corridor** per symbol/class

```rust
#[derive(Clone, Copy)]
pub struct PriceDomain {
	pub floor: Price,
	pub ceil: Price,
	pub tick: Price,
}

impl PriceDomain {
	#[inline] pub fn idx(&self, p: Price) -> Option<PriceIdx> {
		if p < self.floor || p > self.ceil { return None; }
		let d = p - self.floor;
		if d % self.tick != 0 { return None; }
		Some(d / self.tick)
	}
	#[inline] pub fn price(&self, idx: PriceIdx) -> Price {
		self.floor + idx * self.tick
	}
	#[inline] pub fn ladder_len(&self) -> usize {
		(self.ceil - self.floor) as usize / self.tick as usize + 1;
	}
}
```

**Guarantees**

- All book storage uses `PriceIdx` ; conversion validated at admission.
- Tick size misalignment is rejected up front (`BadTick` ).

---

### 2.1.2 Order & Message Shapes

**Inbound messages (from OrderRouter via SPSC)**

Minimal, copyable structs optimized for parsing.

```cpp
#[repr(u8)]
pub enum Side          { Buy = 0, Sell = 1}
#[repr(u8)]
pub enum OrderType     { Limit = 0, Market = 1, Ioc = 2, PostOnly = 3 }

#[repr(u8)]
pub enum MsgKind       { Submit = 0, Cancel = 1 }

pub struct Submit {
	pub order_id:    OrderId,
	pub account_id:  AccountId,
	pub side:        Side,
	pub typ:         OrderType,
	pub price:       Option<Price>, // required for Limit/PostOnly/IOC
	pub qty:         Qty,
	pub ts_norm:     TsNorm,
	pub meta:        u64,
}

pub struct Cancel {
	pub order_id:    OrderId,
	pub ts_norm:     TsNorm,
}

pub struct InboundMsg {
	pub kind:    MsgKind,
	pub submit:  Option<Submit>,
	pub cancel:  Option<Cancel>,
	pub enq_seq: EnqSeq,
}
```

**Notes**

- `enq_seq` is the **tie-breaker** after `ts_norm` .
- All fields are validated at admission (presence, ranges, tick alignment).
- No heap; fixed layout; no strings.

---

### 2.1.3 Outbound events (to ExecutionManager via MPSC)

Canonical family only (ordering fixed per tick). Exact IDL comes later; these are the engine internal structs.

```cpp
pub struct EvTrade {
	pub symbol:     u32,
	pub tick:       TickId,
	pub exec_id:    u64,      // (tick << SHIFT) | seq OR assigned centrally
	pub price:      Price,
	pub qty:        Qty,
	pub taker_side: Side,
	pub maker_order: OrderId,
	pub taker_order: OrderId,
}

pub struct EvBookDelta {
	pub symbol:          u32,
	pub tick:            TickId,
	pub side:            Side,
	pub price:           Price,
	pub level_qty_after: Qty,
	
}

#[repr(u8)]
pub enum LifecycleKind { Accepted=0, Rejected=1, Cancelled=2 }

pub struct EvLifecycle {
    pub symbol: u32,
    pub tick:   TickId,
    pub kind:   LifecycleKind,
    pub order_id: OrderId,
    pub reason:  RejectReason, // Rejected: specific; Accepted/Cancelled: Ok/None
}

pub struct EvTickComplete {
    pub symbol: u32,
    pub tick:   TickId,
}
```

---

### 2.1.4 Reject reasons (explicit, enumerable)

```cpp
#[repr(u16)]
pub enum RejectReason {
    BadTick             = 1,
    OutOfBand           = 2,
    PostOnlyCross       = 3,
    MarketDisallowed    = 4, // cold start / halted
    IocDisallowed       = 5, // cold start / halted
    RiskUnavailable     = 6,
    InsufficientFunds   = 7,
    ExposureExceeded    = 8,
    ArenaFull           = 9,
    QueueBackpressure   = 10,
    Malformed           = 11,
    UnknownOrder        = 12, // cancel for non-existent
    SelfMatchBlocked    = 13, // if policy=prevent on submit
    MarketHalted        = 14,
}
```

**Guarantees**

- Every rejection emits a `Lifecycle` event with a single reason
- These codes are stable and appear in WAL.

---

## 2.1.5 Price Bands & Reference Price

**Policy**

- Reference price `ref_price` used for bands:
    - **Warm start:** `snapshot.last_trade`.
    - **Cold start:** if none available → **MARKET/IOC rejected**, only LIMIT inside band accepted until first trade sets `ref_price`.
- Bands may be absolute or percent; both can be configured. Effective rule:

```rust
pub enum BandMode { Percent(u16), Abs(Price) } // Percent stored in basis points (e.g., 1000 = 10.00%)
pub struct Bands { pub mode: BandMode }

#[inline]
fn in_band(p: Price, refp: Price, bands: &Bands) -> bool {
    match bands.mode {
        BandMode::Abs(b) => (p <= refp.saturating_add(b)) && (p + b >= refp),
        BandMode::Percent(bp) => {
            let up = refp + ((refp as u128 * bp as u128) / 10_000u128) as u32;
            let dn = refp - ((refp as u128 * bp as u128) / 10_000u128) as u32;
            p >= dn && p <= up
        }
    }
}

```

**Admission outcome**

- LIMIT outside band ⇒ `Reject(OutOfBand)`.
- In cold start (no ref): `Reject(MarketDisallowed)` / `Reject(IocDisallowed)`; LIMIT must be within a configured provisional corridor, or within `price_floor..=price_ceil` if strict.

---

## 2.1.6 Priority Key & Tie‑Breaking

- Primary: `ts_norm` (normalized timestamp).
- Secondary: `enq_seq` (per‑tick, per‑symbol).
- Within price level, FIFO is preserved via **intrusive queue** and the above priority at admission.
- **Cancel vs. Fill in same tick:** earlier `(ts_norm, enq_seq)` wins — this is deterministic and consistent with SPSC enqueue order.

---

## 2.1.7 Engine Config (per symbol/class)

```rust
pub struct EngineCfg {
    pub symbol:        u32,
    pub price_domain:  PriceDomain,
    pub bands:         Bands,
    pub batch_max:     u32,   // max msgs processed per tick
    pub arena_capacity:u32,   // max open orders stored
    pub exec_shift:    u8,    // bits in exec_id for local counter
    pub self_match_block: bool,
    pub elastic_arena: bool,  // allow growth only at tick boundaries
    pub allow_market_cold_start: bool, // typically false
}

```

**Invariants**

- `arena_capacity` ≥ max outstanding orders per policy; if exceeded at runtime ⇒ `ArenaFull`.
- `batch_max` bounds work per tick; prevents starvation under extreme ingress.

---

## 2.1.8 Deterministic Execution ID Layout

Two modes (compile/runtime selectable), both deterministic:

1. **Sharded (engine‑local):**

```rust
exec_id = (tick << exec_shift) | local_trade_seq   // local_trade_seq resets each tick
```

- Simple, no shared state; guaranteed unique within a simulation run as long as `(exec_shift)` fits max trades per tick.
1. **Centralized (ExecutionManager):**
- Whistle sends trades with `exec_id = 0`; ExecMgr stamps a global monotonic ID.
- Replay safety: ExecMgr’s allocator is deterministic per run (seeded or tick‑scoped).

**We will default to sharded.** ExecMgr acceptance must honor and never rewrite if already set.

---

## 2.1.9 Sanity & Safety Checks (admission fast‑path)

- `price_domain.idx(price).is_some()` → else `BadTick` or `OutOfRange`.
- `in_band(price, ref_price, bands)` for LIMIT/IOC (IOC compares against submitted price).
- POST‑ONLY: if `would_cross_on_entry(price, side)` ⇒ `PostOnlyCross`.
- Risk cache verdict present (`Ok` or explicit violation).
- `arena.has_capacity()` or `ArenaFull`.
- `enq_seq` monotonic within tick (enforced by router; asserted in debug here).

---

## 2.2 Data Structures (flat ladder, arena, handles, bitset)

### 2.2.1 Overview

We store the book as a **flat, index-addressed price ladder** with **intrusive FIFO queues** per level and an **arena** of `Order` objects referenced by **u32 handles.** A **bitset** tracks non-empty levels for O(1)/O(word) best-price navigation. Cancels are O(1) via a compact `OrderIndex (order_id -> handle)` open—addressed table.

Core properties:

- **Heap-free hot path**. All buffers preallocated.
- **O(1)** insert at tail, cancel/unlink, and level-qty updates.
- **Branch-lean** best-price scan using bit operations.

---

### 2.2.2 Order arena & handle model

**Handle**

```rust
#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq)]
pub struct OrderHandle(u32); // 0..=u32::MAX-1 valid; u32::MAX reserved for "None"
const H_NONE: OrderHandle = OrderHandle(u32::MAX);
```

**Order (packed - hot fields first)**

```rust
#[repr(C)]
pub struct Order {
	// hot: read/updated in matching
	pub id:        OrderId,
	pub acct:      AccountId,
	pub side:      Side,
	pub price_idx: PriceIdx,
	pub qty_open:  Qty,
	pub ts_norm:   TsNorm,
	pub enq_seq:   EnqSeq,
	// intrusive doubly-linked node in price level FIFO
	pub prev:      OrderHandle,
	pub next:      OrderHandle,
	
	// cold/debug (behind feature flags)
	pub typ:       OrderType,
	pub price_raw: Price,
	pub meta:      u64,
}
```

**Arena**

Fixed-capacity array with a LIFO free list.

```rust
pub struct Arena {
	buf:  Box<[Order]>,
	free: Vec<u32>,
}
```

**Operations**

- `alloc() -> Option<OrderHandle>` : pop from `free` (O(1)); None ⇒ `ArenaFull` .
- `free(h)` : push index back to `free` (O(1))
- `get(h) -> &Order` / `get_mut(h) -> &mut Order` : unchecked in release; debug asserts bound.

**Notes**

- LIFO free list improves cache locality under bursty flow.
- We never move `Order` objects; handles remain stable for their lifetime.

---

### 2.2.3 OrderIndex (order_id → handle)

Open-addressed hash table (power-of-two capacity), linear probe.

```rust
pub struct OrderIndexEntry { pub key: OrderId, pub val: OrderHandle } // key=0 used as EMPTY sentinal
pub struct OrderIndex {
	mask: u32,
	tabs: Box<[OrderIndexEntry]>,
	tombstones: u32,
}
```

**Hash:** SplitMix64 on `OrderId` , cast to u32, `idx = hash & mask` .

**Ops**

- `insert(key, h) -> Result<(), Full>` : probe until EMPTY or same key.
- `get(key) -> Option<OrderHandle>` : probe until EMPTY or key match
- `remove(key) -> Option<OrderHandle>` : place **tombstone** (key = TOMBSTONE_CONST), return handle

**Sizing**

- Capacity ≥ **2x peak live orders** (e.g., arena_capacity * 2).
- Rebuild allowed only at **tick boundary** if tombstone ratio > 20% (optional)
- In hot path we never resize; if table full, admission rejects `Reject(ArenaFull)` to preserve determinism

---

### 2.2.4 Price ladder with intrusive FIFO

**Level**

```rust
pub struct Level {
	pub head: OrderHandle,
	pub tail: OrderHandle,
	pub total_qty: Qty,
}
```

- Empty level: `head = tail = H_NONE` , `total_qty = 0`

**Book**

```rust
pub struct Book {
	pub dom: PriceDomain,
	pub levels: Box<[Level]>,
	pub non_empty: Bitset,
	pub best_bid_idx: Option<PriceIdx>,
	pub best_ask_idx: Option<PriceIdx>,
}
```

**Intrusive FIFO ops**

- Insert tail:

```rust
let L = &mut levels[pidx];
if L.tail == H_NONE {
    L.head = h;
    L.tail = h;
} else {
    arena[L.tail].next = h;
    arena[h].prev = L.tail;
    L.tail = h;
}
L.total_qty += qty;
non_empty.set(pidx);
```

- **Unlink** (O(1)):

```rust
let pidx = o.price_idx as usize;
let q    = o.qty_open;

if o.prev != H_NONE { arena[o.prev].next = o.next; } else { levels[pidx].head = o.next; }
if o.next != H_NONE { arena[o.next].prev = o.prev; } else { levels[pidx].tail = o.prev; }

levels[pidx].total_qty = levels[pidx].total_qty.saturating_sub(q);
if levels[pidx].head == H_NONE {
    non_empty.clear(pidx);
}

```

(If partial fill → only adjust `total_qty` , do not unlink.)

- **Partial fill:** `o.qty_open -= traded; levels[pidx].total_qty -= traded;`  (retain position)

**Complexity:** all O(1) except walking FIFO for **self-match skip** (bounded; see 2.2.7)

---

### 2.2.5 Bitset for best-price navigation

**Layout**

- Packed array of `u64` words; size = `ceil(levels.len() / 64)` .
- Index helpers:

```rust
#[inline] fn word_idx(i: usize) -> usize { i >> 6 }
#[inline] fn bit_mask(i: usize) -> u64 { 1u64 << (i & 63) }
```

**Ops**

```rust
fn set(words: &mut [u64], i: usize) {
    let w = word_idx(i);
    let m = bit_mask(i);
    words[w] |= m;
}

fn clear(words: &mut [u64], i: usize) {
    let w = word_idx(i);
    let m = bit_mask(i);
    words[w] &= !m;
}
```

Next non-empty ask ≥ i:

```rust
fn next_ask_at_or_above(bs: &Bitset, i: usize) -> Option<usize> {
    let w = i >> 6; let off = i & 63;
    if w < bs.words.len() {
        let mut bits = bs.words[w] & (!0u64 << off);
        if bits != 0 { return Some((w<<6) + bits.trailing_zeros() as usize); }
        for w2 in (w+1)..bs.words.len() {
            let v = bs.words[w2];
            if v != 0 { return Some((w2<<6) + v.trailing_zeros() as usize); }
        }
    }
    None
}
```

- **Prev non-empty bid ≤ i:** mirror using `leading_zeros()` and reverse scan.

**Best pointers**

- Maintain `best_bid_idx` / `best_ask_idx` for fast top-of-book when unchanged
- Update on:
    - Level becomes non-empty (potentially new best)
    - Level becomes empty (recompute via bitset neighbor)
- Matching uses best pointers; falls back to bitset scan when crossing levels.

---

### 2.2.6 Book API (internal)

```rust
impl Book {
	pub fn insert_tail(&mut self, a: &mut Arena, h: OrderHandle, pidx: PriceIdx, qty: Qty);
	pub fn unlink(&mut self, a: &mut Arena, h: OrderHandle);
	pub fn partial_fill(&mut self, h: OrderHandle, traded: Qty);
	pub fn best_bid(&self) -> Option<PriceIdx>;
	pub fn best_ask(&self) -> Option<PriceIdx>;
	pub fn next_ask_at_or_above(&self, i: PriceIdx) -> Option<PriceIdx>;
	pub fn prev_bid_at_or_below(&self, i: PriceIdx) -> Option<PriceIdx>;
	pub fn level_qty(&self, i: PriceIdx) -> Qty; 
}
```

**Invariants**

- `levels[i].total_qty` equals sum of `qty_open` for all orders at level `i`
- `non_empty[i] == (levels[i].head != H_NONE)`

---

### 2.2.7 Self-Match Skip

When matching an **aggressor** order against the book, Whistle must ensure it never executes a trade where **buyer and seller are the same account** (self-trade). This can occur when:

- The aggressor already has resting orders on the opposite side of the book.
- Multiple strategies or sub-accounts map to the same `account_id` in our simulation

**Why skip in-place rather than reject?**

- Many real exchanges use a *self-match prevention* (SMP) policy where the aggressor’s opposite-side orders are canceled or skipped instead of filling.
- In simulation, we prefer **skip** for determinism and minimal mutation.

---

**Algorithm**

```rust
fn match_with_skip(a: &mut Arena, book: &mut Book, aggressor: &AggressorOrder) {
	let mut price_idx = book.best_ask_idx();  // or best_bid_idx depending on side
	
	while let Some(pidx) = price_idx {
		// Walk FIFO at this price
		let mut h = book.levels[pidx].head;
		while h != H_NONE {
			let resting = &mut a[h];
			if resting.acct == aggressor.acct {
				h = resting.next;
				continue;
			}
			
			// ... execute trade logic ...
			
			// Break if aggressor filled
			if aggressor.qty_open == 0 { return; }
			
			h = resting.next;
		} 
		
		// Move to next level (bitset lookup)
		price_idx = book.next_ask_at_or_above(pidx + 1);
	}
}
```

**Complexity**

- **Worst case:** walks all orders at best price level to skip own orders
- **Bounded in practice:**
    - Aggressors rarely own > few resting orders at best level
    - FIFO structure keeps memory-local pointer chasing.
- Still **O(1)** per order processed if skip count is small (and bitset navigation is used for next level)

---

**Configurable Behavior**

We can allow per-symbol policy for self-match handling:

- **Skip** (default): Leave resting order untouched, no trade.
- **Cancel resting:** Remove own orders from the opposite side to allow aggressor to proceed.
- **Cancel aggressor:** Reject/kill aggressor if it would self-match.

These are pure policy hooks - core matching loop stays the same, only the skip/cancel branch changes.

---

## 2.3 Validation & Admission Rules

### 2.3.1 Overview

Before any order is admitted into the book, it passes through **deterministic, branch-lean validation**.

The goal:

- Reject invalid or risky orders **up front** before touching book state.
- Perform all checks in **hot-path safe** mode: no heap alloc, no syscalls, no locking.
- Use **fail-fast** ordering — cheapest checks first.

---

### 2.3.2 Fast-Path Admission Sequence

**Order of operations (per inbound order):**

1. **Arena capacity check** — if no free order slots, reject `Reject(ArenaFull)`.
2. **Order ID uniqueness** — probe `OrderIndex`; if key already exists, reject `Reject(DuplicateId)`.
3. **Market state** — if symbol halted/paused, reject `Reject(MarketHalted)`.
4. **Tick size check** — `(price_raw % tick_size == 0)`; else reject `Reject(BadTick)`.
5. **Price band check** — `(abs(price - last_trade) <= band_limit)`; else reject `Reject(PriceOutOfBand)`.
6. **Side & type constraints**:
    - `MARKET` must have `qty > 0` and no price.
    - `POST_ONLY` must not cross top-of-book (if it does → reject).
7. **Max order size** — compare `qty` to `max_qty_per_order` and per-account exposure.
8. **Account risk check** — consult `AccountService` cache (non-blocking).
    - If miss or fail → reject `Reject(RiskLimit)`.

If all pass:

- Allocate order in arena.
- Insert into `OrderIndex` and FIFO queue (or pass to matcher immediately if crossing).

---

### 2.3.3 Reject Codes

All rejects return a **deterministic code** (enum) for logging, replay, and testing.

```rust
pub enum RejectCode {
    ArenaFull,
    DuplicateId,
    MarketHalted,
    BadTick,
    PriceOutOfBand,
    InvalidOrderType,
    PostOnlyWouldCross,
    MaxOrderQtyExceeded,
    RiskLimit,
}
```

- Codes are stable across builds for replayability.
- Logged in `OrderLifecycle` events with timestamp, order_id, reason.

---

### 2.3.4 Risk Cache Interface

The **AccountService** maintains a shared read-only cache in Whistle’s thread memory space:

- Per-account:
    - `max_open_qty`
    - `current_open_qty`
    - `max_notional`
    - `current_notional`
- Whistle’s admission path only **reads** from cache (lock-free, NUMA-local).
- Cache is updated off-thread by AccountService; Whistle sees next-tick consistency.

On cache miss → reject with `Reject(RiskLimit)` to preserve determinism (no blocking).

---

### 2.3.5 Determinism Considerations

- Validation order is **fixed** and cannot depend on runtime conditions outside the order message & cached state.
- All rejections are final — no retries within the same tick.
- If an order fails multiple checks, **first failure in the sequence wins** (to ensure replay stability).

---

### 2.3.6 Profiling Hooks

- Optional compile-time feature flag `profile_validation`:
    - Records cycles spent in validation stages.
    - Counts reject frequency per code.
    - Writes to diagnostics ring buffer asynchronously.

---

## 2.4 Matching Logic

### 2.4.1 Overview

Once an inbound order passes validation, Whistle processes it through a **single-threaded, deterministic match loop.**

Key rules:

- **Strict price-time priority -** better price first; earlier timestamp within price wins
- **Partial fills retain queue position -** FIFO within each price level.
- **Self-match skip -** aggressor never trades with own resting orders (policy-driven).
- **Hot path = allocation-free** - all order/price data comes from preallocated arena & ladders.

---

### 2.4.2 Matching Sequence (per aggressor order)

1. **Set target side & ladder direction**
    1. Buy aggressor: match against best ask (lowest price index upward)
    2. Sell aggressor: match against best bid (highest price index downard).
2. **Best price loop**
    1. While aggressor qty > 0 and best-price index crosses aggressor’s limit (or any price if MARKET):
        1. Walk FIFO at that price level
        2. Skip self-matches per 2.2.7
        3. For each resting order:
            1. Compute `trade_pty = min(aggressor.qty_open, resting.qty_open)` 
            2. Generate trade events (no side effects in hot loop besides updating book)
            3. Decrement quantities in both orders & level totals
            4. If resting fully filled → unlink from level, free arena slot, remove from index
            5. If aggresor fully filled → exit match loop
3. Advance to next price level
    1. Use bitset navigation (`next_ask_at_or_above` / `prev_bid_at_or_below` ).
    2. Update best bid/ask pointers as needed
4. Post any remainder (if applicable)
    1. If order type is `LIMIT` or `POST_ONLY` (and POST_ONLY didn’t cross in admission), insert aggressor into tail of FIFO for its price level.
    2. MARKET/IOC orders with remainder are discarded (IOC sends cancel event).

---

### 2.4.3 Price Crossing Conditions

- Buy LIMIT crosses if `best_ask_price <= aggressor_price`
- Sell LIMIT crosses if `best_bid_price >= aggressor_price`
- MARKET always considered crossing (subject to book depth)

---

### 2.4.4 Event Emission Ordering

Events emitted to `ExecutionManager` **after match loop** in canonical sequence:

1. **Trade events -** one per aggressor/resting fill pair
2. **Book delta events -** only for levels whose qty changed
3. **Order lifecycle events -** fills, cancels, rejections
4. **TickComplete(T) -** signals downstream consumers (Replay, UI, Analytics)

---

### **2.4.5 Determinism Rules**

- **Matching loop iterates strictly by `(price_idx, ts_norm, enq_seq)`**
- Self-match skip uses **stable account_id equality** (no aliasing)
- Cancels vs. fills in same tick resolved by:
    - If cancel and matching fill are queued in same tick → cancel wins if enqueued earlier (checked via enq_seq)
- No randomization or time-of-day effects

---

### 2.4.6 Complexity

- O(F) where F = fills executed (bounded by qty & depth)
- No full-book scans: bitset jump reduces level search to O(word)
- Pointer chasing confined to same cache lines in price ladder & arena

---

### 2.4.7 Profiling Hooks

- `profile_match` flag enables:
    - Per-match loop cycle count.
    - Depth traversed per aggressor
    - SMP skip count
- Writes to diagnostics ring buffer asynchronously

---

# 2.5 Trade Event Generation

### 2.5.1 Purpose

Define exactly **what Whistle emits**, **when**, and **with which fields**, so `ExecutionManager` can assign final IDs, fan out, and persist — while Whistle stays hot-path safe and deterministic.

---

### 2.5.2 Event Types (Whistle → ExecutionManager)

All events are pushed to a **single MPSC** per `ExecutionManager` in **canonical order** after the match loop for the current tick batch.

```rust
enum EngineEvent {
  Trade {
    symbol: SymbolId,
    // participants
    maker_order_id: OrderId,
    taker_order_id: OrderId,
    maker_acct: AccountId,
    taker_acct: AccountId,
    // economics
    price_idx: PriceIdx,      // ladder index
    price_raw: Price,         // optional (feature flag)
    qty: Qty,
    aggressor_side: Side,     // Buy or Sell (taker)
    // timing
    tick: TickId,
    ts_norm: TsNorm,          // aggressor normalized ts
    seq_in_tick: u32,         // see 2.5.5
  },

  BookDelta {
    symbol: SymbolId,
    side: Side,               // Bid or Ask
    price_idx: PriceIdx,
    new_total_qty: Qty,       // post‑update level quantity
    tick: TickId,
  },

  OrderLifecycle {
    symbol: SymbolId,
    order_id: OrderId,
    account_id: AccountId,
    event: OrderEventKind,    // Accepted | PartiallyFilled | Filled | Cancelled | Rejected
    reason: Option<RejectCode>,
    last_fill_price_idx: Option<PriceIdx>,
    last_fill_qty: Option<Qty>,
    remaining_qty: Qty,
    tick: TickId,
  },

  TickComplete {
    symbol: SymbolId,
    tick: TickId,
    // optional diagnostics (counts) behind feature flag
  },
}

```

`OrderEventKind` and `RejectCode` are the stable enums we already defined/used.

---

### 2.5.3 What Whistle Stamps vs. What ExecMgr Stamps

| Field | Stamped by | Notes |
| --- | --- | --- |
| `symbol`, `tick`, `price_idx`, `qty`, `side` | **Whistle** | Known in match loop. |
| `maker_order_id`, `taker_order_id`, accounts | **Whistle** | Comes from arena & aggressor. |
| `seq_in_tick` | **Whistle** | Deterministic per-tick counter (see 2.5.5). |
| `execution_id` (global) | **ExecutionManager** | Canonical, monotonic across the entire sim. |
| Final encoding/serialization | **ExecutionManager** | Flatbuffers/Cap’n Proto choice downstream. |

If configured to use **sharded execution IDs** (advanced mode), Whistle can optionally stamp a **local shard_id + local_seq**; ExecMgr will combine to global.

---

### 2.5.4 Canonical Emission Order (per tick batch)

Within a single `tick()` call for a symbol, events are emitted in **this order**:

1. **Trades** (in strict `(price_idx, maker.ts_norm, maker.enq_seq)` order as produced by the loop).
2. **BookDeltas** (for each touched level, coalesced to **final post-state** per level).
3. **OrderLifecycle** (Accepted/Partial/Fill/Cancel/Reject in the order they became true).
4. **TickComplete** (exactly one per symbol per tick).

**Determinism:**

- No interleaving between these groups.
- Within a group, iterate in **ascending `seq_in_tick`**.

---

### 2.5.5 Sequencing Rules (no gaps, total order per tick)

Whistle maintains a **per-symbol, per-tick** counter:

- Start at `0` at tick entry.
- Increment for **every Trade** and **every OrderLifecycle** emission **in the order they are logically produced**.
- BookDeltas don’t need their own sequence; they are deduplicated/coalesced and emitted after Trades using a fixed key order `(side, price_idx)`.

This gives ExecMgr a stable sub-ordering even before it assigns global `execution_id`s.

---

### 2.5.6 Coalescing BookDeltas

During the match loop, track a small map/array:

- Key: `(side, price_idx)`
- Value: **final `new_total_qty`** for that level at end of tick.
- Emit in **(side asc: Bid before Ask, then price_idx ascending for asks / descending for bids)** for readability; or simply price_idx ascending for both if UI prefers consistency. Pick one and freeze it.

---

### 2.5.7 Lifecycle Emission Rules

- **Accepted**: After successful admission (post-validation, after arena alloc/insert if it rests, or after immediate match if fully filled within the same tick you still emit Accepted first).
- **PartiallyFilled**: Emit after each trade that leaves `remaining_qty > 0`.
- **Filled**: Emit when `remaining_qty == 0` for the order.
- **Cancelled**: On explicit cancel or IOC/Market remainder.
- **Rejected**: Emit immediately with `reason`.

**Note:** You may emit **Accepted → Filled** back-to-back if a just‑admitted order fully matches in the same tick.

---

### 2.5.8 Backpressure & Safety

- Whistle **never blocks** on the MPSC to ExecMgr.
- On push failure (queue full), apply **policy**:
    - **In simulation mode**: this is a fatal config error (queue size too small). Whistle sets a **symbol-fatal flag** and signals `SymbolCoordinator` to evict after `TickComplete`.
    - **In profiling mode**: optionally drop **UI‑only** lifecycle events via a feature flag (never Trades or BookDeltas). Default remains **no drop**.

---

### 2.5.9 Minimal Emission Pseudocode

```rust
fn emit_for_tick(symbol, tick, ctx: &mut EmitCtx) {
    // 1) Trades
    for t in ctx.trades.iter_ordered() {
        push(EngineEvent::Trade { /* stamp fields incl. seq_in_tick */ });
    }

    // 2) BookDeltas (coalesced)
    for (side, pidx, qty) in ctx.book_deltas.iter_ordered() {
        push(EngineEvent::BookDelta { symbol, side, price_idx: pidx, new_total_qty: qty, tick });
    }

    // 3) OrderLifecycle
    for ev in ctx.lifecycle.iter_ordered() {
        push(EngineEvent::OrderLifecycle { /* … */ });
    }

    // 4) TickComplete
    push(EngineEvent::TickComplete { symbol, tick });
}

```

`push()` is a non-blocking enqueue to ExecMgr’s MPSC.

---

### 2.5.10 Invariants & Tests

- **Exactly one** `TickComplete` per symbol per tick.
- No `Trade` without a corresponding maker/taker order id/qty update.
- Coalesced `BookDelta` matches the book’s actual post-tick total for that level.
- `seq_in_tick` is **strictly increasing**, starts at 0 every tick, no gaps.
- Replaying the same inputs yields byte-identical `EngineEvent` streams (before ExecMgr stamps `execution_id`).

---

## 2.6 Engine Lifecycle & Clock Hooks

### 2.6.1 States

```rust
Idle → Booting → Running → (StopRequested) → Draining → Stopped
                         ↘ Faulted → Quarantine (policy)

```

- **Idle**: constructed, queues not wired.
- **Booting**: queues wired; arena/book allocated; best pointers initialized
- **Running:** accepts ticks; processes SPSC; emits events
- **StopRequested:** external request to stop; effect takes place at **next tick boundary**
- **Draining:** processes any already-admitted work until `TickComplete` ; no new intake.
- **Stopped:** deregistered; resources freed.
- **Faulted/Quarantine:** hot-path invariant boroke or MPSC push policy says fatal; coordinator decides eviction/restart.

### 2.6.2 Public lifecycle API (symbol-local, single thread)

```rust
pub enum StartMode { Cold, Warm {snapshot: SnapshotBlob} }

impl WhistleEngine {
	pub fn boot(&mut self, mode: StartMode) -> Result<(), BootError>;
	pub fn tick(&mut self, now, TickId);
	pub fn request_stop(&mut self);
	pub fn is_running(&self) -> bool;
	pub fn snapshot(&self) -> SnapshotBlob;
	
}
```

### 2.6.3 Clock & Coordinator contracts

- **Registration**: `SymbolCoordinator` registers the symbol with `SimulationClock` **before** the first tick after `boot()` succeeds.
- **Tick delivery**: `SimulationClock` calls `tick(now)` **once per tick** for each registered symbol on its owning thread.
- **Boundary guardrails**:
    - **No lifecycle mutation mid‑tick.** `request_stop()` sets a flag; the engine enters **Draining** at the **next** tick start.
    - **No structural maintenance mid‑tick.** Any optional compaction (e.g., OrderIndex tombstone rebuild) occurs **after** `TickComplete`.
    - **Exactly one** `TickComplete { symbol, tick }` is emitted per `tick()` call.

### 2.6.4 Tick skeleton (authoritative)

```rust
pub fn tick(&mut self, now: TickId) {
    debug_assert!(self.state == Running || self.state == Draining);

    // 0) tick entry
    self.seq_in_tick = 0;
    self.emit_ctx.reset();

    // 1) drain inbound up to batch_max (SPSC never blocks)
    let n = self.ingest_from_spsc(now, self.batch_max);

    // 2) validate + admit; MATCH; update book
    self.process_admitted(now);

    // 3) emit canonical events for this tick (2.5 order)
    self.emit_for_tick(now);

    // 4) finalize
    self.emit_tick_complete(now);

    // 5) boundary actions (safe to mutate outside hot path)
    if self.maintenance_needed { self.compact_if_needed(); }

    // 6) state transitions at boundary
    if self.stop_requested { self.state = Draining; }
    if self.state == Draining && n == 0 && self.book_is_quiescent() {
        self.state = Stopped;
        // coordinator will deregister and free resources
    }
}

```

Notes:

- `ingest_from_spsc()` preserves SPSC producer order; attaches `enq_seq` if not pre‑stamped.
- `batch_max`: config knob to cap per‑tick work; ensures fairness across symbols. If more orders remain, they are processed on the next tick.

### 2.6.5 Cold vs. Warm boot

- **Cold**:
    - Zeroed book; `best_bid_idx = best_ask_idx = None`.
    - `last_trade_price = default_mid(dom)` (or unset if not used by price bands).
    - `seq_in_tick = 0`, counters reset; tombstones = 0.
- **Warm**:
    - Rehydrate **arena**, **order index**, **levels**, **bitset**, **best pointers**, `last_trade_price`, and **config** (tick size, bands, policies).
    - Validate snapshot invariants, then enter **Running** ready to tick at `resume_tick`.

### 2.6.6 Fault handling (hot‑path safe)

- If **MPSC push** fails and policy is **fatal**: set `fatal_flag`, keep processing local state, still emit `TickComplete`, then coordinator evicts at boundary.
- If an invariant fails in release: convert to **Reject** where possible; in `debug` build, `debug_assert!` trips.

---

## 2.7 Replay & Determinism Hooks

### 2.7.1 Determinism principles

- **Input‑deterministic**: given the same **order stream**, **config**, and **tick schedule**, outputs are **byte‑identical**.
- **Boundary snapshots only**: snapshots are taken **after** `TickComplete(T)` and resume at `T+1`.
- **No wall‑clock** or entropy used; all timestamps are **logical** (tick + `ts_norm`).

### 2.7.2 What to record (for lossless replay)

**Inputs (WAL)**

- For each inbound order (as it enters SPSC):
    - `order_id, account_id, side, typ, price_raw?, qty, ts_norm, symbol, tick_enqueued, enq_seq`
    - Admission verdict (`Accepted/Rejected + RejectCode`) **optional**: only needed if AccountService isn’t snapshotted (see below).
- For cancels: `order_id, account_id, tick_enqueued, enq_seq`.
- **Config** at session start: tick size, price bands, POST‑ONLY policy, SMP policy, `batch_max`, etc.
- **Tick schedule metadata**: start tick, clock stride if synthetic.

**Outputs (for validation / UI)**

- Full `EngineEvent` stream from 2.5 (pre‑`execution_id`), including `seq_in_tick`, `TickComplete`.

### 2.7.3 AccountService interaction (determinism)

- Admission reads a **local, read‑only risk cache** with **epoch** `risk_epoch`.
- **Replay options** (choose one, per run):
    1. **Snapshot the risk cache** (preferred): include per‑account limits and utilization in the snapshot/WAL header for the current `risk_epoch`. Rehydrate exactly → deterministic admission.
    2. **Record admission verdicts** in the input WAL (for each order) and **bypass live risk checks** during replay. (Heavier log, but allows external risk evolution to be ignored.)
- If neither is present → the run is **non‑replayable**; reject at startup in replay mode.

### 2.7.4 Snapshot content (`SnapshotBlob`)

Versioned, fixed‑layout binary. Captured **after** `TickComplete(T)`.

```rust
Header {
  version,
  symbol_id,
  resume_tick,                 // T+1
  config_hash,                 // tick size, bands, policies
  risk_epoch,                  // if using risk snapshot
}

State {
  // Arena
  capacity, free_list[], orders[0..n] (packed Order structs)

  // OrderIndex
  mask, tombstones, table[] (key,val),   // stable layout (incl. tombstones)

  // Book
  dom, levels[] { head, tail, total_qty },
  non_empty_bitset[],
  best_bid_idx, best_ask_idx,
  last_trade_price,

  // Cursors
  seq_in_tick_reset=0,
  maintenance flags (if any; should be false at boundary),
}

```

**Invariants to validate on load**:

- Every non‑EMPTY/ non‑tombstone `OrderIndex` entry points to a live arena slot.
- Each `Level.total_qty` equals sum of `qty_open` for its FIFO chain.
- `non_empty[i] == (levels[i].head != H_NONE)`.
- Best pointers are consistent with bitset (optional re‑derive and compare).

### 2.7.5 Restore sequence (Warm)

```rust
fn boot_warm(snapshot: SnapshotBlob) -> Result<()> {
    parse_and_check_header(snapshot.header)?;
    allocate_buffers_exact(snapshot.sizes)?;
    load_arena(snapshot.orders, snapshot.free_list);
    load_index(snapshot.index);
    load_book(snapshot.levels, snapshot.non_empty, snapshot.best_ptrs);
    self.last_trade_price = snapshot.last_trade_price;
    self.seq_in_tick = 0;
    validate_invariants()?;
    self.state = Running;
    self.resume_tick = snapshot.resume_tick;
    Ok(())
}

```

### 2.7.6 Replay procedure (engine‑local)

1. Initialize Whistle with **Cold** or **Warm** (matching the recorded session).
2. Feed the recorded input WAL to the **same SPSC order** and **cancel** endpoints in the **original enqueue order** (use `tick_enqueued` and `enq_seq`).
3. Drive the **tick schedule** identically.
4. Capture Whistle’s **EngineEvent** stream; compute a rolling hash per tick.
5. Assert equality with the recorded hashes / bytes (pre‑`execution_id`).

### 2.7.7 Hashes (cheap integrity)

- **Book hash (post‑tick)**: xxh3 over `(side, price_idx, level_qty, head_id?, tail_id?)`.
- **Event hash (per tick)**: xxh3 over serialized `EngineEvent`s including `seq_in_tick`.
- Store both in diagnostics (and optionally WAL) for quick diffing.

### 2.7.8 What is *not* allowed in replay mode

- Changing config that affects validation/matching (tick size, bands, POST‑ONLY, SMP, `batch_max`).
- Resizing arena/index or altering placement policies mid‑run.
- Non‑boundary snapshots (mid‑tick). If provided → reject with `SnapshotAtNonBoundary`.

---

## 2.8 Implementation Pitfalls

Common violations of Whistle's non-negotiable invariants that break determinism, performance, or correctness.

**Event order violations** — Emitting events out of canonical sequence (Trades → BookDeltas → OrderLifecycle → TickComplete) breaks replay determinism.

**Mid-tick state mutations** — Modifying book state outside `tick()` violates tick-bounded execution. All state changes must occur within the tick boundary.

**Hot-path allocation** — Heap allocation in match loops kills performance. Use preallocated arenas and fixed-size buffers.

**Priority violations** — Using wall-clock time instead of `(ts_norm, enq_seq)` breaks price-time fairness. Partial fills must retain original priority.

**Backpressure blocking** — Waiting for queue space instead of rejecting causes deadlocks. SPSC full ⇒ Reject(Backpressure).

**Missing TickComplete** — Every `tick()` must emit exactly one `TickComplete` event.

**Sequence numbering errors** — `seq_in_tick` must increment for each Trade and OrderLifecycle event, starting at 0 each tick.

**POST-ONLY validation** — Must reject orders that would cross the spread at submit price (no slide, no price improvement).

**Self-match prevention** — Skip same-account orders during matching to prevent self-trades.

Use debug assertions to validate invariants at runtime. Property tests should verify price-time priority and replay determinism. Performance tests must confirm allocation-free hot paths.