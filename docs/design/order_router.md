> **Implementation Status:** Partially implemented. Shard mapping and basic routing interfaces exist. Full routing logic (enq_seq stamping, backpressure handling, symbol activation coordination) is not yet implemented. Order routing currently handled directly in OrderGateway.

---

## OrderRouter — Design (v0.1)

Owner: David Pak

Status: Draft → Accept upon merge

Scope: Per-symbol ingress routing, sequencing, and lifecycle triggers between OrderGateway and `Whistle`.

Audience: Engineers and reviewers building ingress and engine coordination.

Normative language: MUST / SHOULD / MAY

---

### 1. Overview & Role in System

The `OrderRouter` accepts normalized, validated ingress messages (from `OrderGateway`), assigns per-symbol enqueue sequence numbers, and delivers them to the correct symbol’s SPSC queue. It ensures deterministic per-symbol ordering and enforces non-blocking backpressure policies. The router also participates in basic symbol lifecycle: activating engines on demand (with prewarm) and signaling deactivation via the `SymbolCoordinator` at tick boundaries.

Responsibilities:
- Route each inbound message to the correct symbol’s SPSC queue
- Stamp deterministic `enq_seq` per `(symbol, tick)`
- Enforce non-blocking backpressure (full ⇒ Reject(Backpressure))
- Trigger symbol activation on first activity; cooperate with coordinator for lifecycle
- Provide shard-stable symbol placement (if multiple router instances)

Out of scope:
- Network/Auth (OrderGateway)
- Matching/book updates (Whistle)
- Global event ordering and fan-out (ExecutionManager)
- Risk admission logic (AccountService)

---

### 2. Invariants (Non-Negotiable)

1) One engine per symbol; a symbol’s SPSC has exactly one producer (its assigned router shard).
2) Tick-bounded execution: router sequencing supports `Whistle.tick(T)`; no mid-tick reordering per symbol.
3) Price-time fairness: tie-break remains `(ts_norm, enq_seq)`; router only stamps `enq_seq`.
4) Non-blocking backpressure: SPSC full ⇒ Reject(Backpressure); router never blocks.
5) Canonical event order downstream is preserved by Whistle/ExecMgr; router must not perturb symbol-local message order.
6) Determinism: Given same inputs, config, shard mapping, and tick schedule, outputs are identical.

---

### 3. Functional Requirements

- FR1: Route messages by `symbol_id` to that symbol’s SPSC.
- FR2: Stamp `enq_seq: u32` monotonically per `(symbol, tick)`; reset on tick boundary handshake.
- FR3: On SPSC full, return `Reject(Backpressure)` without blocking.
- FR4: On first message for an inactive symbol, request activation from `SymbolCoordinator` (prewarm may already have activated it).
- FR5: Support per-account/actor latency-normalized timestamps (`ts_norm`) passed through unchanged.
- FR6: Expose lightweight metrics: enqueued, rejected (by reason), queue depth snapshot.

---

### 4. Non-Functional Requirements

- Performance: ≤ 300 ns p50 enqueue path on hot symbols; zero allocations on steady state.
- Throughput: ≥ 500k msgs/s per router shard on commodity cores (ingress only).
- Isolation: No locks on per-symbol hot path; fixed-size rings.
- Determinism: Stable shard mapping and sequence assignment across runs.

---

### 5. Interfaces & Message Shapes

Input (from `OrderGateway`):
- `InboundMsg { kind, submit?, cancel?, ts_norm: TsNorm, meta, /* plus symbol_id upstream */ }`
- Router requires `symbol_id` and `tick_now` (or tick boundary callback) to maintain per-tick sequencing.

Output (to `Whistle` over SPSC):
- Same `InboundMsg` with `enq_seq: EnqSeq` stamped by router.

Errors (synced with reject model):
- `Reject(Backpressure)` when SPSC (and any router-side micro-buffer) are full.
- `Reject(SymbolInactive)` only if policy forbids on-demand activation (disabled by default).

Coordinator API (conceptual):
- `ensure_active(symbol_id) -> ReadyAtTick(T_next)`
- `release_if_idle(symbol_id)` (eviction policy owned by coordinator)

---

### 5.1 Component Interactions & Contracts

This section formalizes the contracts between `OrderRouter` and peer components. All interactions that can affect ordering are aligned to tick boundaries to preserve determinism.

Data-plane vs control-plane (explicit):
- Data-plane: `OrderRouter → SPSC(per-symbol) → Whistle`. The `SymbolCoordinator` only wires/owns lifecycle; it is not in the runtime enqueue path.
- Control-plane: Activation/placement/eviction and tick-boundary visibility happen via `SymbolCoordinator`.

1) OrderGateway → OrderRouter (ingress)
- Contract: Gateway sends normalized, validated envelopes with `symbol_id`, `ts_norm`, and actor metadata. No blocking retries.
- API (conceptual): `route(msg: InboundMsgWithSymbol, tick_now: TickId) -> Result<(), RejectReason>`
- Guarantees: Router stamps `enq_seq`, preserves `ts_norm`, and on overload returns `Reject(Backpressure)` deterministically.
- Notes: Admission control (token buckets/quotas) SHOULD be enforced in Gateway to reduce router drops.

2) OrderRouter ↔ SymbolCoordinator (lifecycle)
- Contract: Router requests activation; Coordinator owns engine creation, queue wiring, and placement. Visibility to Whistle occurs at tick boundaries.
- API: `ensure_active(symbol_id) -> ReadyAtTick(T_next) | Err(Capacity)`, `notify_activity(symbol_id, tick_now)`, `release_if_idle(symbol_id)`.
- Guarantees: Router never produces to a symbol without a wired SPSC. Coordinator ensures a single producer binding per symbol.

3) OrderRouter → Whistle (SPSC)
- Contract: Single-producer, single-consumer ring per symbol (producer = router shard; consumer = Whistle).
- Data: `InboundMsg { kind, submit?, cancel?, ts_norm, enq_seq, meta }`.
- Guarantees: Non-blocking enqueue; on full, router reports `Reject(Backpressure)` to caller. `enq_seq` monotonic per `(symbol, tick)`.

4) SimulationClock → OrderRouter (tick boundary)
- Contract: Router receives tick boundary notifications to reset `enq_seq` and rotate per-tick counters.
- API: `on_tick_boundary(symbol_id, T_next)` or broadcast `on_tick_boundary(T_next)`.
- Guarantees: `enq_seq` resets to 0 for each `(symbol, T_next)` before any new admissions for that tick.

5) ExecutionManager (FYI)
- Router does not interact directly. Sequencing emitted by Whistle relies on `(ts_norm, enq_seq)`; ExecMgr preserves canonical ordering of outcomes.

6) AccountService (FYI)
- Router is pass-through for `ts_norm` and actor/account metadata. Admission/risk verdicts are not consulted by Router.

---

### 5.2 Concrete Type Signatures

These are conceptual Rust signatures to freeze contracts. Exact module placement may vary.

// Gateway → Router
```rust
pub struct InboundMsgWithSymbol {
    pub symbol_id: u32,
    pub msg: InboundMsg, // from whistle::messages (Submit/Cancel, ts_norm, meta)
}

pub trait OrderRouterApi {
    fn route(&self, tick_now: TickId, m: InboundMsgWithSymbol) -> Result<(), RejectReason>;
}
```

// Router → Whistle (SPSC payload)
```rust
// Enriched before enqueue; same as engine expects
pub struct InboundMsg {
    pub kind: MsgKind,
    pub submit: Option<Submit>,
    pub cancel: Option<Cancel>,
    pub ts_norm: TsNorm,
    pub enq_seq: EnqSeq, // stamped by router
    pub meta: u64,
}
```

// Router ↔ Coordinator
```rust
pub trait SymbolCoordinatorApi {
    fn ensure_active(&self, symbol_id: u32) -> Result<ReadyAtTick, CoordError>;
    fn release_if_idle(&self, symbol_id: u32);
}

pub struct ReadyAtTick { pub next_tick: TickId }
pub enum CoordError { Capacity, Faulted, Unknown }
```

// Clock → Router boundary callback
```rust
pub trait TickBoundaryNotify {
    fn on_tick_boundary(&self, symbol_id: u32, next_tick: TickId);
}
```

---

 

### 6. Symbol Identity & Sharding

Symbol ID derivation:
- `symbol_id = SipHash64(PlayerUID, fixed_key)`; PlayerUID is a canonical tuple (e.g., league_id, season_epoch, player_guid).
- Collision policy: Maintain `symbol_id ↔ PlayerUID` registry; on collision, deterministically rehash with salt=1..N and persist mapping.

Sharding (multiple routers):
- Deterministic mapping: `router_shard = symbol_id % num_router_shards`.
- Guarantee: Each symbol has exactly one producing router → one producer per SPSC.

---

### 7. Activation & Lifecycle

Policy (hybrid):
- Prewarm: Coordinator boots engines for top-K hot symbols at startup (configurable), wiring SPSC/MPSC in advance.
- On-demand: First routed message for an inactive symbol triggers `ensure_active`; enqueue begins once SPSC is wired for the next tick window. If capacity for buffering is exceeded before readiness → reject.
- Eviction: Coordinator demotes/evicts engines after idle timeout; router remains stateless besides per-symbol counters.

Boundary rules:
- All start/stop visibility to Whistle occurs at tick boundaries; router’s `enq_seq` resets at `T+1` upon boundary tick callback.

---

### 8. Queues, Sizing & Backpressure

Queues:
- Per-symbol SPSC (producer: router shard; consumer: Whistle).
- Optional router-side micro-buffer (lock-free) for brief pre-activation bursts; bounded and allocation-free.

Sizing:
- Distinct knobs: `spsc_depth` (ingress capacity) and `batch_max` (Whistle per-tick work cap).
- Guideline: `spsc_depth = ceil(arrival_rate_per_tick * burst_window_ticks) + headroom`; default headroom 25–50%.

Backpressure (never block):
- Tier 1 (Gateway): token bucket per account + global shaping; early, explicit rejects under surge.
- Tier 2 (Router): If micro-buffer and SPSC are full ⇒ `Reject(Backpressure)`.
- Tier 3 (Engine): SPSC full ⇒ `Reject(Backpressure)`.

Surge handling (millions of reqs):
- Horizontal scale: Increase router shards; shard by `symbol_id`.
- Prewarm hot symbols on each shard; size `spsc_depth` accordingly.
- Accept deterministic, explicit rejects under extreme overload to preserve latency and fairness.

---

### 9. Sequencing & Time

- Router stamps `enq_seq` per `(symbol, tick)`; resets each tick.
- Priority remains `(ts_norm, enq_seq)`; router does not modify `ts_norm`.
- With multiple shards, ensure a single producer per symbol to avoid distributed sequence.

---

#### 9.1 Sequencing Examples

Example A — Two orders same tick, same symbol:
1) Gateway sends O1(ts_norm=100) then O2(ts_norm=100), router stamps enq_seq=0,1.
2) Whistle prioritizes by `(ts_norm, enq_seq)` ⇒ O1 before O2.
3) At tick boundary T+1, router resets enq_seq to 0.

Example B — Cancel vs Fill race in same tick:
1) Submit O3(ts_norm=200, enq_seq=2); then Cancel(O3)(ts_norm=199, enq_seq=3).
2) Whistle applies `(ts_norm, enq_seq)` ⇒ cancel wins (199,3) < (200,2), consistent with docs.

Example C — Multi-shard safety:
1) `router_shard = symbol_id % S` ensures one producer for a symbol’s SPSC.
2) No cross-shard merges for the same symbol; sequencing remains local and deterministic.

---

### 10. Observability (Off Hot Path)

- Counters: enqueued, rejected (by reason), per-symbol queue depth snapshots, activation requests.
- Timers: enqueue latency, activation latency (request → ready at boundary).
- Never log/format in the hot path; emit via async diagnostics.

---

### 11. Configuration Knobs

- `num_router_shards`
- `prewarm_top_k`
- `spsc_depth_default`, optional per-symbol overrides
- `burst_window_ticks`, `headroom_percent`
- `activation_policy = prewarm|on_demand|hybrid`
- `idle_timeout_ticks`

---

### 12. Failure Modes & Expected Behavior

| Scenario | Behavior |
| --- | --- |
| SPSC full | `Reject(Backpressure)`; never block |
| Micro-buffer full pre-activation | `Reject(Backpressure)` |
| Coordinator at capacity | `Reject(SymbolCapacity)` (explicit) |
| Shard misconfig (two producers) | Configuration error; refuse second producer |

---

### 13. Testing & Acceptance

Unit tests:
- Routing by symbol; enq_seq monotonic reset per tick; SPSC full rejects.

Integration tests:
- Router ↔ Coordinator ↔ Whistle: on-demand activation, prewarm coverage, eviction at boundary.
- Multi-shard determinism: stable shard mapping and no cross-producer symbols.

Property tests:
- Determinism of `(ts_norm, enq_seq)` ordering under permutations.

Performance tests:
- Enqueue p50/p99 under configured depths; surge with token-bucket shaping.

Done when:
- Deterministic ordering and reject behaviors are covered by tests.
- Prewarm + on-demand activation paths validated.
- Bench data meets target envelopes.

---

#### 13.1 Testing & Perf Targets (Expanded)

Unit tests:
- `route()` stamps monotonic `enq_seq` per `(symbol, tick)` and resets on boundary.
- SPSC full causes `Reject(Backpressure)`; no blocking.
- Inactive symbol triggers `ensure_active()` once; no duplicate calls per tick.

Integration tests:
- Prewarm path: pre-activated symbols accept immediately; enq_seq starts at 0 for the first tick.
- On-demand: first message triggers activation; if enqueue occurs before readiness and buffers fill ⇒ rejects are deterministic.
- Multi-shard: shard map stable; exactly one producer per symbol; no data races.

Property tests:
- Permutations of arrival order with same `(ts_norm, enq_seq)` constraints yield identical engine outcomes.

Performance targets:
- Enqueue p50 ≤ 300 ns, p99 ≤ 900 ns with `spsc_depth ≥ 1024` on hot path.
- Sustained ≥ 500k msgs/s per shard on modern x86 core with LTO, release.
- Surge profile: tolerate 10× burst for `burst_window_ticks` without exceeding 1% rejects, given upstream token-bucket shaping.

Configuration defaults (suggested):
- `spsc_depth_default = 2048`, `batch_max = 1024`, `burst_window_ticks = 4`, `headroom_percent = 50`.
- `prewarm_top_k = 128` (env dependent), `idle_timeout_ticks = 1000`.

### 14. Open Questions / Future Work

- Coordinated tick boundary signaling from `SimulationClock` to reset enq_seq without coupling.
- Optional per-actor latency profiles attached upstream (router is pass-through).
- Per-symbol dynamic `spsc_depth` adjustments at boundaries (policy-gated) without violating determinism.


