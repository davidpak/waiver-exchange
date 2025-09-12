#!/usr/bin/env bash
set -euo pipefail

# Fast guardrails before every commit.
echo "fmt..."
cargo fmt --package whistle --package order-router --package symbol-coordinator --package execution-manager --package analytics-engine

echo "clippy..."
cargo clippy --package whistle --package order-router --package symbol-coordinator --package execution-manager --package analytics-engine --package persistence --package player-scraper --package player-registry --package simulation-clock --package order-gateway --package waiver-exchange-service --all-targets -- -D warnings

echo "test..."
cargo test --package whistle --package order-router --package symbol-coordinator --package execution-manager --package analytics-engine --package persistence --package player-scraper --package player-registry --package simulation-clock --package order-gateway --package waiver-exchange-service
