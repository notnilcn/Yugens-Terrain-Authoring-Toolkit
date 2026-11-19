# Release branch

This branch ships **built libraries** so the addon and the demo can be used immediately: clone,
open the project in Godot, press **F5**. No Rust toolchain and no build step. Everything else is
the same project as `rust-rewrite`.

```bash
git clone -b release --recursive <this repo> && cd <repo>
godot -e --path .        # first open imports the project; F5 plays demo/hex_island.tscn
```

## What is committed

Windows x86_64 libraries, built with `cargo build --release` (Rust 1.95, Godot 4.7.2, `api-4-7`),
installed where each `.gdextension` loads them:

| Library | Path (inside the addon / Phantom Camera submodules and this repo) |
|---|---|
| MarchingSquaresTerrain addon | `addons/MarchingSquaresTerrain/bin/marching_squares_terrain.dll` + `..._release.dll` |
| demo player (`MstDemoPlayer`) | `demo/bin/mst_demo.dll` + `mst_demo_release.dll` |
| Phantom Camera (GPL-3.0) | `addons/phantom_camera_rust/bin/phantom_camera_rust.dll` + `..._release.dll` |

The editor always loads the **debug** entry of a `.gdextension`, so the optimized release build is
installed under both names (a real debug build is 60–140 MB per library). Git stores the two names
as one blob, so the branch grows by the release build only. Debugging symbols are not included; to
develop the addon build real debug libraries with `tools/build.sh` (or `tools/build.ps1`), which
overwrites the `<lib>.dll` copies.

Only Windows x86_64 is committed: Linux and macOS need their own toolchain
(`tools/build.sh --release` on that platform, then copy the three libraries into the same paths).

## Updating this branch

1. Merge `rust-rewrite` into `release`.
2. Rebuild the release libraries: `tools/build.ps1 -Release` (Windows) or `tools/build.sh --release`.
3. Copy each `*_release` library over its debug-named twin and commit both in
   `addons/MarchingSquaresTerrain`, `addons/phantom_camera_rust` and `demo/bin`.
4. Commit the parent repo (this branch pins the new submodule commits).

## Publishing

The two submodule commits must be pushed **before** this branch, or a recursive clone cannot fetch
them:

```bash
git -C addons/MarchingSquaresTerrain push origin release
git -C addons/phantom_camera_rust push origin release
git push -u origin release
```
