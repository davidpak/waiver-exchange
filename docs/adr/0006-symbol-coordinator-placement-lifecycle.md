# 0006. SymbolCoordinator Placement & Lifecycle
- Status: Accepted
- Date: 2025-08-16

## Context
Each symbol has one Whistle engine with isolation, predictable latency, and resource control. See spec §2.6 and Threading Model.

## Decision
**Lifecycle states:** Idle → Booting → Running → (StopRequested) → Draining → Stopped; Faulted → Quarantine (policy).  
**Spawn:** On first routed order; queues pre-allocated; thread pinned; NUMA-local allocation.  
**Participation:** Engine is registered with SimulationClock before next tick; never migrates threads during lifetime.  
**Eviction:** If inactive for `evict_after = X ms` (config), stop intake, drain to TickComplete, deregister, free.  
**Placement policy:**  
- Hot symbols: dedicated threads (CPU affinity, optionally isolated core).  
- Cold/bursty symbols: async task pool.  
- NUMA: allocate engine memory on the node of the assigned CPU.  
**Backpressure:** SPSC inbound full ⇒ upstream Reject(Backpressure). No blocking.  
**Fault policy:** Invariant breach or WAL overflow ⇒ mark Faulted; Coordinator either evicts or snapshot-restart per policy.

## Consequences
Predictable latency, clear isolation/failure handling, and deterministic scheduling compatible with tick-bounded execution.