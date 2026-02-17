> **Implementation Status:** Partially implemented. Core lifecycle management interfaces and symbol registry exist. Thread placement policy and NUMA-aware memory allocation are not yet implemented. Whistle instance creation and queue wiring are handled through simplified paths.

---

# SymbolCoordinator Design Document

## 1. Overview

The `SymbolCoordinator` is responsible for **managing the lifecycle of Whistle engine instances**, not for processing orders. It acts as the "manager" while Whistle acts as the "worker".

**Key Responsibilities:**
- **Engine Lifecycle Management**: Create, boot, and destroy Whistle instances
- **Thread Placement**: Assign symbols to threads for deterministic execution
- **Queue Wiring**: Create and wire SPSC queues between OrderRouter and Whistle
- **Symbol Registration**: Track which symbols are active and their states
- **Resource Management**: Manage thread pools and queue allocation

**Critical Integration Contract with OrderRouter:**
- OrderRouter calls `ensure_active(symbol_id)` to request symbol activation
- SymbolCoordinator creates/boots Whistle instance and returns `OrderQueueWriter`
- OrderRouter uses the queue writer to route orders to the active symbol

## 2. Architecture & Design Principles

### 2.1 Separation of Concerns

**SymbolCoordinator (Manager):**
- Manages Whistle instance lifecycle (create/boot/destroy)
- Handles thread assignment and resource allocation
- Coordinates with SimulationClock for tick delivery
- Manages symbol state transitions

**Whistle (Worker):**
- Processes orders within tick boundaries
- Manages order book and matching logic
- Emits canonical events
- Handles internal engine state

**OrderRouter (Router):**
- Routes orders to active symbols
- Stamps `enq_seq` for price-time priority
- Handles backpressure and rejection

### 2.2 Lifecycle Management Model

The SymbolCoordinator manages **Whistle instances**, not order processing:

```
OrderRouter → SymbolCoordinator → Whistle Instance
     ↓              ↓                ↓
   Routes    Manages Lifecycle   Processes Orders
```

**Symbol Lifecycle States:**
- **Unregistered**: Symbol unknown to system
- **Registered**: Whistle instance created, not yet booted
- **Active**: Whistle booted, participating in ticks
- **Evicting**: Whistle draining, no new orders
- **Evicted**: Whistle destroyed, resources freed

## 3. Core Components

### 3.1 SymbolCoordinator

**Primary Interface:**
```rust
pub trait SymbolCoordinatorApi {
    fn ensure_active(&self, symbol_id: u32) -> Result<ReadyAtTick, CoordError>;
    fn release_if_idle(&self, symbol_id: u32);
}
```

**Internal Methods:**
- `activate_symbol_internal()`: Creates and boots Whistle instance
- `get_spsc_writer()`: Returns queue writer for OrderRouter
- `update_tick()`: Updates current tick (called by SimulationClock)

### 3.2 SymbolRegistry

**Tracks Symbol State:**
- Symbol registration and metadata
- Whistle instance handles
- Thread assignments
- Activation status

### 3.3 Thread Management

**Placement Policies:**
- `RoundRobinPolicy`: Simple round-robin assignment
- `HashBasedPolicy`: Deterministic hash-based assignment
- `EngineThreadPool`: Manages thread load and symbol assignments

### 3.4 Queue Management

**QueueAllocator:**
- Creates SPSC queues for symbol activation
- Manages queue pooling and reuse
- Configurable queue depths

## 4. Integration Points

### 4.1 OrderRouter Integration

**Activation Flow:**
1. OrderRouter calls `ensure_active(symbol_id)`
2. SymbolCoordinator creates/boots Whistle if needed
3. Returns `OrderQueueWriter` for the symbol
4. OrderRouter routes orders through the queue

**Queue Handoff:**
- SymbolCoordinator creates `InboundQueue` for each symbol
- Wraps in `OrderQueueWriter` for OrderRouter
- Whistle consumes from the same queue

### 4.2 SimulationClock Integration

**Tick Delivery:**
- SymbolCoordinator registers active symbols with SimulationClock
- SimulationClock calls `tick(now)` on each active Whistle
- SymbolCoordinator updates its tick counter

### 4.3 Whistle Integration

**Engine Management:**
- Creates Whistle instances with proper configuration
- Boots engines (cold/warm start)
- Manages engine state transitions
- Handles engine cleanup and resource deallocation

## 5. Implementation Phases

### Phase 1: Core Integration (Current)
- ✅ Create SymbolCoordinator crate structure
- ✅ Implement basic types and registry
- ✅ Implement thread placement policies
- ✅ Implement queue allocation
- 🔄 Implement real symbol activation (replace placeholder)

### Phase 2: Whistle Integration
- Create and boot Whistle instances
- Wire SPSC queues between OrderRouter and Whistle
- Implement engine lifecycle management
- Add integration tests with real Whistle

### Phase 3: System Integration
- Integration with SimulationClock
- Integration with ExecutionManager
- Add comprehensive lifecycle testing
- Performance optimization

## 6. Critical Implementation Details

### 6.1 Whistle Instance Creation

**When `ensure_active()` is called:**
1. Check if symbol already exists and is active
2. If not, create new Whistle instance with proper config
3. Create SPSC queue and wire to Whistle
4. Boot the engine (cold start)
5. Register with thread pool
6. Return `OrderQueueWriter` to OrderRouter

### 6.2 Queue Wiring

**SPSC Queue Flow:**
```
OrderRouter → OrderQueueWriter → InboundQueue → Whistle
```

- SymbolCoordinator creates `InboundQueue`
- Wraps in `OrderQueueWriter` for OrderRouter
- Whistle consumes from the same queue
- No cloning or duplication of queues

### 6.3 Thread Assignment

**Deterministic Placement:**
- Hash-based assignment for consistency across runs
- Thread load balancing for performance
- Pinned execution (no migration during lifetime)

## 7. Testing Strategy

### 7.1 Unit Tests
- SymbolCoordinator creation and configuration
- Thread placement policies
- Queue allocation and management
- Symbol registry operations

### 7.2 Integration Tests
- OrderRouter + SymbolCoordinator + Whistle
- Full symbol activation and deactivation
- Queue handoff and order routing
- Tick processing and event emission

### 7.3 Performance Tests
- Symbol activation latency
- Queue throughput
- Thread pool efficiency
- Memory usage patterns

## 8. Next Steps

1. **Complete Phase 1**: Implement real symbol activation logic
2. **Begin Phase 2**: Create Whistle instances and wire queues
3. **Add Integration Tests**: Verify OrderRouter ↔ SymbolCoordinator ↔ Whistle flow
4. **Performance Validation**: Ensure sub-microsecond activation latency

---

**Key Principle**: SymbolCoordinator manages Whistle instances, not order processing. Whistle handles the execution, SymbolCoordinator handles the lifecycle.
