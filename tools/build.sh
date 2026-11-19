#!/usr/bin/env bash
# Builds the GDExtension libraries and installs them where the .gdextension files expect them:
#   marching_squares_terrain (crate mst, addon submodule's rust/) -> addons/MarchingSquaresTerrain/bin/
#   mst_demo (demo-only player)                                 -> demo/bin/
#   phantom_camera_rust (demo-only, GPL-3)                      -> addons/phantom_camera_rust/bin/
# Debug libraries are installed as <lib>.<ext> (used by the editor and debug exports), release
# libraries as <lib>_release.<ext> (used by release exports / `godot --export-release`).
# Binaries are not committed (see .gitignore): build them on each platform with this script
# (tools/build.ps1 on Windows).
# Usage: tools/build.sh [--release | --all] [--no-demo]
#   (default)   debug libraries
#   --release   release libraries only (the editor still loads the debug ones)
#   --all       debug and release libraries
#   --no-demo   only the MarchingSquaresTerrain addon (skips mst_demo and phantom_camera_rust)
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
addon_rust="$root/addons/MarchingSquaresTerrain/rust"
demo_rust="$root/rust"
target="$root/rust/target"
profiles=(debug); demo=1
for a in "$@"; do
  case "$a" in
    --release) profiles=(release) ;;
    --all) profiles=(debug release) ;;
    --no-demo) demo=0 ;;
    -h|--help) sed -n '2,14p' "$0"; exit 0 ;;
    *) echo "unknown option: $a" >&2; exit 2 ;;
  esac
done
require_addon() { # <file that proves the submodule is checked out> <label>
  if [[ ! -f "$root/$1" ]]; then
    echo "error: $1 is missing ($2); run: git submodule update --init --recursive" >&2
    exit 1
  fi
}
require_addon addons/MarchingSquaresTerrain/marching_squares_terrain.gdextension "the addon is a git submodule"
require_addon addons/MarchingSquaresTerrain/rust/Cargo.toml "the addon ships its Rust sources"
if [[ $demo == 1 ]]; then
  require_addon addons/phantom_camera_rust/phantom_camera_rust.gdextension "the demo dependency is a git submodule"
fi
install_lib() { # <profile> <crate lib name> <dest dir>
  local profile="$1" name="$2" dest="$3" suffix="" src out
  [[ $profile == release ]] && suffix="_release"
  mkdir -p "$dest"
  case "$(uname -s)" in
    MINGW*|MSYS*|CYGWIN*) src="$name.dll"; out="$name$suffix.dll" ;;
    Darwin) src="lib$name.dylib"; out="lib$name$suffix.dylib" ;;
    *) src="lib$name.so"; out="lib$name$suffix.so" ;;
  esac
  cp "$target/$profile/$src" "$dest/$out"
  echo "installed $profile $dest/$out ($(du -h "$dest/$out" | cut -f1))"
}
# Both workspaces share one target dir: the addon crates and the demo crates never overlap.
export CARGO_TARGET_DIR="$target"
for profile in "${profiles[@]}"; do
  flag=(); [[ $profile == release ]] && flag=(--release)
  (cd "$addon_rust" && cargo build -p mst "${flag[@]}")
  install_lib "$profile" marching_squares_terrain "$root/addons/MarchingSquaresTerrain/bin"
  if [[ $demo == 1 ]]; then
    (cd "$demo_rust" && cargo build -p mst_demo -p phantom-camera-rust "${flag[@]}")
    install_lib "$profile" mst_demo "$root/demo/bin"
    install_lib "$profile" phantom_camera_rust "$root/addons/phantom_camera_rust/bin"
  fi
done
