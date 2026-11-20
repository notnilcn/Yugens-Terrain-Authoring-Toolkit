# QA harness for MarchingSquaresTerrain

Tools used to drive the **real Godot editor** (not a mock) and capture evidence for
`ISSUES.md` / `FEEDBACK.md`. Everything here is disposable QA scaffolding; no addon code is
changed.

## What runs

- `qa/harness/` — a `@tool` `EditorPlugin` enabled in `project.godot`. It listens on
  `127.0.0.1:$MST_QA_PORT` and answers newline-delimited JSON requests (control-tree dumps with
  screen rects, selection, scenes, synthetic input, undo state, terrain digests, screenshots).
- `qa/drive.py` — the client. Talks to the harness and injects **real X11 input** through XTEST
  (mouse + keyboard), so clicks/keys exercise the same code path as a person.
- `qa/session.sh` — starts an isolated test session: its own Xvfb display, its own XDG
  config/data/cache, its own copy of the project under `/tmp/opencode/qa/sessions/<name>/`,
  and the editor with the harness enabled.
- `qa/setup.sh` — checks prerequisites and builds the generated project template.
- `qa/resource-monitor.sh` — the ai-tools wave guard (CPU/RAM/disk samples and a
  `recommended_parallel_subagents` value), read from the ai-tools checkout's `linux` branch.

## Persistent scaffolding

Everything needed to reproduce a pass is in **this repository** (`qa/`); the generated state
lives under `$MST_QA_STATE` (default `/tmp/opencode/qa`) and is rebuilt from the checkout:

| Path | Kind |
|---|---|
| `qa/harness/` (plugin + `harness.gd`) | source, committed |
| `qa/drive.py`, `qa/session.sh`, `qa/runtime.sh`, `qa/setup.sh`, `qa/resource-monitor.sh` | source, committed |
| `qa/README.md`, `qa/FEATURE-MAP.md` | source, committed |
| `$MST_QA_STATE/project_base` | generated from the working tree (excludes `.git`,
  `rust/target`, `qa/screenshots`, `qa/findings`) |
| `$MST_QA_STATE/sessions/<name>` | per-run XDG dirs, project copy, editor log, `env.sh` |
| `$MST_QA_STATE/rmon`, `rmon-tools` | monitor state and the fetched ai-tools script |

The harness is enabled **only in the session copy** (its `project.godot` gets an
`[editor_plugins]` entry); the repository's own `project.godot` is never modified.

## Quick start

```bash
cd /home/clintonnguyen/g/code_examples/rrr

qa/setup.sh                     # prerequisites + project template
qa/resource-monitor.sh init     # wave limits (once)
qa/resource-monitor.sh start    # background samples

qa/session.sh start sculpt --display :31 --port 8811
source /tmp/opencode/qa/sessions/sculpt/env.sh

python3 qa/drive.py info
python3 qa/drive.py select --class MarchingSquaresTerrain
python3 qa/drive.py capture qa/screenshots/sculpt/01-toolbar.png

qa/session.sh stop sculpt
qa/resource-monitor.sh stop
```

Before each wave, `qa/resource-monitor.sh parallel` prints the recommended number of
concurrent subagents from the latest sample. Start a session **once** and keep it for the
whole test pass. Each session is CPU-heavy (llvmpipe); the orchestrator limits how many run
at once.

`qa/setup.sh --refresh` rebuilds the template from the current working tree; `--clean`
removes the generated state; `--check` validates the prerequisites and the built template.
Set `MST_QA_STATE` to move the generated state out of `/tmp` (for example on a machine where
`/tmp` is cleaned aggressively).

## Runtime (non-editor) sessions

The demo/tutorial scenes run as a plain game, without the harness:

```bash
qa/runtime.sh start play --display :44 --scene res://demo/hex_island.tscn
source /tmp/opencode/qa/runtime/<play>/env.sh   # or: qa/runtime.sh env play
python3 qa/drive.py shot qa/screenshots/runtime/01-hex.png
python3 qa/drive.py key F1
qa/runtime.sh stop play
```

`qa/runtime.sh` uses a disposable copy of the template, isolated XDG dirs, and its own
Xvfb display; `MST_WINDOW_MATCH` makes `qa/drive.py` target the game window (the editor
and game titles differ). Use `--width/--height` to test the pixel renderer at different
window sizes, and a separate display from any running editor.

## Commands (the ones you need)

All commands accept `--display` / `--port`, but the env file sets both.

| Command | Purpose |
|---|---|
| `ping`, `info` | Liveness; `info` returns window size/pos, scene, fps, selection. |
| `open-scene res://...` | Open a scene tab (wait ~5–10 s, then `select`). |
| `select --class MarchingSquaresTerrain` / `select --path PixelRenderer` | Select a node (paths are relative to the edited scene root; the root name prefix is optional). |
| `nodes MarchingSquaresTerrain` | List scene nodes of a class with paths. |
| `ui --filter TEXT --save out.json` | Dump visible Controls under the editor base control: rect, min size, text, tooltip, disabled, visible, focus, items, value. |
| `ui --root all --filter "..."` | Include editor windows/dialogs (use for popups and the Builder/Pixel World panels). |
| `ui --root scene --filter ...` | Dump the Controls inside the edited scene (game UI layers, not editor panels). |
| `click X Y` | Real click at window-content coordinates. |
| `click-text "Label"` | Find a visible Control by text (or tooltip/name) and click its center; `--dry-run` prints the rect without clicking; `--mod ctrl` for Ctrl+click. |
| `drag X0 Y0 X1 Y1 --duration 1.2` | Press, move smoothly, release. Use 1–2 s for canvas strokes on llvmpipe. |
| `key z --ctrl` / `key z --ctrl --shift` / `key 5` | Real key press/release. |
| `shot out.png` | Screenshot of the whole Xvfb screen (window at 0,0 plus desktop). Use for resizability evidence. |
| `capture out.png` | In-engine screenshot of the editor window (clean, no desktop). Preferred for UI evidence. |
| `resize 1280 800` | Resize the editor window (Xvfb screen is 1920×1080). |
| `windows` | List windows/dialogs: title, rect, resizable, min/max size. |
| `popups` | List **currently open** popup menus with each item's rect (use after clicking an OptionButton/MenuButton). |
| `undo-state` | Current undo action name, version, history count, has_undo/has_redo. |
| `undo` / `redo` | Direct undo-manager calls. **Do not use these to test the shortcut** — use `key z --ctrl`. They are for isolating a failure. |
| `terrain` | Terrain digest + chunk count + heights of chunk 0,0. Use before/after to prove an edit and its undo. |
| `terrain --chunk-x 2 --chunk-y 1 --verbose` | Specific chunk heights + per-chunk hashes. |
| `prop PROP --path NODE` / `set-prop PROP VALUE` | Read/write a node property (JSON value). |
| `call METHOD [JSON_ARGS...] --path NODE` | Call a method (`get_chunks()`, `get_chunk_data(...)`, etc.). |

`click` / `drag` / `key` take **window-content coordinates**. `ui` returns `rect` in exactly
those coordinates, so `ui` → `click rect.center` is always aligned. `shot` images are the full
screen; the window is at 0,0, so the same coordinates apply inside the window area.

## Driving dropdowns and menus

Popup items are not Controls, so `ui`/`click-text` cannot see them.

1. `popups` — list open popups and each item's rect (`pos` + item `rect`).
2. Or use the keyboard: `click-text "Round"` to open the `Brush Type` OptionButton, then
   `key down`, `key enter`.

## Undo / redo protocol (required for every editing feature)

1. `terrain` (or `undo-state`) → record digest/version.
2. Perform **one** stroke / one change.
3. `terrain` → digest must differ; `undo-state` → an action name, `has_undo: true`.
4. `key z --ctrl` → `terrain` should return to the recorded digest; `has_redo: true`.
5. `key z --ctrl --shift` → digest returns to the edited value.
6. Also test: two strokes then two Ctrl+Z (each stroke is one step), Ctrl+Z after switching
   tools, and Ctrl+Z after clicking in the 3D view (focus not in a LineEdit).

Addon edits go through `EditorUndoRedoManager`; a broken step shows as no digest change after
Ctrl+Z, or one Ctrl+Z reverting more than one stroke.

## Screenshots and findings

See [`SCREENSHOTS.md`](SCREENSHOTS.md) for the per-area index, counts and the key evidence.

- Screenshots: `qa/screenshots/<area>/NN-short-description.png`. Take one **before** and one
  **after** for each visual change; one extra for each defect.
- Findings: `qa/findings/<area>.md`, one section per finding:

```markdown
## Finding title
- Severity: error | warning | minor | ui
- Steps: 1) ... 2) ...
- Expected: ...
- Actual: ...
- Evidence: qa/screenshots/<area>/03-thing.png ; editor.log line "ERROR: ..."
- Undo: works / broken / not applicable
```

- Also append a `## Covered` checklist of what worked, so coverage is auditable.
- Scan the session log for new noise before/after a test:
  `/tmp/opencode/qa/sessions/<name>/editor.log`.

### Known startup noise (ignore)

- `Missing .uid file ... re-created from cache`
- `Failed to start Debug adapter server on port 6006: Already in use`
- `Failed to start GDScript language server on port 6005: Already in use`
- `Could not set V-Sync mode ...` (llvmpipe)
- `Detected another project.godot ...` (only from a previously nested copy; should be gone)

Anything else, especially `ERROR:` or `SCRIPT ERROR:` from `mst`/`MarchingSquares`, is a
finding.

## Session facts

- The template project boots on `res://tutorial/02_island.tscn` (restored tabs). It has
  `Island/MarchingSquaresTerrain`, `PixelWater`, `PixelSky`, `PixelDayNight`, `PixelRenderer`.
- `res://demo/hex_island.tscn` is the richest sandbox (hex grid, grass, many nodes).
- `res://tutorial/03_pixel_look.tscn` and `04_world_mood.tscn` are useful for pixel/effects.
- The editor runs at ~5–10 fps under llvmpipe. Wait 1–2 s after clicks that rebuild panels;
  use 1–2 s drags; don't hammer commands.
- One editor per session/display/port; never reuse another agent's session. Never touch the
  real repo's `project.godot` or scenes — work only in the session copy.

## Rules

- Test, do not fix. Do not edit files under `addons/MarchingSquaresTerrain/`.
- Only write under `qa/screenshots/<area>/` and `qa/findings/<area>.md`.
- Stop your session when your pass is complete.
