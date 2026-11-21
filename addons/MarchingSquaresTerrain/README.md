# MarchingSquaresTerrain (addon payload)

This branch carries the contents of `addons/MarchingSquaresTerrain/` at its **root**, so the
parent repository can consume it as a submodule:

| Parent path | Submodule URL | Branch |
|---|---|---|
| `addons/MarchingSquaresTerrain` | `https://github.com/notnilcn/Yugens-Terrain-Authoring-Toolkit-rust` | `addons` |

The addon's Rust sources are in `rust/` (`pt_core` + `mst`), with their own Cargo workspace and
lockfile. The demo-only `mst_demo` and GPL-3.0 `phantom_camera_rust` crates stay in the parent
repo and are not part of this addon.

After cloning the parent repository, run:

```bash
git submodule update --init --recursive
tools/build.sh --no-demo    # addon only; omit --no-demo to build the demo libraries too
```

To build the addon on its own (the `.gdextension` loads a prebuilt `bin/`, which is ignored):

```bash
cd rust
cargo build --release
# copy target/release/<lib> to ../bin/ with the platform/name the .gdextension lists
# (libmarching_squares_terrain.so / .dylib, or marching_squares_terrain.dll;
#  add `_release` before the extension for release exports).
```

The parent `tools/build.sh` / `tools/build.ps1` install the correct names for debug and release.
Source is MIT; see `CREDITS.md` in the parent repository for reference attributions.
