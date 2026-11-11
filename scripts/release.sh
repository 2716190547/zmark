#!/bin/bash
# Builds, verifies, signs and publishes a ZMark release to the public
# zmark-releases repository. The private key stays outside the repository.
set -euo pipefail

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="${1:-}"
RELEASES_REPO="${ZMARK_RELEASES_REPO:-2716190547/zmark-releases}"
RELEASE_KEY="${ZMARK_RELEASE_KEY:-$HOME/.config/zmark/release.key}"
WINDOWS_SETUP="${ZMARK_WINDOWS_SETUP:-}"

if [ -z "$VERSION" ]; then
    echo "usage: scripts/release.sh <version>" >&2
    exit 1
fi

CARGO_VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$DIR/crates/zmark_app/Cargo.toml" | head -n1)"
if [ "$VERSION" != "$CARGO_VERSION" ]; then
    echo "error: $VERSION does not match crates/zmark_app/Cargo.toml ($CARGO_VERSION)" >&2
    exit 1
fi
if [ ! -f "$RELEASE_KEY" ]; then
    echo "error: release key not found at $RELEASE_KEY" >&2
    exit 1
fi
if [ ! -f "$WINDOWS_SETUP" ]; then
    echo "error: set ZMARK_WINDOWS_SETUP to the Windows installer for version $VERSION" >&2
    exit 1
fi
if ! gh auth status >/dev/null 2>&1; then
    echo "error: gh is not authenticated" >&2
    exit 1
fi
if ! gh repo view "$RELEASES_REPO" >/dev/null 2>&1; then
    echo "==> Creating public release repository $RELEASES_REPO"
    gh repo create "$RELEASES_REPO" --public --add-readme \
        --description "ZMark signed release assets"
fi
VISIBILITY="$(gh repo view "$RELEASES_REPO" --json visibility -q .visibility)"
if [ "$VISIBILITY" != "PUBLIC" ]; then
    echo "error: $RELEASES_REPO must be public (got $VISIBILITY)" >&2
    exit 1
fi

echo "==> Running delivery gates..."
cargo test --workspace
cargo test -p zmark_app --features test-observation \
    --test phase_i_syntax_and_warm_palette_test \
    --test rework4_stage_a_workspace_test \
    --test phase_h_b1_real_geometry \
    --test phase_h_b2_highlight_matches_copy \
    --test phase_h_b3_autoscroll_real_events \
    --test phase_h_b4_cmd_c_routing -- --test-threads=1
./scripts/check_rename.sh

ZIP="$DIR/target/release/ZMark-$VERSION.zip"
rm -f "$ZIP"
if [ -n "${ZMARK_MACOS_ZIP:-}" ]; then
    echo "==> Using prebuilt macOS bundle: $ZMARK_MACOS_ZIP"
    cp "$ZMARK_MACOS_ZIP" "$ZIP"
else
    echo "==> Building ZMark.app..."
    ./scripts/bundle_macos.sh
    echo "==> Packaging the signed staging bundle..."
    ditto -c -k --sequesterRsrc --keepParent "$DIR/target/bundle/ZMark.app" "$ZIP"
fi

VERIFY_DIR="$(mktemp -d)"
trap 'rm -rf "$VERIFY_DIR"' EXIT
ditto -x -k "$ZIP" "$VERIFY_DIR"
codesign --verify --deep --strict "$VERIFY_DIR/ZMark.app"
PACKAGED_VERSION="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$VERIFY_DIR/ZMark.app/Contents/Info.plist")"
if [ "$PACKAGED_VERSION" != "$VERSION" ]; then
    echo "error: packaged version $PACKAGED_VERSION != $VERSION" >&2
    exit 1
fi

SHA256="$(shasum -a 256 "$ZIP" | awk '{print $1}')"
URL="https://github.com/$RELEASES_REPO/releases/download/v$VERSION/ZMark-$VERSION.zip"
PUBLISHED_AT="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
PAYLOAD="$VERIFY_DIR/payload.txt"
printf '%s\n%s\n%s\n%s' "$VERSION" "$URL" "$SHA256" "$PUBLISHED_AT" > "$PAYLOAD"
SIGNATURE="$(cargo run -q -p release_tool -- sign --key "$RELEASE_KEY" --payload-file "$PAYLOAD" | tr -d '\n')"
PUBLIC_KEY="$(cargo run -q -p release_tool -- pubkey --key "$RELEASE_KEY" | tr -d '\n')"
if [ "$PUBLIC_KEY" != "d2b3380214d08522d78227e369da57c19f6746ec96135cae65ae85e42053dda5" ]; then
    echo "error: release signing key does not match the app's public key" >&2
    exit 1
fi
cargo run -q -p release_tool -- verify --pubkey "$PUBLIC_KEY" --payload-file "$PAYLOAD" --sig "$SIGNATURE"

MANIFEST="$VERIFY_DIR/latest.json"
cargo run -q -p release_tool -- manifest \
    --version "$VERSION" \
    --url "$URL" \
    --sha256 "$SHA256" \
    --published-at "$PUBLISHED_AT" \
    --signature "$SIGNATURE" > "$MANIFEST"

WINDOWS_ASSET="$VERIFY_DIR/ZMark-$VERSION-setup.exe"
cp "$WINDOWS_SETUP" "$WINDOWS_ASSET"
WINDOWS_SHA256="$(shasum -a 256 "$WINDOWS_ASSET" | awk '{print $1}')"
WINDOWS_URL="https://github.com/$RELEASES_REPO/releases/download/v$VERSION/ZMark-$VERSION-setup.exe"
printf '%s\n%s\n%s\n%s' "$VERSION" "$WINDOWS_URL" "$WINDOWS_SHA256" "$PUBLISHED_AT" > "$PAYLOAD"
WINDOWS_SIGNATURE="$(cargo run -q -p release_tool -- sign --key "$RELEASE_KEY" --payload-file "$PAYLOAD" | tr -d '\n')"
cargo run -q -p release_tool -- verify --pubkey "$PUBLIC_KEY" --payload-file "$PAYLOAD" --sig "$WINDOWS_SIGNATURE"
WINDOWS_MANIFEST="$VERIFY_DIR/latest-windows.json"
cargo run -q -p release_tool -- manifest --version "$VERSION" --url "$WINDOWS_URL" \
    --sha256 "$WINDOWS_SHA256" --published-at "$PUBLISHED_AT" --signature "$WINDOWS_SIGNATURE" > "$WINDOWS_MANIFEST"

echo "==> Publishing draft v$VERSION to $RELEASES_REPO..."
gh release create "v$VERSION" \
    --repo "$RELEASES_REPO" \
    --draft \
    --title "ZMark v$VERSION" \
    --notes "ZMark v$VERSION" \
    "$ZIP" "$MANIFEST" "$WINDOWS_ASSET" "$WINDOWS_MANIFEST"

DRAFT_DIR="$VERIFY_DIR/draft"
mkdir -p "$DRAFT_DIR"
gh release download "v$VERSION" \
    --repo "$RELEASES_REPO" \
    --pattern 'ZMark-*.zip' \
    --pattern 'latest.json' \
    --pattern 'latest-windows.json' \
    --pattern 'ZMark-*-setup.exe' \
    --dir "$DRAFT_DIR"
cmp "$MANIFEST" "$DRAFT_DIR/latest.json"
cmp "$WINDOWS_MANIFEST" "$DRAFT_DIR/latest-windows.json"
cmp "$WINDOWS_ASSET" "$DRAFT_DIR/ZMark-$VERSION-setup.exe"
DOWNLOADED_SHA="$(shasum -a 256 "$DRAFT_DIR/ZMark-$VERSION.zip" | awk '{print $1}')"
if [ "$DOWNLOADED_SHA" != "$SHA256" ]; then
    echo "error: uploaded zip hash mismatch" >&2
    exit 1
fi

echo "==> Publishing v$VERSION..."
gh release edit "v$VERSION" --repo "$RELEASES_REPO" --draft=false

echo "==> Final public checks..."
curl -fsSL "https://github.com/$RELEASES_REPO/releases/latest/download/latest.json" -o "$VERIFY_DIR/public.json"
cmp "$MANIFEST" "$VERIFY_DIR/public.json"
curl -fsSL "https://github.com/$RELEASES_REPO/releases/latest/download/latest-windows.json" -o "$VERIFY_DIR/public-windows.json"
cmp "$WINDOWS_MANIFEST" "$VERIFY_DIR/public-windows.json"
curl -fsSL -o "$VERIFY_DIR/public-setup.exe" "$WINDOWS_URL"
cmp "$WINDOWS_ASSET" "$VERIFY_DIR/public-setup.exe"
curl -fsSL -o "$VERIFY_DIR/public.zip" "$URL"
PUBLIC_SHA="$(shasum -a 256 "$VERIFY_DIR/public.zip" | awk '{print $1}')"
if [ "$PUBLIC_SHA" != "$SHA256" ]; then
    echo "error: public zip hash mismatch" >&2
    exit 1
fi

echo "==> ZMark v$VERSION published"
echo "    feed: https://github.com/$RELEASES_REPO/releases/latest/download/latest.json"
echo "    zip:  $URL"
echo "    sha256: $SHA256"
