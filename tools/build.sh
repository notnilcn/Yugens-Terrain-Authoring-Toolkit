#!/usr/bin/env bash
# Builds the GDExtension libraries and installs them where the .gdextension files expect them:
#   marching_squares_terrain (crate mst, addon submodule's rust/) -> addons/MarchingSquaresTerrain/bin/
#   mst_demo (demo-only player)                                 -> demo/bin/
#   phantom_camera_rust (demo-only, GPL-3)                      -> addons/phantom_camera_rust/bin/
# Debug libraries are installed as <lib>.<ext> (used by the editor and debug exports), release
# libraries as <lib>_release.<ext> (used by release exports / `godot --export-release`).
# Binaries are not committed (see .gitignore): build them on each platform with this script
# (tools/build.ps1 on Windows).
# Usage: tools/build.sh [--release | --ship | --all] [--no-demo]
#   (default)   debug libraries
#   --release   release libraries only (the editor still loads the debug ones)
#   --ship      release libraries only, installed as both <lib>_release.<ext> and <lib>.<ext>
#               (editor and exports load the optimized build; no debug libraries)
#   --all       debug and release libraries
#   --no-demo   only the MarchingSquaresTerrain addon (skips mst_demo and phantom_camera_rust)
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
addon_rust="$root/addons/MarchingSquaresTerrain/rust"
demo_rust="$root/rust"
target="$root/rust/target"
profiles=(debug); demo=1; ship=0
for a in "$@"; do
  case "$a" in
    --release) profiles=(release) ;;
    --ship) profiles=(release); ship=1 ;;
    --all) profiles=(debug release) ;;
    --no-demo) demo=0 ;;
    -h|--help) sed -n '2,16p' "$0"; exit 0 ;;
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
install_lib() { # <profile> <crate lib name> <dest dir> <also install as plain name: 0|1>
  local profile="$1" name="$2" dest="$3" also_plain="${4:-0}" suffix="" src out plain
  [[ $profile == release ]] && suffix="_release"
  mkdir -p "$dest"
  case "$(uname -s)" in
    MINGW*|MSYS*|CYGWIN*) src="$name.dll"; out="$name$suffix.dll"; plain="$name.dll" ;;
    Darwin) src="lib$name.dylib"; out="lib$name$suffix.dylib"; plain="lib$name.dylib" ;;
    *) src="lib$name.so"; out="lib$name$suffix.so"; plain="lib$name.so" ;;
  esac
  # Remove first so a hardlinked plain-name copy from an earlier --ship build is unlinked
  # instead of being written through.
  rm -f "$dest/$out"
  cp "$target/$profile/$src" "$dest/$out"
  echo "installed $profile $dest/$out ($(du -h "$dest/$out" | cut -f1))"
  if [[ $also_plain == 1 && $plain != "$out" ]]; then
    # Share the inode with the _release copy where the filesystem allows it, so a shipped
    # tree carries each library once. Fall back to a copy (e.g. FAT/exFAT).
    rm -f "$dest/$plain"
    if ln "$dest/$out" "$dest/$plain" 2>/dev/null; then
      echo "installed ship $dest/$plain (hardlink, optimized, no debug info)"
    else
      cp "$dest/$out" "$dest/$plain"
      echo "installed ship $dest/$plain ($(du -h "$dest/$plain" | cut -f1), optimized, no debug info)"
    fi
  fi
}
# Both workspaces share one target dir: the addon crates and the demo crates never overlap.
export CARGO_TARGET_DIR="$target"
for profile in "${profiles[@]}"; do
  flag=(); [[ $profile == release ]] && flag=(--release)
  (cd "$addon_rust" && cargo build -p mst "${flag[@]}")
  install_lib "$profile" marching_squares_terrain "$root/addons/MarchingSquaresTerrain/bin" "$ship"
  if [[ $demo == 1 ]]; then
    (cd "$demo_rust" && cargo build -p mst_demo -p phantom-camera-rust "${flag[@]}")
    install_lib "$profile" mst_demo "$root/demo/bin" "$ship"
    install_lib "$profile" phantom_camera_rust "$root/addons/phantom_camera_rust/bin" "$ship"
  fi
done
