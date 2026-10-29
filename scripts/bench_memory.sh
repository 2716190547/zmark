#!/usr/bin/env bash
# Samples idle RSS for the release app on the default sample and a ~1.5MB document.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN="${GLIDEMARK_BIN:-$ROOT/target/release/glidemark_app}"
SETTLE_SECONDS="${SETTLE_SECONDS:-6}"
SAMPLE_COUNT="${SAMPLE_COUNT:-8}"
SAMPLE_INTERVAL="${SAMPLE_INTERVAL:-0.5}"

if [[ ! -x "$BIN" ]]; then
  echo "Building release binary..."
  cargo build --release -p glidemark_app --manifest-path "$ROOT/Cargo.toml"
fi

TMP_DIR="$(mktemp -d)"
APP_PID=""
cleanup() {
  if [[ -n "$APP_PID" ]] && kill -0 "$APP_PID" 2>/dev/null; then
    kill "$APP_PID" 2>/dev/null || true
    wait "$APP_PID" 2>/dev/null || true
  fi
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT

LARGE_DOC="$TMP_DIR/large.md"
python3 - "$LARGE_DOC" <<'PY'
import sys

path = sys.argv[1]
target = 1_500_000
written = 0
i = 0
with open(path, "w", encoding="utf-8") as f:
    while written < target:
        block = f"## Section {i}\n\nShort **paragraph** with `code` and a [link](https://example.com/{i}).\n\n"
        f.write(block)
        written += len(block.encode("utf-8"))
        i += 1
PY

measure() {
  local label="$1"
  local file="${2:-}"

  if [[ -n "$file" ]]; then
    "$BIN" "$file" >/dev/null 2>&1 &
  else
    "$BIN" >/dev/null 2>&1 &
  fi
  APP_PID=$!
  sleep "$SETTLE_SECONDS"

  if ! kill -0 "$APP_PID" 2>/dev/null; then
    printf '%-18s failed to stay running\n' "$label"
    APP_PID=""
    return
  fi

  local max_kb=0
  local rss_kb
  for _ in $(seq 1 "$SAMPLE_COUNT"); do
    rss_kb="$(ps -o rss= -p "$APP_PID" | tr -d ' ' || true)"
    if [[ -n "$rss_kb" ]] && (( rss_kb > max_kb )); then
      max_kb=$rss_kb
    fi
    sleep "$SAMPLE_INTERVAL"
  done

  kill "$APP_PID" 2>/dev/null || true
  wait "$APP_PID" 2>/dev/null || true
  APP_PID=""

  awk -v label="$label" -v kb="$max_kb" 'BEGIN { printf "%-18s %7.1f MB\n", label, kb / 1024 }'
}

echo "GlideMark idle RSS (release, peak of $SAMPLE_COUNT samples)"
echo "-----------------------------------------------------------"
measure "default" 
measure "1.5MB document" "$LARGE_DOC"
echo "-----------------------------------------------------------"
echo "Large document: $(wc -c < "$LARGE_DOC" | tr -d ' ') bytes, $(grep -c '^## ' "$LARGE_DOC") headings"
