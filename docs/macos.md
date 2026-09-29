# Spellcode on macOS — bundle, signature and Gatekeeper

Scope: how `scripts/bundle.sh` builds `Spellcode.app`, what is signed, and what
is deliberately left out. Applies to every release.

## Why the bundle is signed at all

`cargo build --release` leaves the executable with the **linker's ad-hoc
signature** only. That is not a bundle signature: the executable is marked as
signed while the bundle's resources (`Info.plist`, `Spellcode.icns`) are not
sealed into it. When such an app is launched from Finder, macOS reports it as
damaged:

```
Spellcode.app: code has no resources but signature indicates they must be present
```

Release **v0.1.0** shipped exactly that, so the download could not be opened.

## What is done

`scripts/bundle.sh` assembles the bundle first (icon, executable, `Info.plist`)
and then re-signs the **complete** bundle, in that order — signing before the
icon is in place would leave the resources unsealed again:

```
codesign --force --deep --sign - "$app"
codesign --verify --deep --strict --verbose=2 "$app"
```

The result:

- `flags=0x2(adhoc)`, `Info.plist entries=11`;
- `Sealed Resources version=2 rules=13 files=1`;
- `codesign --verify --deep --strict` exits 0.

`scripts/dmg.sh` re-checks the bundle before building the image, so a `.dmg` is
never produced around an unsealed app. The release workflow has the same check
as a dedicated step (*Verify the bundle signature*), which fails the release
instead of publishing a broken image.

## What is NOT done

The signature is **ad-hoc only**: no Developer ID, no notarisation. As a
result Gatekeeper still rejects the app on a normal double-click:

```
spctl -a -t exec Spellcode.app   # -> rejected
```

To launch it, the user opens it once with **right-click → Open** (or *System
Settings → Privacy & Security → Open Anyway*). The "damaged" dialog no longer
appears; the standard "unidentified developer" path applies instead.

Opening with a plain double-click, without any prompt, requires:

1. an **Apple Developer ID Application** certificate, which needs a **paid
   Apple Developer Program membership**;
2. `codesign --sign "Developer ID Application: …"` with the hardened runtime;
3. notarisation (`xcrun notarytool submit --wait`) and stapling
   (`xcrun stapler staple`).

None of this is set up: it needs the paid account, and its credentials would
have to be stored as a secret in the repository. It is the only way to remove
the right-click step.
