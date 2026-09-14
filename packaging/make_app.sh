#!/bin/zsh
# Build Go Teacher.app and a DMG installer from the Rust binary.
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"

APP_NAME="Go Teacher"
BUNDLE_ID="local.go-teacher.app"
VERSION=$(grep -m1 '^version' Cargo.toml | sed 's/.*"\(.*\)".*/\1/')
DIST=dist
APP="$DIST/$APP_NAME.app"

echo "==> building release binary"
cargo build --release

echo "==> assembling $APP"
rm -rf "$DIST"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp target/release/go_teacher "$APP/Contents/MacOS/go_teacher"

echo "==> drawing icon"
ICONSET=$(mktemp -d)/AppIcon.iconset
mkdir -p "$ICONSET"
python3 packaging/make_icon.py "$ICONSET"
for s in 16 32 128 256 512; do
  mv "$ICONSET/icon_${s}.png" "$ICONSET/icon_${s}x${s}.png"
  d=$((s*2))
  cp "$ICONSET/icon_${d}.png" "$ICONSET/icon_${s}x${s}@2x.png"
done
rm -f "$ICONSET"/icon_64.png "$ICONSET"/icon_1024.png
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/AppIcon.icns"

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>$APP_NAME</string>
  <key>CFBundleDisplayName</key><string>$APP_NAME</string>
  <key>CFBundleIdentifier</key><string>$BUNDLE_ID</string>
  <key>CFBundleVersion</key><string>$VERSION</string>
  <key>CFBundleShortVersionString</key><string>$VERSION</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleExecutable</key><string>go_teacher</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
  <key>LSMinimumSystemVersion</key><string>12.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>LSApplicationCategoryType</key><string>public.app-category.board-games</string>
  <key>NSHumanReadableCopyright</key><string>Go Teacher — local KataGo game review</string>
</dict>
</plist>
PLIST
echo -n "APPL????" > "$APP/Contents/PkgInfo"

echo "==> signing (ad hoc)"
codesign --force --deep --sign - "$APP"
codesign --verify --verbose=1 "$APP"

echo "==> building DMG"
STAGE=$(mktemp -d)
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
DMG="$DIST/GoTeacher-$VERSION.dmg"
hdiutil create -volname "$APP_NAME" -srcfolder "$STAGE" -ov -format UDZO -quiet "$DMG"
rm -rf "$STAGE"

echo
echo "Done:"
echo "  $APP"
echo "  $DMG"
echo "Install: open the DMG and drag '$APP_NAME' to Applications, or:  cp -R \"$APP\" /Applications/"
