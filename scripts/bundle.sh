#!/usr/bin/env bash
# Assemble Spellcode.app around the release binary and the app icon.
#
#   scripts/bundle.sh [chemin/vers/spellcode01.png]
#
# A bare Mach-O binary has no identity on macOS: the Dock, the Finder and the
# window title bar all fall back to the generic icon. Wrapping it in a bundle
# with an Info.plist and an .icns is what gives it a name and a face.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source_icon="${1:-$here/assets/spellcode01.png}"
binary="$here/target/release/spellcode-app"
app="$here/target/Spellcode.app"

[[ -f "$binary" ]] || { echo "build first: cargo build --release" >&2; exit 1; }
[[ -f "$source_icon" ]] || { echo "icon not found: $source_icon" >&2; exit 1; }

# --- icon ---------------------------------------------------------------
iconset="$(mktemp -d)/Spellcode.iconset"
mkdir -p "$iconset"
for size in 16 32 64 128 256 512 1024; do
  sips -z "$size" "$size" "$source_icon" --out "$iconset/icon_${size}x${size}.png" >/dev/null
done
# The @2x variants are the same files, macOS picks by name.
cp "$iconset/icon_32x32.png"   "$iconset/icon_16x16@2x.png"
cp "$iconset/icon_64x64.png"   "$iconset/icon_32x32@2x.png"
cp "$iconset/icon_256x256.png" "$iconset/icon_128x128@2x.png"
cp "$iconset/icon_512x512.png" "$iconset/icon_256x256@2x.png"
cp "$iconset/icon_1024x1024.png" "$iconset/icon_512x512@2x.png"

mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
iconutil -c icns "$iconset" -o "$app/Contents/Resources/Spellcode.icns"
rm -rf "$(dirname "$iconset")"

# --- executable ---------------------------------------------------------
cp "$binary" "$app/Contents/MacOS/Spellcode"
chmod +x "$app/Contents/MacOS/Spellcode"

# --- identity -----------------------------------------------------------
cat > "$app/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>              <string>Spellcode</string>
    <key>CFBundleDisplayName</key>       <string>Spellcode</string>
    <key>CFBundleExecutable</key>        <string>Spellcode</string>
    <key>CFBundleIdentifier</key>        <string>dev.spellcode.Spellcode</string>
    <key>CFBundleIconFile</key>          <string>Spellcode</string>
    <key>CFBundlePackageType</key>       <string>APPL</string>
    <key>CFBundleShortVersionString</key><string>0.1.0</string>
    <key>CFBundleVersion</key>           <string>0.1.0</string>
    <key>LSMinimumSystemVersion</key>    <string>11.0</string>
    <key>LSApplicationCategoryType</key> <string>public.app-category.developer-tools</string>
    <key>NSHighResolutionCapable</key>   <true/>
</dict>
</plist>
PLIST

# Let Launch Services forget any earlier registration of the same path.
touch "$app"
echo "$app"
