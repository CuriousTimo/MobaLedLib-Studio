#!/usr/bin/env bash
# The local quality gate: the same checks as .github/workflows/ci.yml, run before every commit.
# Exit status 0 only when everything passes.
set -euo pipefail

cd "$(dirname "$0")/.."

echo "== fmt"
cargo fmt --all --check
echo "== clippy"
cargo clippy --workspace --all-targets -- -D warnings
echo "== test (core, CLI and simulator)"
cargo test -p mll-core -p mll-cli -p mll-sim
echo "== build the app"
cargo build -p mobaledlib-studio

echo "== check: all passed"
