#!/bin/bash
# Fails when the ZMark rename leaves the legacy brand outside the explicit
# allowlist. Historical ADRs/plans are excluded by design; test fixtures are
# allowed because their text is not product identity (crate imports are still
# enforced by the compiler).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ALLOWLIST="$ROOT/scripts/rename_allowlist.txt"

# path-prefix<TAB>content-pattern. Empty content pattern matches any line.
is_allowed() {
    local rel="$1" line="$2"
    local prefix pattern
    while IFS=$'\t' read -r prefix pattern; do
        [ -z "$prefix" ] && continue
        case "$prefix" in \#*) continue ;; esac
        case "$rel" in
            "$prefix"*) ;;
            *) continue ;;
        esac
        if [ -z "${pattern:-}" ] || printf '%s' "$line" | grep -Eq "$pattern"; then
            return 0
        fi
    done < "$ALLOWLIST"
    return 1
}

check_tree() {
    local root="$1"
    local hits=0
    while IFS= read -r hit; do
        local rel="${hit#"$root"/}"
        rel="${rel#./}"
        local line="${hit#*:*:}"
        if is_allowed "$rel" "$line"; then
            continue
        fi
        echo "rename leftover: $hit"
        hits=$((hits + 1))
    done < <(cd "$root" && grep -rnE 'glidemark|GlideMark' . \
        --exclude-dir=target \
        --exclude-dir=.git \
        --exclude-dir=adr \
        --exclude-dir=plans \
        --exclude=check_rename.sh \
        --exclude=rename_allowlist.txt 2>/dev/null || true)
    return "$hits"
}

self_test() {
    local tmp
    tmp="$(mktemp -d)"
    trap 'rm -rf "${tmp:-}"' EXIT

    mkdir -p "$tmp/clean/src" "$tmp/fixture/crates/zmark_app/tests" "$tmp/bad/src"
    printf 'hello\n' > "$tmp/clean/src/ok.rs"
    printf 'fixture GlideMark text\n' > "$tmp/fixture/crates/zmark_app/tests/sample.rs"
    printf 'let app = "GlideMark";\n' > "$tmp/bad/src/leak.rs"

    local clean_rc fixture_rc bad_rc
    check_tree "$tmp/clean" >/dev/null && clean_rc=0 || clean_rc=$?
    check_tree "$tmp/fixture" >/dev/null && fixture_rc=0 || fixture_rc=$?
    check_tree "$tmp/bad" >/dev/null 2>&1 && bad_rc=0 || bad_rc=$?

    [ "$clean_rc" -eq 0 ] || { echo "self-test: clean tree failed ($clean_rc)"; return 1; }
    [ "$fixture_rc" -eq 0 ] || { echo "self-test: fixture allowlist failed ($fixture_rc)"; return 1; }
    [ "$bad_rc" -ne 0 ] || { echo "self-test: unallowlisted leftover was accepted"; return 1; }
    echo "check_rename self-test: ok"
}

if [ "${1:-}" = "--self-test" ]; then
    self_test
    exit $?
fi

if ! check_tree "$ROOT"; then
    echo "error: legacy brand remains outside the allowlist" >&2
    exit 1
fi
echo "check_rename: ok"
