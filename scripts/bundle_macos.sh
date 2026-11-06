#!/bin/bash
set -euo pipefail

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_NAME="ZMark"
BUNDLE_DIR="$DIR/target/bundle/$APP_NAME.app"
APP_DEST="/Applications/$APP_NAME.app"
LSREGISTER="/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister"

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$DIR/crates/zmark_app/Cargo.toml" | head -n1)"
if [ -z "$VERSION" ]; then
    echo "error: cannot read version from crates/zmark_app/Cargo.toml" >&2
    exit 1
fi

echo "==> Building $APP_NAME $VERSION release binary..."
cargo build --release -p zmark_app

echo "==> Creating macOS App Bundle layout at $BUNDLE_DIR..."
rm -rf "$BUNDLE_DIR"
mkdir -p "$BUNDLE_DIR/Contents/MacOS"
mkdir -p "$BUNDLE_DIR/Contents/Resources"

echo "==> Copying binary..."
cp "$DIR/target/release/zmark_app" "$BUNDLE_DIR/Contents/MacOS/$APP_NAME"
chmod +x "$BUNDLE_DIR/Contents/MacOS/$APP_NAME"

echo "==> Copying app icon and modern asset catalog..."
if [ -f "$DIR/assets/AppIcon.icns" ]; then
    cp "$DIR/assets/AppIcon.icns" "$BUNDLE_DIR/Contents/Resources/AppIcon.icns"
fi
if [ -f "$DIR/assets/Assets.car" ]; then
    cp "$DIR/assets/Assets.car" "$BUNDLE_DIR/Contents/Resources/Assets.car"
fi

echo "==> Generating Info.plist..."
cat << EOF > "$BUNDLE_DIR/Contents/Info.plist"
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>$APP_NAME</string>
    <key>CFBundleDisplayName</key>
    <string>$APP_NAME</string>
    <key>CFBundleIdentifier</key>
    <string>com.zmark.app</string>
    <key>CFBundleVersion</key>
    <string>$VERSION</string>
    <key>CFBundleShortVersionString</key>
    <string>$VERSION</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleSignature</key>
    <string>????</string>
    <key>CFBundleExecutable</key>
    <string>$APP_NAME</string>
    <key>CFBundleIconFile</key>
    <string>AppIcon</string>
    <key>CFBundleIconName</key>
    <string>AppIcon</string>
    <key>LSMinimumSystemVersion</key>
    <string>12.0</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>NSSupportsAutomaticGraphicsSwitching</key>
    <true/>
    <key>CFBundleDocumentTypes</key>
    <array>
        <dict>
            <key>CFBundleTypeName</key>
            <string>Markdown Document</string>
            <key>CFBundleTypeRole</key>
            <string>Editor</string>
            <key>LSHandlerRank</key>
            <string>Alternate</string>
            <key>LSItemContentTypes</key>
            <array>
                <string>net.daringfireball.markdown</string>
                <string>public.plain-text</string>
            </array>
        </dict>
    </array>
</dict>
</plist>
EOF

echo "==> Applying Ad-Hoc signature to the staging bundle..."
codesign --force --deep --sign - "$BUNDLE_DIR"
codesign --verify --deep --strict "$BUNDLE_DIR"

# Never replace a running bundle. pgrep exit 1 means "not running"; any other
# failure is treated as an error instead of guessing.
set +e
/usr/bin/pgrep -x "$APP_NAME" >/dev/null 2>&1
PGREP_RC=$?
set -e
if [ "$PGREP_RC" -eq 0 ]; then
    echo "error: $APP_NAME is running; quit it before installing" >&2
    exit 1
elif [ "$PGREP_RC" -ne 1 ]; then
    echo "error: /usr/bin/pgrep failed with status $PGREP_RC" >&2
    exit 1
fi

echo "==> Installing to $APP_DEST..."
BACKUP="$APP_DEST.previous.$(date +%Y%m%d%H%M%S)"
HAD_OLD=0
if [ -d "$APP_DEST" ]; then
    mv "$APP_DEST" "$BACKUP"
    HAD_OLD=1
fi
if ! ditto "$BUNDLE_DIR" "$APP_DEST"; then
    rm -rf "$APP_DEST"
    if [ "$HAD_OLD" -eq 1 ]; then mv "$BACKUP" "$APP_DEST"; fi
    echo "error: install failed; previous bundle restored" >&2
    exit 1
fi
if ! codesign --verify --deep --strict "$APP_DEST"; then
    rm -rf "$APP_DEST"
    if [ "$HAD_OLD" -eq 1 ]; then mv "$BACKUP" "$APP_DEST"; fi
    echo "error: installed bundle failed signature verification" >&2
    exit 1
fi
touch "$APP_DEST"

echo "==> Registering with macOS LaunchServices..."
if [ -x "$LSREGISTER" ]; then
    "$LSREGISTER" -f "$APP_DEST" || true
fi

# Old GlideMark bundle: move it out (rollback copy) only after the new bundle
# is installed and verified. Never touch it while it is running.
OLD_APP="/Applications/GlideMark.app"
if [ -d "$OLD_APP" ]; then
    set +e
    /usr/bin/pgrep -x GlideMark >/dev/null 2>&1
    OLD_RC=$?
    set -e
    if [ "$OLD_RC" -eq 0 ]; then
        echo "warning: GlideMark is still running; leaving $OLD_APP untouched" >&2
    else
        OLD_BACKUP="$DIR/target/bundle/GlideMark.previous.$(date +%Y%m%d%H%M%S).app"
        mv "$OLD_APP" "$OLD_BACKUP"
        if [ -x "$LSREGISTER" ]; then "$LSREGISTER" -u "$OLD_APP" || true; fi
        echo "==> Moved legacy bundle to $OLD_BACKUP"
    fi
fi

echo "==> $APP_NAME $VERSION successfully installed to $APP_DEST!"
