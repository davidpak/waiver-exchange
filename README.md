# The Waiver Exchange — Workspace

High-discipline Rust workspace for the **Whistle** matching engine: deterministic, test-first, and performance-driven.

---

## Quickstart

```bash
# Build & test everything
cargo build --workspace
cargo test  --workspace
```
### Verify-only (what CI runs)
```
cargo fmt   --all -- --check      # no diffs allowed
cargo clippy --workspace -- -D warnings
```
### Fix locally (before committing)
```
cargo fmt   --all                  # writes formatting changes
cargo clippy --workspace -- -D warnings
cargo test  --workspace
```

Tip: Keep formatting stable on stable Rust. Avoid nightly-only rustfmt options
(e.g., group_imports = "StdExternalCrate"). If you see a rustfmt warning in CI, run
cargo fmt --all, commit, and push.

## Developer Workflow (before push/PR)

**📖 For detailed development guidelines, see [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md)**

### 1. Format
```
cargo fmt --all
```

### 2. Lint as errors

```
cargo clippy --workspace -- -D warnings
```


### 3. Tests

```
cargo test --workspace
```


### 4. (Optional) Supply chain

```
cargo deny check
cargo audit -D warnings
```

### Git hooks (auto-help)
One-time:
```
chmod +x scripts/pre-commit.sh
ln -sf "$(pwd)/scripts/pre-commit.sh" .git/hooks/pre-commit
```
### Branching & PRs

* Create a short-lived feature branch: `git switch -c feat/<topic>`

* Run the steps above

* Push and open a PR to main

* CI must be green (fmt/clippy/tests, deny/audit)

### Benchmarks
```
cargo bench -p whistle-bench
# HTML report: target/criterion/<bench>/report/index.html
```
### Repository Structure
```
engine/whistle           # core engine (library)
engine/whistle-bench     # Criterion benchmarks (no deps in hot path)
exe/execution-manager    # downstream sink (placeholder)
exe/simclock             # logical tick driver (placeholder)
tools/replay             # replay checker (placeholder)
tools/whistle-playground # session-based CLI for testing Whistle
tools/whistle-monitor    # real-time dashboard and session engine
docs/adr                 # ADRs: determinism, rejects, event sequencing, etc.
```

## 🚀 **Session-Based Trading System**

The Waiver Exchange now includes a complete session-based trading system with multi-account support and real-time monitoring.

### **Quick Start: Complete Trading Session**

1. **Create a Trading Session**:
   ```bash
   cargo run --bin whistle-playground -- create-session my-trading --accounts 5
   ```

2. **Start the Real-time Monitor**:
   ```bash
   cargo run --bin whistle-monitor -- start-session my-trading --display dashboard
   ```

3. **Submit Orders as Different Accounts**:
   ```bash
   # Account 1: Market maker
   cargo run --bin whistle-playground -- submit my-trading --account-id 1 --side buy --order-type limit --price 150 --qty 20
   cargo run --bin whistle-playground -- submit my-trading --account-id 1 --side sell --order-type limit --price 155 --qty 20
   
   # Account 2: Takes liquidity
   cargo run --bin whistle-playground -- submit my-trading --account-id 2 --side buy --order-type market --qty 5
   
   # Account 3: Places limit order
   cargo run --bin whistle-playground -- submit my-trading --account-id 3 --side sell --order-type limit --price 160 --qty 10
   ```

4. **Watch Real-time Updates**: The dashboard shows live order book changes, trade executions, and session statistics.

### **Key Features**

- **🎯 Multi-Account Trading**: Switch between accounts and trade as different users
- **📊 Real-time Dashboard**: Beautiful terminal UI with live order book updates
- **🔄 Session Management**: Create, join, and manage trading sessions
- **📈 Account Status**: View active orders, recent trades, and positions
- **🎨 Color-coded Trades**: Green for buys, red for sells (like real exchanges)
- **⚡ Smart Updates**: Dashboard only updates when there are actual changes

### **System Architecture**

```
┌─────────────────┐    File-based    ┌─────────────────┐
│   Playground    │ ◄──────────────► │  Session Engine │
│   (Client)      │   Communication  │   (Monitor)     │
└─────────────────┘                  └─────────────────┘
        │                                      │
        │ Writes orders to                     │ Reads orders from
        │ orders.jsonl                         │ orders.jsonl
        │                                      │
        │ Reads responses from                 │ Writes responses to
        │ responses.jsonl                      │ responses.jsonl
```

### **Documentation**

- **[Whistle Playground](tools/whistle-playground/README.md)** - Session and account management
- **[Whistle Monitor](tools/whistle-monitor/README.md)** - Real-time dashboard and monitoring
- **[Whistle Engine](engine/whistle/README.md)** - Core matching engine

### Interactive Testing

Test the Whistle engine interactively with the playground tool:

```bash
# Quick demo
cargo run --bin whistle-playground demo --symbol 42

# Interactive session
cargo run --bin whistle-playground interactive

# Custom configuration
cargo run --bin whistle-playground interactive \
  --symbol 42 \
  --price-floor 100 \
  --price-ceil 200 \
  --tick-size 5
```

Perfect for testing new features, debugging, and learning how the engine works!

### Quality Gates (Local & CI)

* `cargo fmt --all -- --check` (no diffs in CI)

* `cargo clippy --workspace -- -D warnings`

* `cargo test --workspace`

* `cargo deny check and cargo audit -D warnings`

The commands above are mirrored in CI so your local pass == green PR.