# The Waiver Exchange — Workspace

High-discipline Rust workspace for the **Whistle** matching engine: deterministic, test-first, and performance-driven.

## Quickstart
```bash
# Pin toolchain (edit rust-toolchain.toml to exact version when ready)
cargo build --workspace
cargo test  --workspace
cargo fmt   --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```
## Benchmarks
```
cargo bench -p whistle-bench
# HTML report: target/criterion/<bench>/report/index.html
```

## Structure
```
engine/whistle           # core engine (library)
engine/whistle-bench     # Criterion benchmarks (no deps in hot path)
exe/execution-manager    # downstream sink (placeholder)
exe/simclock             # logical tick driver (placeholder)
tools/replay             # replay checker (placeholder)
docs/adr                 # ADRs: determinism, data/rejects, event sequencing
```

## Quality Gates

* fmt + clippy -D warnings locally and in CI

* cargo-audit & cargo-deny in CI

* Benchmarks tracked via docs/perf-baselines.json