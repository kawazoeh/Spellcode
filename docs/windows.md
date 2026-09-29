# Spellcode on Windows — build and packaging notes

Scope: what is needed to build and ship the Windows x86_64 binary. The
release workflow (`.github/workflows/release.yml`) implements the steps below.

## Renderer

GPUI 0.2.2 on Windows draws through **Direct3D 11**, not WebGPU and not
Vulkan:

- `gpui` 0.2.2 `src/platform/windows/directx_devices.rs`,
  `directx_renderer.rs`, `directx_atlas.rs` — D3D11 device, swap chain and
  atlases (`windows` crate features `Win32_Graphics_Direct3D11`,
  `Win32_Graphics_Dxgi`, `Win32_Graphics_DirectComposition`).
- Text uses DirectWrite (`src/platform/windows/direct_write.rs`).
- Shaders are HLSL, compiled at build time with `fxc.exe` (shader model 4.1)
  — see `build.rs`, `mod windows`, `compile_shaders()`.

There is no software rasteriser fallback: the runtime needs a D3D11-capable
GPU. DirectComposition is used for composition.

## Build-time requirements

| Requirement | Why | Provided by |
| --- | --- | --- |
| Rust stable, `x86_64-pc-windows-msvc` | MSVC ABI | `dtolnay/rust-toolchain@stable` |
| MSVC build tools (VS 2022 or 2026) | C/C++ linker | `windows-latest` image |
| Windows SDK with `fxc.exe` | gpui compiles HLSL in release builds | `windows-latest` image |
| `fxc.exe` discoverable | gpui fails with `Failed to find fxc.exe` otherwise | see below |

`gpui`'s `build.rs` looks for `fxc.exe` in this order:

1. the `GPUI_FXC_PATH` environment variable (if the file exists);
2. `where.exe fxc.exe` on `PATH`;
3. a hard-coded path
   `C:\Program Files (x86)\Windows Kits\10\bin\10.0.26100.0\x64\fxc.exe`.

The Windows SDK version directory changes between runner images, so the
workflow globs `C:\Program Files (x86)\Windows Kits\10\bin\*\x64\fxc.exe`,
takes the newest, and exports `GPUI_FXC_PATH`. This is the one environment
fix that keeps the Windows build from being red.

The shaders are compiled only in release mode (`#[cfg(not(debug_assertions))]`),
so a debug build does not need `fxc.exe`.

## Feature selection

`crates/spellcode-app` enables gpui's `macos-blade` renderer by default. That
feature pulls `objc2` / `objc2-metal`, which are macOS-only; the Windows build
therefore runs with `--no-default-features`. gpui keeps its own default
features (`windows-manifest`, …) because they are dependency defaults, not
this crate's.

## Runtime requirements

- Windows 10 1809+ (64-bit): `portable-pty` 0.9 uses the Windows ConPTY API.
- Direct3D 11 GPU.

Not verified here (the build was never run on Windows): the exact D3D11
feature level accepted, and whether `WindowBackgroundAppearance::Blurred`
(the translucent tab bar) is honoured on Windows.

## Packaging

Shipped as a `.zip` (`Spellcode-windows-x86_64.zip`). The zip contains:

- `spellcode-app.exe`;
- `icon.png` (the app icon as a standalone file);
- `README.txt` (from `packaging/windows/INSTALL.txt`).

A real installer is realistic but is **not** done here:

- **MSI** via WiX, driven by `cargo-wix`, would produce
  `spellcode-app-<version>-x86_64.msi` with Start-menu shortcuts.
- **NSIS** would produce a small `setup.exe`.

Either one needs an `.ico` and an app-level resource script (`.rc`) to embed
the icon into the executable. Embedding is deliberately left out: gpui's own
`build.rs` owns the Windows resource compilation, and adding a second one
would touch the build of the application, which is out of scope. Distribute
the zip as-is, or add the installer after the repository is public.

Code signing is also needed to avoid SmartScreen warnings; that requires a
certificate that is not available in the current setup.
