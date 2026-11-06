#!/usr/bin/env python3
"""Analyze the ZMark frame probe output and apply the §2.4 thresholds."""

import json
import statistics
import sys

path = sys.argv[1] if len(sys.argv) > 1 else "/tmp/zmark_frames.json"
try:
    with open(path, "r", encoding="utf-8") as handle:
        data = json.load(handle)
except (OSError, ValueError) as error:
    print(f"FAIL: cannot read {path}: {error}")
    sys.exit(1)

samples = sorted(value for value in data.get("samples_ms", []) if value > 0)
if len(samples) < 30:
    print(f"FAIL: only {len(samples)} frames captured (need >= 30)")
    sys.exit(1)

p95 = samples[max(0, int(len(samples) * 0.95) - 1)]
worst = samples[-1]
median = statistics.median(samples)
refresh = 1000.0 / median if median > 0 else 0.0
high_refresh = refresh >= 90.0
p95_limit = 8.3 if high_refresh else 16.7
worst_limit = 16.7 if high_refresh else 33.0
ok = p95 <= p95_limit and worst <= worst_limit

print(
    f"{'PASS' if ok else 'FAIL'}: frames={len(samples)} "
    f"refresh~{refresh:.1f}Hz p95={p95:.2f}ms worst={worst:.2f}ms "
    f"(limits p95 {p95_limit}ms, worst {worst_limit}ms)"
)
sys.exit(0 if ok else 1)
