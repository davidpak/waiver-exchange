# Development Guidelines

This document outlines the development workflow, standards, and best practices for the Waiver Exchange project.

## Pre-Push Checklist

Before pushing any changes, ensure you've completed the following:

### 1. Format Code
```bash
# Core engine components only (recommended for development)
cargo fmt --package whistle --package order-router --package symbol-coordinator --package execution-manager --package analytics-engine

# Or format all components (may have formatting issues in tools)
cargo fmt --all
```

### 2. Lint as Errors
```bash
# Core engine components only (recommended for development)
cargo clippy --workspace --all-targets --exclude admin-cli -- -D warnings

# Or run on all components (may have warnings in tools)
cargo clippy --workspace -- -D warnings
```

### 3. Run All Tests
```bash
cargo test --workspace
```

### 4. Verify No Formatting Diffs
```bash
# Core engine components only (recommended for development)
cargo fmt --package whistle --package order-router --package symbol-coordinator --package execution-manager --package analytics-engine -- --check

# Or check all components (may have formatting issues in tools)
cargo fmt --all -- --check
```

### 5. License Compliance (REQUIRED)
```bash
# Check license compliance
cargo deny check licenses

# Full supply chain security check
cargo deny check
```

### 6. Optional: Additional Security
```bash
# Ignore known unmaintained dependencies in tools
cargo audit --ignore RUSTSEC-2024-0436 -D warnings

# Or audit all components (may have warnings in tools)
cargo audit -D warnings
```

## Test-Driven Development (TDD) Standards

**⚠️ CRITICAL: All new features MUST follow TDD approach**

### TDD Workflow (Mandatory)

1. **Documentation First**
   - Update design docs (`docs/design/`) to reflect new requirements
   - Document expected behavior, inputs, outputs, and edge cases
   - Update ADRs if architectural decisions are needed

2. **Test Planning**
   - Write test specifications before any implementation
   - Define expected inputs, outputs, and behavior
   - Document test scenarios and edge cases
   - Plan integration test requirements

3. **Test Implementation**
   - Write failing tests first (Red phase)
   - Ensure tests capture all documented requirements
   - Include edge cases and error conditions
   - Test both happy path and failure scenarios

4. **Implementation**
   - Write minimal code to make tests pass (Green phase)
   - Refactor while keeping tests green (Refactor phase)
   - Ensure implementation matches documented specifications

5. **Validation**
   - Verify all tests pass
   - Ensure documentation remains accurate
   - Update documentation if implementation reveals new insights

### Test Requirements

**For CRITICAL Components (Whistle, OrderRouter, ExecutionManager):**
- ✅ **Unit tests** for all public APIs
- ✅ **Integration tests** for component interactions
- ✅ **Property-based tests** for invariants (price-time priority, determinism)
- ✅ **Performance tests** for latency/throughput requirements
- ✅ **Replay tests** for determinism validation

**Test Categories:**
1. **Functional Tests** - Verify correct behavior
2. **Invariant Tests** - Verify system invariants (price-time priority, canonical ordering)
3. **Edge Case Tests** - Boundary conditions, error handling
4. **Performance Tests** - Latency/throughput requirements
5. **Determinism Tests** - Replay stability across runs

### Test Documentation Standards

Every test must include:
```rust
#[test]
fn descriptive_test_name() {
    // GIVEN: Setup and preconditions
    let cfg = EngineCfg { /* ... */ };
    let mut eng = Whistle::new(cfg);
    
    // WHEN: Action being tested
    let events = eng.tick(100);
    
    // THEN: Expected outcomes
    assert_eq!(events.len(), 2);
    // ... more assertions
}
```

## Documentation-First Development

**⚠️ CRITICAL: Documentation is the source of truth**

### Documentation Update Requirements

1. **Before Implementation**
   - Update design docs to reflect new requirements
   - Document expected behavior, interfaces, and constraints
   - Update ADRs for architectural decisions

2. **During Implementation**
   - Keep documentation in sync with implementation
   - Update docs when implementation reveals new insights
   - Document any deviations from original design

3. **After Implementation**
   - Verify documentation accuracy
   - Update examples and code snippets
   - Ensure all interfaces are documented

### Documentation Standards

**Design Documents (`docs/design/`):**
- ✅ **Clear interfaces** with type signatures
- ✅ **Behavioral specifications** with examples
- ✅ **Invariant definitions** and constraints
- ✅ **Error handling** and edge cases
- ✅ **Performance requirements** and SLOs

**Architecture Decision Records (`docs/adr/`):**
- ✅ **Context** and problem statement
- ✅ **Decision** with rationale
- ✅ **Consequences** and trade-offs
- ✅ **Implementation notes** and examples

## Implementation Standards

### Code Quality Requirements

1. **Follow Rust Best Practices**
   - Use `cargo clippy` recommendations
   - Follow Rust naming conventions
   - Use appropriate error types and Result handling

2. **Performance Requirements**
   - No heap allocation in hot paths
   - Bounded memory usage
   - Deterministic execution
   - Meet latency SLOs (p50 ≤ 1.0 μs, p99 ≤ 3.0 μs)

3. **Safety Requirements**
   - No unsafe code without justification
   - Proper error handling and validation
   - Thread safety where required
   - Memory safety and no undefined behavior

### Component Development Checklist

**Before implementing any component:**

1. **Documentation Review**
   - [ ] Design doc exists and is complete
   - [ ] Interfaces are clearly defined
   - [ ] Behavioral requirements are specified
   - [ ] Performance requirements are documented

2. **Test Planning**
   - [ ] Test scenarios are documented
   - [ ] Edge cases are identified
   - [ ] Integration test requirements are clear
   - [ ] Performance test criteria are defined

3. **Implementation Planning**
   - [ ] Data structures are designed
   - [ ] Algorithms are specified
   - [ ] Error handling is planned
   - [ ] Performance optimizations are identified

## License Requirements

### Adding New Crates/Packages

**⚠️ CRITICAL: Every new crate MUST have a license specified in its `Cargo.toml`**

When creating a new crate (like `tools/admin-cli` or `engine/analytics-engine`), you **must** add:

```toml
[package]
name = "your-crate-name"
version = "0.1.0"
edition = "2021"
license = "Apache-2.0 OR MIT"  # ← REQUIRED!
```

### Allowed Licenses

The project allows these licenses (defined in `deny.toml`):
- `Apache-2.0` - Apache License 2.0
- `MIT` - MIT License  
- `BSD-2-Clause` - BSD 2-Clause License
- `BSD-3-Clause` - BSD 3-Clause License
- `ISC` - ISC License
- `MPL-2.0` - Mozilla Public License 2.0
- `Unicode-DFS-2016` - Unicode License Agreement
- `Unicode-3.0` - Unicode License

### License Compliance Checklist

Before pushing any new crate:
1. ✅ **Add license field** to `Cargo.toml`
2. ✅ **Use allowed license** from the list above
3. ✅ **Run license check** with `cargo deny check licenses`
4. ✅ **Verify no violations** in CI

### Common License Errors & Solutions

**Error:** `admin-cli = 0.1.0 is unlicensed` or `analytics-engine = 0.1.0 is unlicensed`
- **Solution:** Add `license = "Apache-2.0 OR MIT"` to `Cargo.toml`

**Error:** `colored v2.2.0 license = "MPL-2.0" rejected: license is not explicitly allowed`
- **Solution:** Add `"MPL-2.0"` to the `allow` list in `deny.toml`

## Common Issues & Solutions

### Formatting Issues

**Problem:** CI fails with formatting diffs
**Solution:** Run `cargo fmt --package whistle --package order-router --package symbol-coordinator --package execution-manager --package analytics-engine` before pushing

### Linting Issues

**Problem:** CI fails with clippy warnings
**Solution:** Run `cargo clippy --workspace --all-targets --exclude admin-cli -- -D warnings` and fix all warnings

### Test Failures

**Problem:** Tests fail locally or in CI
**Solution:** 
1. Run `cargo test --workspace` locally
2. Check for flaky tests or timing issues
3. Ensure deterministic behavior
4. Update tests if requirements changed

### License Violations

**Problem:** CI fails with license check errors
**Solution:**
1. Add missing license to `Cargo.toml`
2. Add new license to `deny.toml` if needed
3. Run `cargo deny check licenses` locally

### Performance Regressions

**Problem:** Performance tests fail
**Solution:**
1. Run performance benchmarks locally
2. Identify the regression
3. Optimize hot paths
4. Update baseline if requirements changed

## Always Run Full CI Suite Locally

```bash
# This is what CI runs - run it locally first!
cargo fmt --package whistle --package order-router --package symbol-coordinator --package execution-manager --package analytics-engine -- --check
cargo clippy --workspace --all-targets --exclude admin-cli -- -D warnings
cargo test --workspace
cargo audit --ignore RUSTSEC-2024-0436 -D warnings
cargo deny check licenses  # ← Don't forget license compliance!
```

**CI will fail if any of these fail:**
- ❌ `cargo fmt --package whistle --package order-router --package symbol-coordinator --package execution-manager --package analytics-engine -- --check` (formatting diffs)
- ❌ `cargo clippy --workspace --all-targets --exclude admin-cli -- -D warnings` (linting errors)
- ❌ `cargo test --workspace` (test failures)
- ❌ `cargo audit --ignore RUSTSEC-2024-0436 -D warnings` (security vulnerabilities)
- ❌ `cargo deny check licenses` (license violations)

## Before Every Push

```bash
# 1. Run the full checklist
cargo fmt --package whistle --package order-router --package symbol-coordinator --package execution-manager --package analytics-engine
cargo clippy --workspace --all-targets --exclude admin-cli -- -D warnings
cargo test --workspace
cargo audit --ignore RUSTSEC-2024-0436 -D warnings
cargo deny check licenses  # ← License compliance check
cargo fmt --package whistle --package order-router --package symbol-coordinator --package execution-manager --package analytics-engine -- --check
```

```bash
# 2. Stage and commit
git add .
git commit -m "type(scope): description"

# 3. Push
git push
```

## Git Workflow

### Commit Message Format

Use conventional commits format:
```
type(scope): description

- type: feat, fix, docs, style, refactor, test, chore
- scope: component name (whistle, router, etc.)
- description: concise summary of changes
```

### Branch Strategy

1. **main** - Production-ready code
2. **develop** - Integration branch for features
3. **feature/*** - Individual feature branches
4. **hotfix/*** - Critical bug fixes

### Code Review Process

1. **Self-review** before creating PR
2. **Peer review** required for all changes
3. **Documentation review** for design changes
4. **Test coverage** verification
5. **Performance impact** assessment

## Troubleshooting

### Common Build Issues

**Problem:** `cargo build` fails with linking errors
**Solution:** 
1. Check Rust toolchain version
2. Clean and rebuild: `cargo clean && cargo build`
3. Update dependencies if needed

**Problem:** Tests fail intermittently
**Solution:**
1. Check for race conditions
2. Ensure deterministic behavior
3. Add proper synchronization
4. Use deterministic test data

### Performance Issues

**Problem:** Performance benchmarks fail
**Solution:**
1. Profile the hot path
2. Check for allocations in hot loops
3. Verify cache locality
4. Update baseline if requirements changed

### Documentation Issues

**Problem:** Documentation is out of sync
**Solution:**
1. Update design docs to match implementation
2. Verify all interfaces are documented
3. Add examples and code snippets
4. Review ADRs for accuracy
