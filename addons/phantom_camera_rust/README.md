# Phantom Camera (Rust port)

Rust GDExtension port of [Phantom Camera](https://github.com/ramokz/phantom-camera) `0.11.0.3`
(MIT, Marcus Skov) for Godot 4.7, shipped as a self-contained second GDExtension library.

> **In the Marching Squares Terrain repository** it is a *demo-only*, GPL-3.0 dependency (see
> `LICENSE`): it drives the demo player camera (`demo/player.tscn`, `MstDemoPlayer` in
> `rust/crates/mst_demo`). The MarchingSquaresTerrain addon (crate `mst`) does not link or
> reference it. `project.godot` autoloads `PhantomCameraManager` from
> `phantom_camera_manager.tscn`; scenes name the native classes directly
> (`type="PhantomCamera3D"`, …) with no GDScript bindings.

## Install / build

This repository holds the addon files only; the crate lives in the host project
(`rust/crates/phantom_camera_rust` in Marching Squares Terrain). Its `tools/build.sh` /
`tools/build.ps1` (skip with `--no-demo` / `-NoDemo`) build it and install the library here:

```bash
tools/build.sh                                                   # from the host project root
godot --headless --path . --editor --quit                        # refresh .godot/extension_list.cfg
godot --headless --path . res://addons/phantom_camera_rust/dev/probe_check.tscn
```

Library: `res://addons/phantom_camera_rust/bin/[lib]phantom_camera_rust.{dll,so,dylib}` (debug)
and the `_release` twin. `bin/` is build output and is not committed (see `.gitignore`).

## Classes

| Class | Base | Notes |
|---|---|---|
| `PhantomCamera2D` | `Node2D` | `NONE`/`GLUED`/`SIMPLE`/`GROUP`/`PATH`/`FRAMED`, damping, lookahead, dead zones, limits, noise, tween-on-switch |
| `PhantomCamera3D` | `Node3D` | 2D's modes plus `THIRD_PERSON` (runtime `SpringArm3D`), look-at modes, `Camera3DResource` overrides |
| `PhantomCameraHost` | `Node` | child of the `Camera2D`/`Camera3D`; priority arbitration, activation signal order, tween state machine, per-frame pump |
| `PhantomCameraManager` | `Node` | autoload singleton: registries + 16 fan-out signals; also `Engine.get_singleton("PhantomCameraManager")` |
| `PhantomCameraNoiseEmitter2D/3D` | `Node2D`/`Node3D` | trauma grow → hold → decay state machine |
| `PhantomCameraNoise2D/3D` | `Resource` | `FastNoiseLite` perlin, positional/rotational multipliers |
| `PhantomCameraTween` | `Resource` | `duration`/`transition`/`ease`, ordinals equal to Godot's `Tween` enums |
| `PhantomCameraTweenDirector` | `Node` | data container consulted by the host |
| `TweenDirectorResource` | `Resource` | From/To matching for conditional tweens |
| `Camera3DResource` | `Resource` | `Camera3D` overrides applied on activation |
| `PcamRustProbe` | `Node` | port/upstream version + registered-class list (harness smoke check) |
| `PcamRecorder` (dev) | `Node` | parity recorder (port source repository harness) |

All exported property names, defaults, signal names/arities, `#[func]` signatures and enum integer
values match the GDScript original: the demo scenes are the ABI test.

## Scope

Ported: every runtime behavior the 18 demo scenes exercise, including the tween/easing math
(Godot's `Tween.interpolate_value` table, verified numerically) and the oracle's `_smooth_damp`.

Not ported, deliberately:

- **Editor layer** — `plugin.gd`, the inspector plugin, the 3D gizmo plugins, the bottom-panel
  viewfinder dock, the self-updating updater panel, themes and icons. None of it is on a game path,
  and it is all editor UI rather than camera behavior. `show_viewfinder_in_play` exists as a stored
  property and is inert.
- **`@tool` niceties** — `_validate_property` inspector hiding, `_get_configuration_warnings`,
  frame-preview `_draw`, gizmo line drawing, the "Align with view" tool buttons.
- **C# facade files** — the oracle's 12 `.cs` files were never Godot scripts and are not compiled
  in this project.

Behavioral quirks preserved on purpose (parity includes bugs): the 3D `FRAMED` dead branch whose
"rotation changed" comparison is unreachable, the host tween diff reading two wrong `_prev_cam_*`
values, the inert `keep_aspect` override, and 2D `FRAMED`'s incremental (not absolute) dead-zone
mutation. Each is marked with a `// parity:` comment naming the oracle line.

## Parity benchmark

The parity harness belongs to the port's source repository, which keeps the unmodified GDScript
addon as the oracle (`parity/pcam_oracle/phantom_camera/`) and holds
`dev/abi_dump.gd`'s recorded `abi_gd.json`. This repository ships only the Rust implementation's
addon files; the oracle is not vendored here, because its `class_name`s would hide the native
classes. In the source repository both sides run the same demo scenes and driver scripts, so a
diff isolates the addon implementation.

## Layout

| Path | What |
|---|---|
| `phantom_camera_rust.gdextension` | library mapping (`res://addons/phantom_camera_rust/bin/…`; upstream uses `res://bin/`) |
| `phantom_camera_manager.tscn` | the autoload node |
| `LICENSE` / `LICENSE-PhantomCamera-MIT` | GPL-3.0 (this port) / upstream MIT text |
| `dev/probe_check.tscn` | `PcamRustProbe` smoke scene |
| `bin/` | built libraries (ignored) |
| `PORT-PLAN.md` | port contract |
