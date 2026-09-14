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
rm -rf "$APP"
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
  <key>CFBundleDocumentTypes</key>
  <array>
    <dict>
      <key>CFBundleTypeName</key><string>Smart Game Format</string>
      <key>CFBundleTypeRole</key><string>Viewer</string>
      <key>CFBundleTypeIconFile</key><string>AppIcon.icns</string>
      <key>LSHandlerRank</key><string>Alternate</string>
      <key>CFBundleTypeExtensions</key><array><string>sgf</string></array>
      <key>LSItemContentTypes</key><array><string>local.go-teacher.sgf</string></array>
    </dict>
  </array>
  <key>UTImportedTypeDeclarations</key>
  <array>
    <dict>
      <key>UTTypeIdentifier</key><string>local.go-teacher.sgf</string>
      <key>UTTypeDescription</key><string>Smart Game Format</string>
      <key>UTTypeConformsTo</key><array><string>public.text</string></array>
      <key>UTTypeIconFile</key><string>AppIcon.icns</string>
      <key>UTTypeTagSpecification</key>
      <dict>
        <key>public.filename-extension</key><array><string>sgf</string></array>
        <key>public.mime-type</key><string>application/x-go-sgf</string>
      </dict>
    </dict>
  </array>
</dict>
</plist>
PLIST
echo -n "APPL????" > "$APP/Contents/PkgInfo"

if [ -n "${CODESIGN_IDENTITY:-}" ]; then
  echo "==> signing with $CODESIGN_IDENTITY (hardened runtime)"
  codesign --force --deep --options runtime --timestamp --sign "$CODESIGN_IDENTITY" "$APP"
else
  echo "==> signing (ad hoc; set CODESIGN_IDENTITY=\"Developer ID Application: ...\" for a distributable build)"
  codesign --force --deep --sign - "$APP"
fi
codesign --verify --verbose=1 "$APP"

echo "==> building DMG"
STAGE=$(mktemp -d)
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
DMG="$DIST/GoTeacher-$VERSION.dmg"
hdiutil create -volname "$APP_NAME" -srcfolder "$STAGE" -ov -format UDZO -quiet "$DMG"
rm -rf "$STAGE"
if [ -n "${CODESIGN_IDENTITY:-}" ]; then
  codesign --force --timestamp --sign "$CODESIGN_IDENTITY" "$DMG"
fi
if [ -n "${NOTARY_PROFILE:-}" ]; then
  # One-time setup: xcrun notarytool store-credentials "$NOTARY_PROFILE" --apple-id ... --team-id ... --password <app-specific>
  echo "==> notarizing with keychain profile $NOTARY_PROFILE"
  xcrun notarytool submit "$DMG" --keychain-profile "$NOTARY_PROFILE" --wait
  xcrun stapler staple "$DMG"
  xcrun stapler staple "$APP"
fi

echo
echo "Done:"
echo "  $APP"
echo "  $DMG"
echo "Install: open the DMG and drag '$APP_NAME' to Applications, or:  cp -R \"$APP\" /Applications/"
