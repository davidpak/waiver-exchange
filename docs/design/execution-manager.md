> **Implementation Status:** Mostly implemented. MPSC event ingestion, normalization, fanout dispatch, tick tracking, and trade settlement via AccountService all working. PostSettlementCallback extension mechanism exists. Flatbuffers serialization not used (direct event structs instead). WAL writing happens here, not in a separate ReplayEngine.

---

---

## 1. Overview

The `ExecutionManager` is the central post-match emission and event distribution service within The Waiver Exchange architecture. It is the **only authorized egress point** for market events produced by the `Whistle` matching engine, ensuring that all trade executions, order state changes, and system-level outputs are captured, formatted, and dispatched deterministically. 

Every `Whistle` instance emits its results exclusively to the `ExecutionManager` , which in turn **fans out structured messages** to the system’s crucial observability, analytics, persistence, and user-facing components - including:

- `ReplayEngine` (for deterministic WAL logging and snapshot linking)
- `AnalyticsEngine` (for post-trade metrics, latency, and diagnostics)
- `WebUI` (for real-time depth, trades, and user visibility)
- Trace/debug streams (for CLI visibility, test validation, or inspection)

This fan-out is implemented via **non-blocking, lock-free queues,** with strict decoupling between match processing and emission side effects. By isolating this responsibility in a single, dedicated module, the system maintains **determinism, performance,** and **observability** without compromising core engine latency.

## Primary Responsibilities

- Receives execution events from all active `Whistle` engines
- Validates, normalize, and enrich events (e.g. add computed fields like aggressor side or maker/taker flags)
- Dispatch events to all subscribed downstream consumers
- Serialize all data according to configurable schemes (Flatbuffers, Protobuf, JSON)
- Guarantee delivery to the replay log, while allowing lossy or async delivery to UI/analytics
- Enforce non-blocking emission, never allowing backpressure to impact matching speed

| Function | Description |
| --- | --- |
| **Event Normalization** | Receives `EngineEvent`s from `Whistle`, validates, timestamps, and augments (e.g., adds `liquidity_flag`, assigns `execution_id`) |
| **Fanout Dispatch** | Emits formatted execution events to all downstream subscribers: `AnalyticsEngine`, `ReplayEngine`, `WebUI`. |
| **Queue Management** | Owns the lock-free `MPSC` queue (or equivalent) used by `Whistle` to emit engine events; ensures safe async consumption |
| **Serialization** | Translates raw events into structured formats (e.g., Flatbuffers, Cap’n Proto) for fast transmission |
| **Ordering Guarantees** | Ensures events are delivered in tick-consistent order (especially important for replay or tick-accurate visualization) |
| **Tick Completion Emission** | Emits a `TickComplete(tick_id)` marker once all events from that logical tick have been processed |
| **Diagnostics** | Tracks event latency, queue depth, throughput, dropped frames (if any), etc. Emits structured metrics for observability |

## Events Emitted

The `ExecutionManager` processes and emits a strict set of post-match event types:

| Event Type | Description |
| --- | --- |
| `ExecutionReport` | Per-order fill event (price, quantity, counterparty, timestamp) |
| `TradeEvent` | Market-leve trade tick (symbol, price, quantity, aggressor side) |
| `OrderCancelled` | User- or system-initiated cancel acknowledgement |
| `BookDelta` | Depth updates after resting order changes (insert/remove/modify) |
| `TickComplete`  | End-of-tick marker, emitted per simulation cycle |
| `SystemLog` | Optional structured diagnostics, used in debug or trace mode |

Each message is emitted with its source context and logical timestamp, ensuring downstream systems can maintain correct ordering and causality.

## Role in the System

In the broader architecture, `ExecutionManager` acts as the deterministic clearinghouse for all trading outcomes:

- Downstream of `Whistle` - receives all output from symbol-level engines
- Upstream of `ReplayEngine` , `AnalyticsEngine` , and `WebUI`
- Indirectly drives UI, strategy logic, and audit tooling by emitting canonical events

It is the bridge between the core execution kernel and the system’s observability, persistence, and intelligence layers.

---

## 2. Functional Requirements

### 2.1 Event Ingestion

- Accepts inbound events via a **lock-free, single-producer-multiple-consumer (MPSC) queue.**
- All events originate from `Whistle` instances, and include:
    - `Trade` (execution info)
    - `OrderAccepted` , `OrderRejected` , `OrderCancelled`
    - **`BookDelta`** (price level changes)
    - `TickComplete` (logical clock boundary marker)

Each event includes:

- `symbol` : the market the event belongs to
- `logical_timestamp` : tick number from `SimulationClock`
- `sequence_id` : monotonically increasing with each tick
- `execution_id` (for trades): globally unique identifier
- `side` , `price` , `quantity`, `account_id`

> ⚠️ Events are ingested from an **append-only queue**, preserving strict ordering by logical tick and input sequence.
> 

### 2.2 Event Normalization

All inbound events are:

- **Validated** for consistency (e.g., timestamps, IDs, symbol mapping)
- **Augmented** with derived fields (e.g., liquidity flag, trade aggressor side)
- **Timestamped** with emission time (simulation and optionally wall-clock)
- **Structured** into a canonical format (e.g., Flatbuffers struct or Cap’n Proto message)
- Each trade is assigned a globally unique `execution_id` using an atomic counter or monotonic generator.
- Optional: prefix with symbol or shard ID to avoid coordination overhead.

⚠️ Events must **not** be modified after normalization. They are treated as immutable atomic records from this point forward.

### 2.3 Fanout and Downstream Dispatch

Each normalized event is pushed to the appropriate subscriber queues, including:

| Destination | Channel | Notes |
| --- | --- | --- |
| `ReplayEngine` | Lock-free batch pipeline | WAL log + snapshot event stream |
| `AnalyticsEngine` | Async metrics channel | Structured event + timing data |
| `WebUI` | Shared memory or pub-sub socket | Real-time stream (e.g., trades, deltas) |

Dispatch logic guarantees:

- **Per-tick order consistency**
- **Backpressure isolation** (slow subscribers are buffered or dropped safely)
- **No blocking** in the `ExecutionManager` processing thread

### 2.4 Tick Boundary Handling

- Receives `TickComplete(tick_id)` from each Whistle and emits `TickBoundaryEvent` downstream only **after all expected symbols have submitted for tick T**.
- ExecutionManager aggregates tick completion before triggering downstream syncs (e.g., snapshots, UI flush).

This guarantees downstream systems operate on **well-formed, tick-delimited streams**.

### 2.5 Failure Isolation and Fault Tolerance

- Individual malformed or rejected events are logged but **do not block** processing.
- Subscriber failures (e.g., slow consumer) are isolated via:
    - Drop-on-overflow mode (for non-critical systems)
    - Backpressure metrics
    - Bounded buffering with metrics alerting
- Crash safety is enforced by:
    - Explicit flushing on tick boundaries
    - Structured panic recovery (e.g., for log write failures)

### 2.6 Replay Compatibility

All events are serialized using a **stable schema**, preserving:

- Field ordering and layout (version-controlled)
- Deterministic ordering (within and across ticks)
- Compact encoding (e.g., Flatbuffers or Cap’n Proto)

This allows downstream systems (ReplayEngine, diffing tools) to reconstruct the **exact event stream** with **zero loss**.

### 2.7 Observability and Metrics

ExecutionManager emits metrics such as:

| Metric | Purpose |
| --- | --- |
| `events_ingested_total` | Total event count |
| `event_latency_tick_p50/p95/p99` | Time from tick open to event flush |
| `queue_depth` | Current queue size (per subscriber) |
| `tick_duration_ns` | Time to fully flush a tick |
| `serialization_time_ns` | Time spent encoding events |

These are exported to the `AnalyticsEngine` or exposed via structured logs.

> ⚠️ Logging occurs outside the hot path. No file I/O or format strings occur in the match loop.
> 

### 2.8 Lifecycle Management

- Starts with simulation engine boot
- Dynamically registers symbols as `Whistle` instances come online
- Gracefully shuts down after all `TickComplete` markers are processed
- Emits final flush markers to all downstream systems

### 2.9 Symbol Lifecycle and Registration

- ExecutionManager maintains a registry of active symbols.
- Registration is **pushed** by the `SymbolCoordinator` during engine boot.
- De-registration occurs on Whistle shutdown.
- No events are accepted for symbols that haven't registered.

---

## 3. Non-Functional Requirements

### 3.1 Performance Targets

The `ExecutionManager` operates in the post-match path, but its throughput and latency still directly affect system visibility, replayability, and audit fidelity. It must operate with high throughput, bounded latency, and strict ordering — all without introducing any contention or blocking on the match engine.

| Metric | Target | Notes |
| --- | --- | --- |
| **Ingestion Latency (p50)** | ≤ **4 μs** | From Whistle enqueue to internal normalization |
| **Ingestion Latency (p99)** | ≤ **10 μs** | Under full system load |
| **Flush Latency (Tick Flush)** | ≤ **50 μs** | From last event of tick to final emission |
| **Tick Throughput** | ≥ **10,000 ticks/sec** | Assuming max symbol fanout |
| **Max Event Throughput** | ≥ **1,000,000 events/sec** | Across all symbols and streams |
| **Serialization Overhead** | ≤ **20%** of tick flush time | Measured with Flatbuffers pipeline |
| **Startup Time** | ≤ **2 ms** | Cold boot and symbol registration |

These are simulation-time targets. Real-time delays (e.g., disk write or WebSocket latency) are excluded unless they impact hot-path flush latency.

---

### 3.2 Determinism and Replayability

The `ExecutionManager` is the canonical source of all post-match event streams. It must guarantee full replayability and deterministic behavior:

- **Stable Output Contract**: All events follow a fixed schema and emit order (e.g., trade → book delta → cancel → tick complete).
- **Tick-Bound Ordering**: Events are guaranteed to be flushed **in full** and **in order** before the next tick starts.
- **ID Assignment**: Execution IDs are assigned consistently from a monotonic generator (per-simulation run), ensuring bitwise reproducibility.
- **No Side Effects**: Any subscriber failure must not alter event order or cause mutation.
- **Replayable Stream**: The serialized output is sufficient to reconstruct entire post-trade state (via `ReplayEngine`) with zero loss.

This ensures that any simulation — when replayed — produces exactly the same trades, state transitions, and analytics metrics.

### 3.3 MPSC Queue Implementation Strategy

Based on analysis of the current Whistle implementation, the ExecutionManager will use a **new OutboundQueue (MPSC)** for event emission:

#### 3.3.1 Queue Design
```rust
// New OutboundQueue for MPSC (Multiple Producers, Single Consumer)
pub struct OutboundQueue {
    queue: Arc<MpscQueue<EngineEvent>>,
    capacity: usize,
    backpressure_policy: BackpressurePolicy,
}

pub enum BackpressurePolicy {
    Fatal,  // System exits on overflow (recommended for data integrity)
    Drop,   // Drop events on overflow (with comprehensive metrics)
}
```

#### 3.3.2 Backpressure Policy: Fatal (Recommended)
- **Rationale**: Lost events = corrupted replay and broken determinism
- **Implementation**: System exits with error code on queue overflow
- **Metrics**: Track all backpressure events for capacity planning
- **Recovery**: Restart system with increased queue capacity

#### 3.3.3 Queue Configuration
- **Default Capacity**: 8,192 events per symbol (configurable)
- **Memory**: Pre-allocated to avoid hot-path allocation
- **Threading**: Lock-free MPSC for zero-contention emission
- **Monitoring**: Real-time queue depth and utilization metrics

---

### 3.3 Resource Usage and Isolation

| Resource | Constraint |
| --- | --- |
| **Threading** | Runs in its own async task or core-pinned thread |
| **Memory Allocation** | All queues are preallocated; no dynamic resizing in hot path |
| **Queue Depth** | Default depth of 8k events per symbol |
| **Backpressure Policy** | Bounded queues with drop-on-overflow for non-critical consumers |
| **CPU Affinity** | ExecutionManager may be bound to its own core if under load |
| **Symbol Fanout** | Handles ≥10,000 active symbols concurrently, without lock contention |

The design must support **zero backpressure** into `Whistle`, and **per-destination buffer configuration** (e.g., `ReplayEngine` is never lossy, `WebUI` can be lossy).

---

### 3.4 Failure Modes and Recovery

ExecutionManager must degrade gracefully and isolate faults:

| Failure Case | Behavior |
| --- | --- |
| **Slow Subscriber** | Buffer fills; overflow mode applies per destination policy |
| **Serialization Error** | Logs and drops event; never blocks loop |
| **Disk Flush Failure (ReplayEngine)** | Triggers fatal error and halts simulation (unless marked non-critical) |
| **Tick Incomplete** | Simulation stalls; emits diagnostics until timeout threshold |
| **Crash Mid-Tick** | Events already flushed are recoverable via WAL; rest is lost unless snapshot gated |

To support testability and operational resilience:

- All failures are logged with tick and symbol context
- Soft failure tolerance is configurable (e.g., allow UI drop, but not Replay drop)
- Tick timeouts are configurable to detect stalled engines or lost `TickComplete` signals

---

### 3.5 Observability and Debugging

ExecutionManager must expose structured metrics and tracing hooks:

| Signal | Channel |
| --- | --- |
| `tick_flush_latency_ns` | Emitted per tick |
| `events_ingested_total` | Cumulative, per symbol |
| `queue_occupancy_percent` | Per downstream consumer |
| `tick_flush_skew` | Measures inter-symbol skew before full tick flush |
| `dropped_events_total` | Per stream, per destination |
| `serialization_time_ns` | Total per batch |
| `replay_consistency_hash` (optional) | Per event for replay validation |

### 3.6 Coverage Audit vs Project Goals

| **Goal** | **Coverage** | Status |
| --- | --- | --- |
| **Performance**(< 2μs match latency) | Covered via tick flush latency, serialization time, queue metrics | Complete |
| **Determinism**(bitwise replayable) | Canonical tick-flush order, stable schemas, no hot-path mutation | Complete |
| **Throughput**(100k+ orders/sec) | Implicit via `events_ingested_total`, but no per-symbol or tick-rate metric | Partial |
| **Extensibility**(easy new bots/symbols) | Not directly tied to observability; covered elsewhere | Complete |
| **Observability**(zero overhead, full visibility) | Covered well via async logging, queue depth, flush times | Complete |
| **Testability**(unit + replay) | Partially implied (diagnostics + metrics), but not called out in metrics | Partial |
| **Modularity**(clear interfaces) | Outside scope of metrics, handled in design — fine | Complete |
| **Concurrency**(lock-free buffers) | Queue occupancy metrics = good; but not enough pressure or skew info | Partial |
| **Scalability**(dynamic symbol mgmt) | No specific metric tied to # of active symbols or tick skew | Needs Work |

These are consumed by:

- `AnalyticsEngine` (metrics ingest)
- Debug CLI (human-readable trace output)
- Test harnesses (event latency diffing, snapshot validators)

> ⚠️ Observability is never allowed to block the event loop. All instrumentation is async or emitted via diagnostic rings.
> 

## 4. Architecture and Execution Flow

### 4.1 Module Layout

The `ExecutionManager` is composed of the following internal modules:

| Module | Purpose |
| --- | --- |
| `ingest_loop`  | Pulls events from the shared MPSC queue, in logical-tick order |
| `event_normalizer` | Validates and enriches each event (e.g., add `execution_id` , `liquidity_flag` ) |
| `dispatcher` | Fanout logic - emits events to downstream subscribers: `ReplayEngine` , `AnalyticsEngine` , `WebUI` |
| `tick_tracker` | Gathers structured performance and queue statistics |

Each module is isolated, testsable, and executes in a strict, deterministic loop per tick.

### 4.2 Threading and Concurrency Model

| Component | Model |
| --- | --- |
| **Inbound Event Queue** | Lock-free MPSC queue (one per ExecutionManager instance) |
| **Processing Loop** | Single-threaded loop per `ExecutionManager` , optionally core-pinned |
| **Fanout Channels** | Lock-free bounded queues per destination (`ReplayEngine`, `AnalyticsEngine` , `WebUI` ) |
| **Tick Coordination** | Internal `tick_tracker` ensures barrier sync across symbols before `TickBoundaryEvent` is emitted |

### 4.3 Per-Tick Flow

Below defines the lifecycle for a single tick inside `ExecutionManager` :

1. **Ingest Phase**
    1. Pull events from MPSC queue while `tick_id == current_tick` 
    2. Events are batched by symbol, maintaining submission order
2. **Normalize Phase**
    1. For each event:
        1. Validate event shape and type
        2. Assign `execution_id` if it’s a trade
        3. Augment with derived fields (`liquidity_flag` , `aggressor_side` )
        4. Timestamp with logical + wall clock time
3. **Dispatch Phase**
    1. Emit to all enabled destinations:
        1. `ReplyEngine` (lossless, required)
        2. `AnalyticsEngine` (best effort)
        3. `WebUI` (lossy allowed)
4. Tick Completion
    1. Once all expected symols emit `TickComplete(tick_id)` :
        1. `tick_tracker` triggers flush barrier
        2. Emit `TickBoundaryEvent` downstream
        3. Snapshow hooks (if any) are triggered indirectly via `ReplayEngine`
5. Metrics Phase
    1. Emit
        1. Tick duration
        2. Queue depth per consumer
        3. Drop statistics
        4. Throughput for tick

### 4.4 Symbol-Aware Event Routing

The system is built to scale horizontally across the thousands of symbols. Each event is tagged with:

- `symbol_id`
- `tick_id`
- `sequence_number`

The `dispatcher` routes events using the symbol tag to appropriate subscribers. Events for inactive or unregistered symbols are rejected at ingestion.

Symbol lifecycle is coordinate via `SymbolCoordinator` , which pushes registration messages to `ExecutionManager` .

### 4.5 Output Contracts

Each subscriber has a strict delivery contract:

| Destination | Delivery Type | Notes |
| --- | --- | --- |
| `ReplayEngine` | Required / Reliable | Events must be logged; failure = fatal |
| `AnalyticsEngine` | Best-effort | Dropped if queue overflows; metrics track loss |
| `WebUI` | Lossy / Optional | Reconnectable via snapshots or catch-up stream |
| Test Harness | Configurable | May switch to required for validation mode |

Each channel has:

- Preallocated buffer (e.g., 4096-16k events)
- Overflow policy (`drop` , `block` , or `alert` )
- Serialization settings (e.g., Flatbuffers)

### 4.6 Tick Skew Handling

To avoid downstream inconsistency due ot symbol procesing skew:

- `tick_tracker` holds off on emitting `TickBoundaryEvent` until all registered symbols have submitted `TickComplete(tick_id)`
- A timeout threshold (e.g., 100μs) emits a warning if lag detected
- This ensures Replay and Analytics receive flush-consistent tick snapshots

### 4.7 Crash Recovery and Replay Safety

`ExecutionManager` guarantees:

- All flushed events are persisted after advancing ticks
- If crash occurs **mid-tick**, `ReplayEngine` can resume from last completed tick using:
    - Event logs (`tick_n.fb.zst` )
    - Snapshot (state of books, accounts)

A test harness can verify:

- Tick hashes match
- Event order is preserved
- No events lost or duplicated

---

## 5. Implementation Plan - `ExecutionManager`

This section outlines the development strategy, core interfaces, and verification methodology for the `ExecutionManager` . The module is central to observability, determinism, and data integrity across the system, and must meet stringent correctness, performance, and testability standards.

### 5.1 Development Phases

| Phase | Goal | Deliverable |
| --- | --- | --- |
| **P1: OutboundQueue Implementation** | Add MPSC queue to Whistle for event emission | New OutboundQueue + integration tests |
| **P2: Whistle Integration** | Modify Whistle.tick() to emit to queue instead of returning Vec | Updated Whistle + backward compatibility |
| **P3: ExecutionManager Core** | Implement basic ExecutionManager with queue consumption | Core ExecutionManager + event processing |
| **P4: Event Normalization** | Validate and enrich all input events; assign `execution_id` s | Deterministic transformer + coverage tests |
| **P5: Output Fanout** | Push to `ReplayEngine` , `AnalyticsEngine` , `WebUI` , etc. | Lock-free push queues; mock subscribers |
| **P6: Tick Coordination** | Aggregate `TickComplete` and emit unified `TickBoundaryEvent`  | Tick collector + downstream flush hook |
| **P7: Comprehensive Metrics** | Implement full metrics strategy from metrics-strategy.md | All component metrics + alerting |
| **P8: Fault Tolerance** | Drop handling, diagnostics, recovery mode, bounded buffers | Fatal backpressure policy + metrics |
| **P9: Serialization Strategy** | Serialize to Flatbuffers (and optionally JSON, Protobuf) | Serialization pipeline + performance tests |

### 5.2 Core Types

| Concept | Type | Notes |
| --- | --- | --- |
| `ExecutionId` | `u64` | Assigned per trade, globally unique |
| `TickId` | `u64` | Global logical tick number |
| `SymbolId` | `u32` | Compact market ID (player-based) |
| `EngineEvent` | Enum | Fro `Whistle` , normalized |
| `DispatchEvent` | Struc | Canonical outbound message (after formatting) |
| `FlushTrigger`  | Struct | `TickBoundaryEvent` metadata |

All structs must be Flatbuffers-encodable and version-safe

### 5.3 Determinism and Replay Guarantees

ExecutionManager guarantees **bitwise-deterministic output** given the same input stream:

- `execution_id` s assigned via atomic monotonic counter
- Events batched in input order, flushed per tick
- Outputs written to Flatbuffers logs before any optinoal consumers

### 5.4 Implementation Layout

```jsx
execution_manager/
├── src/
│   ├── engine.rs         // Main ingest + tick flush loop
│   ├── normalizer.rs     // Event transform logic
│   ├── dispatch.rs       // Fanout to Replay/UI/Analytics
│   ├── tick_coordinator.rs // TickComplete aggregator
│   ├── id_allocator.rs   // Execution ID logic
│   ├── metrics.rs        // Observability hooks
│   └── types.rs          // EngineEvent, DispatchEvent, etc
├── tests/
│   ├── normalizer.rs
│   ├── tick_boundary.rs
│   ├── output_contracts.rs
│   └── replay_vectors.rs
├── schema/
│   ├── events.fbs        // Flatbuffers IDL
│   └── tick.fbs
```

---

## 6. Interfaces and API

### 6.1 Public Interface (`ExecutionManager` API Surface)

The `ExecutionManager` exposes a minimal, deterministic interface to downstream systems and internal consumers. It is a **single-writer, event-drive,** and invoked only via append-only input queues and tick notifications.

```jsx
pub struct ExecutionManager {
	pub fn new(config: ExecManagerConfig) -> Self;
	pub fn register_symbol(&mut self, symbol: Symbol);
	pub fn ingest_event(&mut self, event: EngineEvent);
	pub fn notify_tick_complete(&mut self, symbol: Symbol, tick: LogicalTimestamp);
	pub fn flush_if_ready(&mut self, tick: LogicatTimestamp);
	pub fn shutdown(&mut self);
}
```

| Method | Description |
| --- | --- |
| `new` | Initializes `ExecutionManager` , allocates queues, sets config |
| `register_symbol` | Adds a symbol to the tick coordination set |
| `ingest_event` | Ingests a normalized `EngineEvent` from `Whistle` via MPSC |
| `flush_if_ready` | Emits `TickBoundaryEvent` + fans out events once all symbols have completed |
| `shutdown` | Final tick flush and safe closure of all downstream queues |

All methods are invoked on a **single-threaded loop,** maintaining full determinism.

### 6.2 Input Contract (Events from `Whistle` )

```jsx
pub enum EngineEvent {
	OrderAccepted { order_id, timestamp },
	OrderRejected { reason, order_id },
	OrderCancelled { order_id, filled_qty },
	Trade {
		buy_order_id,
		sell_order_id,
		price,
		quantity,
		aggressor_side,
		timestamp,
	},
	BookDelta {
        symbol,
        side,
        price_level,
        new_qty,
    },
    TickComplete {
        symbol,
        logical_timestamp,
    },
}
```

Each event is guaranteed to:

- Be produced from a `Whistle` instance running in isolated tick context
- Carry a symbol ID and logical tick
- Preserve strict sequence ordering

### 6.3 Output Events (To Replay, Analytics, UI)

```jsx
pub enum DispatchEvent {
	ExecutionReport {
		execution_id,
		order_id,
		price,
		quantity,
		side,
		aggressor_flag,
		timestamp,
	},
	TradeEvent {
		symbol,
		price,
		quantity,
		aggressor_side,
		logical_timestamp,
	},
	OrderCancelled {
		order_id,
		reason,
		timestamp,
	},
	BookDelta {
		symbol,
		price_level,
		side,
		delta,
	},
	TickBoundaryEvent {
		tick,
		flushed_symbols,
		timestamp,
	},
	SystemLog {
		level,
		message,
		symbol: Option<Symbol>,
		tick: Option<LogicalTimestamp>,
	},
}
```

DispatchEvents are emitted in **batch mode** per tick and routed to:

- `ReplayEngine` (required)
- `AnalyticsEngine` (optional)
- `WebUI` (optional, pub-sub or shared memory)
- `SnapshotManager` (triggered on `TickBoundaryEvent`)

### **6.4 Internal Interfaces (Dependency Injection)**

The `ExecutionManager` depends on a narrow set of async-safe sinks and interfaces:

| Dependency | Trait / Interface | Description |
| --- | --- | --- |
| `ReplayEngine` | `EventSink<DispatchEvent>` | Consumes all canonical events in order |
| `AnalyticsEngine` | `MetricSink<EventMetric>` | Asynchronous consumer of diagnostics, counters |
| `WebUI` | `PubSubSink<DispatchEvent>` | Optional push channel for trades and depth |
| `SnapshotManager` | `TickHook` | Optional callback on tick flush |
| `EngineClock` | None (tick time passed explicitly) | Logical tick ownership remains upstream |

All traits should be thread-safe (`Send + Sync`) and mockable.

### **6.5 Concurrency Model / Thread-Safety**

The `ExecutionManager` is designed for **isolated execution** and **non-blocking fanout**:

- **Input Queue:** One MPSC per instance, with all `Whistle` producers pushing into it
- **Processing Loop:** Runs as a single-threaded task (optionally core-pinned)
- **Fanout:** Downstream queues are independent and buffered; failure in one does not affect others
- **Flush Barriers:** Tick-based batching ensures causal integrity per logical tick

No internal state is shared across threads. There is **no concurrent mutation** outside the loop. Tick isolation ensures replay fidelity and prevents interleaved state.

---

## **7. Testing and Verification**

The `ExecutionManager` is a post-match, determinism-critical component responsible for routing all canonical trade events. It is verified via **deterministic event stream replay**, **fanout correctness tests**, **tick-based batch assertions**, and **fault isolation testing.**

Every output of the `ExecutionManager` must match expectations under:

- High-throughput, multi-symbol ingestion
- Deliberate downstream failures
- Replay-mode validation (bitwise match)
- Tick-aligned output stream checks

---

### **7.1 Module Tests**

| Module | Tests |
| --- | --- |
| `ingestion` | Per-symbol MPSC ingestion, out-of-order guardrails |
| `normalization` | Execution ID assignment, derived field correctness |
| `dispatch` | Fanout behavior to Replay/Analytics/UI under load |
| `tick_coordination` | Aggregation logic across multiple `Whistle` instances |
| `flush` | Guarantees correct tick boundaries under race conditions |

All modules are tested using **in-memory mocks**, with precise tick sequencing and timestamped input.

---

### **7.2 Replay Safety Tests**

A canonical test suite validates **output stream determinism** by asserting:

- Identical `DispatchEvent` sequences on repeat runs
- Tick boundaries emitted **only** when all expected symbols flush
- No mutation of normalized events during fanout

Tests include:

- Multi-symbol trade streams
- Mismatched `TickComplete` arrival orders
- Event delay scenarios (one symbol stalls)
- Replay hash checks

---

### **7.3 Fanout Fault Isolation**

Each subscriber (ReplayEngine, AnalyticsEngine, UI) is tested as a **potential failure point**. Tests simulate:

| Scenario | Expectation |
| --- | --- |
| ReplayEngine temporarily stalls | Other systems continue to receive events |
| AnalyticsEngine panics | Event stream bypasses it; no crash |
| UI queue overflow | Events dropped safely, with metrics logged |
| SnapshotManager unavailable | Tick flush still completes (optional hook) |

Faults are logged but **never block** upstream event processing or tick advancement.

---

### **7.4 Tick-Alignment Invariants**

Invariants enforced and tested:

- All output events for tick `T` are flushed **after** all `Whistle` engines submit `TickComplete(T)`
- All fanouts are triggered in **sequence-stable** order
- `TickBoundaryEvent` marks the **only permissible flush point** for downstream systems

Fuzz and property tests validate:

- No early flush
- No duplicate `TickBoundaryEvent`
- No cross-symbol output interleaving

---

### **7.5 Property-Based Testing**

Using `proptest`, the `ExecutionManager` validates behaviors under randomized tick streams:

Examples:

- If symbol `S` emits no events but does emit `TickComplete`, it still participates in the flush
- If one symbol is delayed, no partial tick is flushed
- No `DispatchEvent` may reference a non-registered symbol

All generated test cases must pass consistency and ordering rules.

---

### **7.6 Performance Benchmarks**

Benchmarks are run to ensure:

| Metric | Target |
| --- | --- |
| Event ingestion throughput | ≥ 500k events/sec |
| Tick flush latency | ≤ 1ms (p99 under load) |
| Queue backpressure overhead | < 5% CPU in worst case |
| Per-subscriber dispatch latency | < 100μs avg |
| Execution ID allocation time | < 10ns avg (atomic increment) |

All benchmarks are:

- CPU-pinned (to isolate noise)
- Repeated under load profiles (burst vs steady)
- Measured with tick boundaries for consistency

---

### **7.7 Integration Tests**

The `ExecutionManager` is tested in full-system flow with:

- `Whistle` emitting real match events
- `ReplayEngine` recording and replaying streams
- `WebUI` observing real-time depth and trades

Scenarios:

- Symbol joins mid-simulation
- Delayed `Whistle` flushes
- Tick stall detection and recovery
- Replay → snapshot restore → flush consistency

---

### **7.8 Test Infrastructure Layout**

```bash
execution_manager/
├── src/
│   ├── manager.rs
│   ├── dispatch.rs
│   ├── ingestion.rs
│   ├── tick_tracker.rs
│   └── ...
├── tests/
│   ├── unit/
│   ├── replay/
│   ├── fanout/
│   ├── fuzz/
│   └── integration/
├── benches/
│   └── dispatch_bench.rs
└── fixtures/
    ├── sample_event_streams/
    └── tick_snapshots/

```

---

## **8. Directory Layout & Build Targets**

The `ExecutionManager` is structured for clean separation of ingestion, dispatch, tick coordination, serialization, and test utilities. It follows a modular, test-driven layout with inline support for performance benchmarking and replay-mode verification.

---

### **8.1 Source Layout**

```bash
execution_manager/
├── src/
│   ├── lib.rs                 # Public interface and top-level struct
│   ├── config.rs              # Config structs (e.g., batching, flush behavior)
│   ├── event.rs               # Normalized event types, dispatchable payloads
│   ├── id_allocator.rs        # Execution ID generator (atomic or sharded)
│   ├── ingestion.rs           # MPSC receiver loop, per-symbol event intake
│   ├── normalization.rs       # Adds execution IDs, aggressor side, etc.
│   ├── dispatch/
│   │   ├── mod.rs             # Fanout coordinator
│   │   ├── replay.rs          # ReplayEngine sink logic
│   │   ├── analytics.rs       # AnalyticsEngine sink logic
│   │   ├── ui.rs              # WebUI or trace sink logic
│   ├── tick_tracker.rs        # TickComplete aggregation, flush triggers
│   ├── metrics.rs             # Queue depth, latency, and drop counters
│   └── shutdown.rs            # Graceful flush + teardown logic

```

---

### **8.2 Test Suite Layout**

```bash
execution_manager/
├── tests/
│   ├── unit/
│   │   ├── id_allocator.rs
│   │   ├── event_normalization.rs
│   │   ├── tick_tracker.rs
│   ├── replay/
│   │   ├── hash_consistency.rs
│   │   └── snapshot_flush.rs
│   ├── integration/
│   │   ├── end_to_end_fanout.rs
│   │   └── replay_compat.rs
│   ├── fuzz/
│   │   ├── randomized_tick_stream.rs
│   ├── property/
│   │   └── tick_alignment_props.rs

```

---

### **8.3 Benchmarks**

```bash
execution_manager/
├── benches/
│   ├── dispatch_bench.rs          # Dispatch throughput under varying loads
│   ├── tick_flush_bench.rs       # Tick-to-flush latency measurement
│   └── fanout_saturation.rs      # Stress fanout with slow consumers

```

> Benchmarks use Criterion or a custom CPU-pinned runner to ensure reproducibility.
> 

---

### **8.4 Fixtures and Mock Data**

```bash
execution_manager/
├── fixtures/
│   ├── input_streams/
│   │   └── test_symbols.fb.zst
│   ├── snapshots/
│   │   └── pre_flush_state/
│   └── config/
│       └── test_config.toml

```

---

### **8.5 Build Targets**

| Target | Purpose |
| --- | --- |
| `cargo build --package execution_manager` | Core library build |
| `cargo test --package execution_manager` | Full unit and integration test run |
| `cargo bench --package execution_manager` | Run microbenchmarks |
| `cargo run --example dispatch_loop` | Manual loop invocation with mock data |
| `cargo test --test replay` | Validate determinism and flush boundaries |

> All builds must compile with --no-default-features and support --release mode under CI.
> 

---

### **8.6 Feature Flags**

| Flag | Description |
| --- | --- |
| `trace_metrics` | Enable internal latency + queue logging |
| `replay_mode` | Enforces output hashing and snapshot tagging |
| `fail_on_drop` | Aborts on unhandled dispatch drops (for fuzz testing) |

---

This layout ensures that `ExecutionManager` development is **incremental, debuggable, and test-first**, while supporting long-term evolution (e.g., adding batching modes, different serialization targets, or tick flush variants).