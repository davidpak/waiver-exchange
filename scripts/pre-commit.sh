#!/usr/bin/env bash
set -euo pipefail

# Fast guardrails before every commit.
echo "fmt..."
cargo fmt --package whistle --package order-router --package symbol-coordinator --package execution-manager --package analytics-engine

echo "clippy..."
cargo clippy --workspace --all-targets --exclude admin-cli -- -D warnings

echo "test..."
cargo test --workspace --exclude admin-cli
