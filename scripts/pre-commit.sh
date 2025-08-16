#!/usr/bin/env bash
set -euo pipefail

# Fast guardrails before every commit.
echo "fmt..."
cargo fmt --all

echo "clippy..."
cargo clippy --workspace --all-targets -- -D warnings

echo "test..."
cargo test --workspace
