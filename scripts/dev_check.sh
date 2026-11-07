#!/bin/bash
# Fast development loop: run every test once in parallel, plus the rename gate.
# Use this after each change/stage. Run scripts/full_gate.sh on the final candidate.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

./scripts/check_rename.sh
cargo nextest run --workspace --no-fail-fast
