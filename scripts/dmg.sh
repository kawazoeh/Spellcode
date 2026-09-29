#!/usr/bin/env bash
# Build a distributable .dmg around Spellcode.app.
#
#   scripts/dmg.sh [chemin/Spellcode.app] [sortie.dmg]
#
# hdiutil ships with macOS, so the image is produced without any third-party
# action or helper to pin. The volume holds the app plus a symlink to
# /Applications, which is what makes the image installable by drag-and-drop.
set -euo pipefail

app="${1:-target/Spellcode.app}"
out="${2:-Spellcode.dmg}"
volname="${VOLNAME:-Spellcode}"

[[ -d "$app" ]] || { echo "app not found: $app" >&2; exit 1; }

# Only ship a bundle whose resources are sealed. An unsigned bundle (the bare
# linker ad-hoc signature) makes the Finder call the app "damaged", so refuse
# to build an image around it rather than publish a broken .dmg.
codesign --verify --deep --strict --verbose=2 "$app"

staging="$(mktemp -d)"
trap 'rm -rf "$staging"' EXIT

# ditto keeps the bundle metadata; a plain cp can drop extended attributes.
ditto "$app" "$staging/$(basename "$app")"
ln -s /Applications "$staging/Applications"

rm -f "$out"
hdiutil create \
  -volname "$volname" \
  -srcfolder "$staging" \
  -ov -format UDZO \
  "$out" >/dev/null

echo "$out"
