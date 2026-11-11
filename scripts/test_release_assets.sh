#!/bin/bash
# Checks release preflight without building, signing, networking or publishing.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VERSION="$(awk '/^version = / {gsub(/\"/, "", $3); print $3; exit}' "$ROOT/crates/zmark_app/Cargo.toml")"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
mkdir -p "$TMP/linux" "$TMP/bin"
printf fixture > "$TMP/key"
printf fixture > "$TMP/windows.exe"
printf '#!/bin/bash\nexit 1\n' > "$TMP/bin/gh"
chmod +x "$TMP/bin/gh"
run_preflight() {
    PATH="$TMP/bin:$PATH" ZMARK_RELEASE_KEY="$TMP/key" \
        ZMARK_WINDOWS_SETUP="$TMP/windows.exe" ZMARK_LINUX_ASSETS_DIR="$TMP/linux" \
        bash "$ROOT/scripts/release.sh" "$VERSION" > "$TMP/output" 2>&1
}
if run_preflight; then echo 'missing artifacts unexpectedly accepted' >&2; exit 1; fi
grep -q 'complete Linux CI artifacts' "$TMP/output"
for suffix in deb tar.gz; do printf fixture > "$TMP/linux/ZMark-$VERSION-linux-x86_64.$suffix"; done
printf 'wrong  ZMark-%s-linux-x86_64.deb\n' "$VERSION" > "$TMP/linux/SHA256SUMS-linux"
if run_preflight; then echo 'bad checksums unexpectedly accepted' >&2; exit 1; fi
grep -q 'Linux artifact checksum mismatch' "$TMP/output"
(cd "$TMP/linux" && shasum -a 256 "ZMark-$VERSION-linux-x86_64.deb" "ZMark-$VERSION-linux-x86_64.tar.gz") > "$TMP/linux/SHA256SUMS-linux"
if run_preflight; then echo 'unauthenticated release unexpectedly accepted' >&2; exit 1; fi
grep -q 'gh is not authenticated' "$TMP/output"
echo 'release asset preflight: missing / bad checksum / complete artifacts OK'
