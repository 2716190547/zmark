#!/bin/bash
set -e

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_NAME="GlideMark"
BUNDLE_DIR="$DIR/target/bundle/$APP_NAME.app"
APP_DEST="/Applications/$APP_NAME.app"

echo "==> Building $APP_NAME release binary..."
cargo build --release -p glidemark_app

echo "==> Creating macOS App Bundle layout at $BUNDLE_DIR..."
rm -rf "$BUNDLE_DIR"
mkdir -p "$BUNDLE_DIR/Contents/MacOS"
mkdir -p "$BUNDLE_DIR/Contents/Resources"

echo "==> Copying binary..."
cp "$DIR/target/release/glidemark_app" "$BUNDLE_DIR/Contents/MacOS/$APP_NAME"
chmod +x "$BUNDLE_DIR/Contents/MacOS/$APP_NAME"

echo "==> Copying app icon and modern asset catalog..."
if [ -f "$DIR/assets/AppIcon.icns" ]; then
    cp "$DIR/assets/AppIcon.icns" "$BUNDLE_DIR/Contents/Resources/AppIcon.icns"
fi
if [ -f "$DIR/assets/Assets.car" ]; then
    cp "$DIR/assets/Assets.car" "$BUNDLE_DIR/Contents/Resources/Assets.car"
fi

echo "==> Generating Info.plist..."
cat << 'EOF' > "$BUNDLE_DIR/Contents/Info.plist"
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>GlideMark</string>
    <key>CFBundleDisplayName</key>
    <string>GlideMark</string>
    <key>CFBundleIdentifier</key>
    <string>com.glidemark.app</string>
    <key>CFBundleVersion</key>
    <string>0.1.0</string>
    <key>CFBundleShortVersionString</key>
    <string>0.1.0</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleSignature</key>
    <string>????</string>
    <key>CFBundleExecutable</key>
    <string>GlideMark</string>
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

echo "==> Installing to $APP_DEST..."
rm -rf "$APP_DEST"
cp -R "$BUNDLE_DIR" "$APP_DEST"
touch "$APP_DEST"

echo "==> Applying local Ad-Hoc code signature (zero-fee self-signed)..."
codesign --force --deep --sign - "$APP_DEST"

echo "==> Registering with macOS LaunchServices..."
if [ -x "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister" ]; then
    /System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f "$APP_DEST" || true
fi

echo "==> GlideMark successfully installed to $APP_DEST!"
