#!/bin/bash
# Fast parallel & grouped development check.
# Usage: ./scripts/dev_check.sh [all|fast|core|state|editor|preview|media|lightbox|obs]
# Uses unified workspace feature resolution so switching groups never invalidates
# compiled dependency caches, and runs independent test binaries concurrently
# (1 OS process per binary) while keeping scripts/full_gate.sh for release gates.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

GROUP="${1:-all}"

./scripts/check_rename.sh

run_parallel_cargo_tests() {
    local label="$1"
    local filter_targets="$2"
    local extra_bin_args="$3"
    shift 3
    python3 - "$label" "$filter_targets" "$extra_bin_args" "$@" <<'PY'
import json
import os
import re
import subprocess
import sys
import time
from concurrent.futures import ThreadPoolExecutor, as_completed

label = sys.argv[1]
allowed = {t for t in sys.argv[2].split(",") if t} if sys.argv[2] else None
extra_bin_args = [arg for arg in sys.argv[3].split() if arg]
cargo_args = sys.argv[4:]

t_build_start = time.monotonic()
proc = subprocess.run(
    ["cargo", "test", "--no-run", "--message-format=json", *cargo_args],
    stdout=subprocess.PIPE,
    text=True,
)
if proc.returncode != 0:
    sys.exit(proc.returncode)

executables = []
seen = set()
for line in proc.stdout.splitlines():
    if not line.startswith("{"):
        continue
    msg = json.loads(line)
    if msg.get("reason") == "compiler-artifact" and msg.get("profile", {}).get("test"):
        exe = msg.get("executable")
        name = msg.get("target", {}).get("name", os.path.basename(exe or ""))
        if allowed is not None and name not in allowed:
            continue
        if exe and exe not in seen:
            seen.add(exe)
            executables.append((name, exe))

t_build = time.monotonic() - t_build_start
if not executables:
    print(f"[{label}] no test binaries matched")
    sys.exit(0)

max_workers = min(max(os.cpu_count() or 4, 2), 6, len(executables))
t_run_start = time.monotonic()

def run_one(item):
    name, exe = item
    start = time.monotonic()
    res = subprocess.run([exe, "--quiet", *extra_bin_args], stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    return name, res.returncode, res.stdout, time.monotonic() - start

total_passed = 0
failures = []
with ThreadPoolExecutor(max_workers=max_workers) as pool:
    futures = [pool.submit(run_one, item) for item in executables]
    for fut in as_completed(futures):
        name, rc, out, elapsed = fut.result()
        m = re.search(r"(\d+) passed;\s*(\d+) failed", out)
        counts = f"{m.group(1)} passed" if m else ("ok" if rc == 0 else "failed")
        if m:
            total_passed += int(m.group(1))
        status = counts if rc == 0 else f"FAILED({rc})"
        print(f"  [{label}] {name}: {status} ({elapsed:.2f}s)")
        if rc != 0:
            failures.append((name, out))

t_run = time.monotonic() - t_run_start
print(f"==> [{label}] build {t_build:.2f}s, parallel run ({len(executables)} bins, {total_passed} tests) {t_run:.2f}s")

if failures:
    for name, out in failures:
        print(f"\n--- FAILURE OUTPUT: {name} ---\n{out}")
    sys.exit(1)
PY
}

case "$GROUP" in
    core)
        run_parallel_cargo_tests "core" "zmark_core,core_all,cache_accounting_test,release_tool" "" --workspace
        ;;
    state)
        run_parallel_cargo_tests "state" "app_state" "" -p zmark_app --test app_state
        ;;
    editor)
        run_parallel_cargo_tests "editor" "app_editor" "" -p zmark_app --test app_editor
        ;;
    preview)
        run_parallel_cargo_tests "preview" "app_preview" "" -p zmark_app --test app_preview
        ;;
    media)
        run_parallel_cargo_tests "media" "app_media" "" -p zmark_app --test app_media
        ;;
    lightbox)
        run_parallel_cargo_tests "lightbox" "" "" -p zmark_app --test hidpi_lightbox_test --test lightbox_color_regression_test --test lightbox_paper_canvas_test --test lightbox_test
        ;;
    obs)
        # Observation targets share process-global recorders; serialize within each binary while running binaries concurrently.
        run_parallel_cargo_tests "obs" "" "--test-threads=1" -p zmark_app --features test-observation --test phase_i_syntax_and_warm_palette_test --test rework4_stage_a_workspace_test --test phase_h_b1_real_geometry --test phase_h_b2_highlight_matches_copy --test phase_h_b3_autoscroll_real_events --test phase_h_b4_cmd_c_routing
        ;;
    fast)
        run_parallel_cargo_tests "fast" "release_tool,zmark_core,core_all,cache_accounting_test,zmark_app,app_state,app_editor,app_preview,app_media" "" --workspace
        ;;
    all)
        run_parallel_cargo_tests "workspace" "" "" --workspace
        run_parallel_cargo_tests "obs" "" "--test-threads=1" -p zmark_app --features test-observation --test phase_i_syntax_and_warm_palette_test --test rework4_stage_a_workspace_test --test phase_h_b1_real_geometry --test phase_h_b2_highlight_matches_copy --test phase_h_b3_autoscroll_real_events --test phase_h_b4_cmd_c_routing
        ;;
    *)
        echo "usage: $0 [all|fast|core|state|editor|preview|media|lightbox|obs]" >&2
        exit 2
        ;;
esac
