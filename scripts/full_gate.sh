#!/bin/bash
# Full delivery gate. Run once on the final candidate, not after every change.
# Pass --bundle to also rebuild and install /Applications/ZMark.app.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

echo "==> cargo test --workspace"
cargo test --workspace

echo "==> observation + Phase H (serial)"
cargo test -p zmark_app --features test-observation \
    --test phase_i_syntax_and_warm_palette_test \
    --test rework4_stage_a_workspace_test \
    --test phase_h_b1_real_geometry \
    --test phase_h_b2_highlight_matches_copy \
    --test phase_h_b3_autoscroll_real_events \
    --test phase_h_b4_cmd_c_routing -- --test-threads=1

echo "==> rename gate"
./scripts/check_rename.sh

if [ "${1:-}" = "--bundle" ]; then
    echo "==> bundle"
    ./scripts/bundle_macos.sh
fi
