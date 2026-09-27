# AGENTS.md — Yūgen's Terrain Authoring Toolkit (dev repo)

This repo is the development project for the [`addons/MarchingSquaresTerrain`](addons/MarchingSquaresTerrain/) editor plugin. It contains the plugin, demo scenes, tests, and CI. The plugin is distributed by copy-pasting its folder into another project's `addons/`; the addon has its own `AGENTS.md` documenting its internals — read it before changing plugin code.

## Layout

| Path | Role |
|---|---|
| `addons/MarchingSquaresTerrain/` | The plugin. See its `AGENTS.md`. |
| `scenes/mst_demo_scene.tscn` | Main scene (`project.godot`), demo terrain. |
| `scenes/mst_demo_presets.tscn` | Texture-preset demo. |
| `scenes/*_TerrainData/` | Committed demo terrain data (`chunk_<x>_<y>/metadata.res`, per grid-mode subfolder). |
| `scripts/` | Demo helpers (`demo_scene.gd`, `rotating_camera.gd`). |
| `tests/` | `run_*.gd` checks and `capture_*.gd` visual captures, all plain `SceneTree` scripts. |
| `addons/open_godot_mcp/` | Junction to `C:\Users\Clinton\g\main\client\addons\open_godot_mcp`; local MCP bridge, not part of this project. Don't edit or commit through this repo. |
| `.github/workflows/run_tests.yml` | CI: GdUnit4 action over `tests/`, on `main`/`public-testing`. |
| `addons/MarchingSquaresTerrain/documentation/` | Contributor docs: `plugin_quick_guide.md`, `documentation+/code_style_guide.md`, `internal_tool_system.md`, `code_locations.md`. |

## Godot and commands (this machine)

Godot 4.7.1 mono console build: `C:\Users\Clinton\g\Godot\Godot-stable_mono_win64_console.exe` (use the `_console` variant so output reaches the terminal).

```powershell
# Parse/import check (headless, no GPU). Run after script or shader edits.
& "C:\Users\Clinton\g\Godot\Godot-stable_mono_win64_console.exe" --headless --editor --quit --path .

# Run the demo (windowed, real GPU).
& "C:\Users\Clinton\g\Godot\Godot-stable_mono_win64_console.exe" --path .

# One addon behaviour test (from the repo root).
& "C:\Users\Clinton\g\Godot\Godot-stable_mono_win64_console.exe" --headless --path . --script res://tests/run_grid_tests.gd
```

- Most `run_*.gd` tests are headless; `run_cell_grass_tests.gd` and all `capture_*.gd` need a windowed GPU run. Captures take `-- --capture` and write PNGs to `user://`.
- Test scripts print results and exit non-zero on failure.
- CI uses the GdUnit4 action, but the checked-in tests are plain `SceneTree` scripts — the `--script` invocations above are the reliable local path.

## Plugin facts (duplicated from the addon's AGENTS.md)

- Grid types `SQUARE`/`TRIANGLE`/`HEX` are switchable per terrain; all painting tools work on all three.
- Terrain chunks are saved outside the scene in per-mode subfolders (`square/`, `triangle/`, `hex/`) under the terrain's `data_directory`, one `chunk_<x>_<y>/metadata.res` per chunk. Directories are generated next to the owning scene as `<SceneName>_TerrainData/<NodeName>_<uid>/`.
- Editor tools: brush, level, smooth, bridge, grass mask, vertex paint, debug brush, chunk management, terrain settings (`MarchingSquaresTerrainPlugin.TerrainToolMode`).
- Contributor docs live in `addons/MarchingSquaresTerrain/documentation/`; code style is `documentation+/code_style_guide.md` (tabs, `MarchingSquares`/`MST` prefixes).
- Keep the addon self-contained; it must never depend on anything outside `res://addons/MarchingSquaresTerrain/`.

## Pitfalls

- **Editor sessions rewrite terrain data.** Opening the demo scenes can regenerate/relocate `scenes/*_TerrainData/` folders and `.tscn` files (see commit `912ba97`). Always `git status` those paths after running the editor and only commit intended changes.
- `.gitignore` ignores `square/`, `triangle/`, `hex/` (which also matches `addons/MarchingSquaresTerrain/algorithm/hex/`), plus `*.uid` and `*.import`. New files under `algorithm/hex/` need `git add -f`.
- `addons/open_godot_mcp` is a junction into another repo — changes there are not this repo's changes.
- Windowed Godot runs can hang; use a timeout and kill leftover Godot processes.

## Verification

- No formatter/linter: the parse check plus the relevant `run_*.gd` tests are the verification.
- For rendering changes (shaders, grass, cell meshes), run a windowed `capture_*.gd -- --capture` and inspect the PNG.
- Commit style: short imperative summaries, sometimes `M<n>:` milestone prefixes.
