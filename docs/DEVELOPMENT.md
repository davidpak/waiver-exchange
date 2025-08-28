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

### 5. Optional: Supply Chain Security
```bash
# If cargo-deny is installed
cargo deny check

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

**Your local pass == green CI** ✅

---

**Remember: Always run the full checklist before pushing!**
