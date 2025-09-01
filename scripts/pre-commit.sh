#!/usr/bin/env bash
set -euo pipefail

# Fast guardrails before every commit.
echo "fmt..."
cargo fmt --package whistle --package order-router --package symbol-coordinator

echo "clippy..."
cargo clippy --workspace --all-targets --exclude whistle-playground --exclude whistle-monitor -- -D warnings

echo "test..."
cargo test --workspace
