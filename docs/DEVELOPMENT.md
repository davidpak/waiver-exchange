# Development Guide

## Pre-Push Checklist (MANDATORY)

**⚠️ ALWAYS run these commands before pushing to prevent CI failures:**

### 1. Format Code
```bash
cargo fmt --all
```

### 2. Lint as Errors
```bash
cargo clippy --workspace -- -D warnings
```

### 3. Run All Tests
```bash
cargo test --workspace
```

### 4. Verify No Formatting Diffs
```bash
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
# If cargo-audit is installed  
cargo audit -D warnings
```

## Why CI Fails vs Local

**Common causes of CI failures:**

1. **Different Rust toolchain versions** (stable vs nightly)
2. **Different clippy versions** (newer versions have more lints)
3. **Different OS/environments** (Linux vs Windows)
4. **Different dependency versions** in CI cache
5. **Line ending differences** (CRLF vs LF)

## Best Practices

### Always Run Full CI Suite Locally
```bash
# This is what CI runs - run it locally first!
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings  
cargo test --workspace
cargo deny check licenses  # ← Don't forget license compliance!
```

### Use Consistent Toolchain
- Use **stable Rust** (same as CI)
- Avoid nightly-only features in rustfmt config
- Keep dependencies up to date

### Commit Strategy
- **Small, focused commits** with clear messages
- **Test each commit** before pushing
- **Use conventional commits**: `type(scope): description`

### When CI Fails
1. **Pull latest changes** from remote
2. **Run the full checklist locally**
3. **Fix issues** and test again
4. **Amend commits** if needed (use `--force-with-lease`)

## License Requirements

### Adding New Crates/Packages

**⚠️ CRITICAL: Every new crate MUST have a license specified in its `Cargo.toml`**

When creating a new crate (like `tools/whistle-playground`), you **must** add:

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
3. ✅ **Run license check**: `cargo deny check licenses`
4. ✅ **Fix any license violations** before pushing

### Common License Issues

**Error: `whistle-playground = 0.1.0 is unlicensed`**
```bash
# Solution: Add license to Cargo.toml
[package]
license = "Apache-2.0 OR MIT"
```

**Error: `license is not explicitly allowed`**
```bash
# Solution: Add the license to deny.toml allow list
allow = [
    "Apache-2.0",
    "MIT", 
    "MPL-2.0",  # ← Add missing license
    # ... other licenses
]
```

## Common Issues & Solutions

### Formatting Issues
```bash
# Problem: CI expects different formatting
# Solution: Always run cargo fmt --all before committing
cargo fmt --all
git add .
git commit --amend --no-edit
git push --force-with-lease
```

### Clippy Warnings
```bash
# Problem: New clippy warnings in CI
# Solution: Run clippy locally with same flags
cargo clippy --workspace -- -D warnings
# Fix warnings, then commit and push
```

### Test Failures
```bash
# Problem: Tests pass locally but fail in CI
# Solution: Run tests in clean environment
cargo clean
cargo test --workspace
```

## Git Workflow

### Before Every Push
```bash
# 1. Run the full checklist
cargo fmt --all
cargo clippy --workspace -- -D warnings
cargo test --workspace
cargo deny check licenses  # ← License compliance check
cargo fmt --all -- --check

# 2. Stage and commit
git add .
git commit -m "type(scope): description"

# 3. Push
git push
```

### When Amending Commits
```bash
# After making changes to last commit
git add .
git commit --amend --no-edit
git push --force-with-lease  # Safe force push
```

## Quality Gates

**CI will fail if any of these fail:**
- ❌ `cargo fmt --all -- --check` (formatting diffs)
- ❌ `cargo clippy --workspace -- -D warnings` (linting errors)
- ❌ `cargo test --workspace` (test failures)
- ❌ `cargo deny check licenses` (license violations)

**Your local pass == green CI** ✅

---

**Remember: Always run the full checklist before pushing!**
