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
docs/adr                 # ADRs: determinism, rejects, event sequencing, etc.
```

### Quality Gates (Local & CI)

* `cargo fmt --all -- --check` (no diffs in CI)

* `cargo clippy --workspace -- -D warnings`

* `cargo test --workspace`

* `cargo deny check and cargo audit -D warnings`

The commands above are mirrored in CI so your local pass == green PR.