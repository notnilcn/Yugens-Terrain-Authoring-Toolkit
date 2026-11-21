# Sculpting QA r2 — Brush / Level / Smooth / Bridge / toolbar / undo

Session `r2-sculpt` (display `:31`, port `8811`, Godot 4.7.2, llvmpipe, hex_island,
Hex Rings, window 1600x900). Re-run after two harness restarts; screenshots `01`–`07` and
the first `ui/*.json` dumps predate this pass. My own captures are `08`–`23`, `35`, `36`.
Note: other files appeared in this directory while the pass ran (e.g. `11-session-restart-terrain.png`,
`12-key2-viewport-focused.png`, `25`–`34`); they are **not** mine and are not cited below.
A second client was also driving **this same session/port** (`ps` showed
`python3 qa/drive.py ui --root all --save /tmp/opencode/qa/sessions/r2-sculpt/ui-b2.json` while the
session was live), which explains the unexplained tool reset to `Brush (Raise / Lower)`, the extra
screenshots and the stray `Save before closing?` modal. Findings that depend on editor state after
the mid-pass burst (especially the Level item) must be re-confirmed in a clean, exclusive session;
the digest sequence in the first undo finding was captured command-by-command and is reported as
observed, but concurrency cannot be fully excluded.

Editor log: `/tmp/opencode/qa/sessions/r2-sculpt/editor.log` (13 lines) contains only the known
startup noise — V-Sync on llvmpipe, Debug adapter/GDScript language-server ports, missing `.uid`
re-creation. No `ERROR` / `SCRIPT ERROR` / addon warning. **Log scan: clean.**

## Ctrl+Z after undo → redo → a new stroke removes two steps at once

- Severity: error
- Steps: 1) Brush → Drag; paint a 100-cell selection. 2) Drag from a selected cell
  `(880,350)` to `(880,320)` → digest `3064656926` → `1762056754`. 3) `Ctrl+Z` → `3064656926`,
  `Ctrl+Shift+Z` → `1762056754`. 4) Press `Esc` twice, then drag `(880,350)`→`(880,320)`;
  digest → `1147063117`, undo-state = `terrain height draw`, `version: 2`, `undo_count: 2`.
  5) One `Ctrl+Z`.
- Expected: one step back to the state before the last stroke, digest `1762056754`
  (`version: 1`).
- Actual: the single `Ctrl+Z` landed on `3064656926` — the pre-*first*-stroke baseline — i.e. it
  reverted both the new height draw and the earlier (undone/redone) move. This is the
  "one Ctrl+Z reverts more than one stroke" failure from the brief.
- Evidence: digests above; `qa/screenshots/r2-sculpt/15-brush-move-raised.png`,
  `17b-brush-move-after-full.png`, `18-brush-move-undone.png`,
  `20-esc-then-drag-status.png`; undo-state output `{"action":"terrain height draw",
  "version":2,"undo_count":2}` then digest `3064656926`.
- Undo: **broken** for the undo/redo/new-edit sequence. Plain one-stroke Ctrl+Z and
  Ctrl+Shift+Z do work (see Covered).

## Esc clears the visible selection but the next drag still performs a height draw

- Severity: warning
- Steps: 1) Brush → Drag, paint a selection (~100 cells). 2) Press `Esc` twice. 3) Note the
  status line has no `N selected` count and reads `LMB: select cells`. 4) Drag where the old
  selection was, `(880,350)`→`(880,320)`.
- Expected: with the selection cleared, the drag starts a **new selection**; terrain digest is
  unchanged.
- Actual: the status line after `Esc` no longer shows a count, but the following drag produced a
  `terrain height draw` undo entry (digest `1762056754` → `1147063117`) and ended with
  `1 selected` / `LMB: new sel...`. The selection is not fully reset, so an intended
  re-selection silently edits terrain. (The previous 2026-11-20 pass saw the white selection
  survive both Esc presses; here the surface state is partially cleared but the gesture state is
  stale — same class, different symptom.)
- Evidence: `qa/screenshots/r2-sculpt/19-esc-after-selection.png` (no count after Esc),
  `20-esc-then-drag-status.png` (`1 selected`, after a height-draw digest change);
  `qa/screenshots/r2-sculpt/21-after-rmb-esc-status.png` (RMB/Esc status).
- Undo: the inadvertent height draw was undone; the Esc behaviour itself is not undoable
  (selection state).

## Number keys 1–9 do not switch tools; digits go to the 3D view instead

- Severity: warning
- Steps: 1) Select the terrain (strip shows `Brush (Raise / Lower)`). 2) Click in the 3D
  viewport and press `2`. 3) Read `ToolTitle` and the viewport header.
- Expected: tool switches to Level (`docs/12 Cheat Sheet.md` "2 Level"; tooltips say
  `Brush Tool [1]`, `Bridge Tool [4]`).
- Actual: `ToolTitle` stays `Brush (Raise / Lower)`; the digit is consumed by the 3D view's
  view-orientation shortcut (header crop captured; `7`/`1` were already shown to switch
  Top/Front Orthogonal in the previous pass). Tools only change by clicking the toolbar.
- Evidence: `qa/screenshots/r2-sculpt/35-key2-viewport-header.png`; immediate
  `ui --filter ToolTitle` after `key 2` returned `Brush (Raise / Lower)`.
- Undo: not applicable.

## Level drags produced no terrain change; strip later showed Brush (Raise / Lower)

- Severity: warning (unconfirmed — session state was contaminated by a stray viewport click)
- Steps: 1) Re-select the terrain, click the `Level` toolbar button; strip title reads `Level`,
  `Height 0.0`, `Size 15.0`. 2) Drag `(850,340)`→`(900,360)`, then `(880,300)`→`(930,330)`
  (1.2 s each). 3) Check digest and undo-state. 4) Move the pointer over the island and capture
  the strip.
- Expected: two Level strokes flatten the cells toward `Height 0.0`, changing the digest and
  adding one undo step each.
- Actual: digest stayed `3064656926` after both drags; undo-state remained `Create Node`
  (`version: 2`, `undo_count: 1`) — no Level action was recorded. The next strip capture shows
  `Brush (Raise / Lower)` instead of `Level`, so the active tool/mode changed without a
  toolbar click. A mis-aimed click at the intended Height field landed in the 3D viewport and
  created a stray `Node` (unsaved), so the exact trigger of the tool reset is not isolated.
  Recorded as suspicious; re-test in a clean session before promoting to ISSUES.
- Evidence: `qa/screenshots/r2-sculpt/22-level-strip.png` (Level strip, Height 0.0),
  `23-level-hover-status.png` (strip = `Brush (Raise / Lower)`), digest unchanged.
- Undo: not applicable (no edit occurred).

## Brush status/hint line is clipped at the right edge at 1600x900

- Severity: ui
- Steps: 1) Brush → Drag. 2) Paint a selection. 3) Read the overlay hint/status at the bottom
  of the viewport.
- Expected: the full hint (`RMB / Esc: clear`, `Shift: add, Ctrl: remove, Alt: one cell`) fits
  or wraps.
- Actual: the line is cut at the viewport edge: `... RMB / Esc: c...` and after a move
  `... Shift: add, Ctrl: remove, RMB/Esc` with no `clear` suffix. The selection count/hint is
  custom-drawn, not a Control, so `ui` cannot measure it.
- Evidence: `qa/screenshots/r2-sculpt/14-brush-selection-painted.png`,
  `16-brush-selection2-strip.png`.
- Undo: not applicable.

## Covered

- [x] Toolbar tool clicks switch Brush / Level / Smooth / Bridge; `ToolTitle` and strip layout
  change; strips captured (`09-level-strip-full`, `10-smooth-strip-full`,
  `11-bridge-strip-full`, `12-brush-strip-full`; earlier `02`–`05` crops).
- [x] Brush → Drag selection paint: white band, status `183 selected`, digest unchanged
  (selection is not a terrain undo step).
- [x] Brush → Drag move: drag starting on a selected cell changed the digest
  (`3064656926`→`1762056754`), one `Ctrl+Z` restored `3064656926`, `Ctrl+Shift+Z` restored
  `1762056754` — plain undo/redo **works**.
- [x] RMB right-click clears the selection; Esc partially clears (finding above).
- [x] Undo action names reported correctly for the height draw (`terrain height draw`) once the
  undo manager had processed the step; early reads returned an empty `action` with
  `has_undo:false` immediately after the stroke (harness timing, not counted as a defect).
- [x] `1`–`9` shortcut failure independently re-confirmed (finding above).
- [x] 1280×800 resize accepted (window reported 1280×800); capture `36-1280x800-strip.png` is
  contaminated by a "Save before closing?" modal that appeared during the resize, so strip
  clipping at 1280×800 is **not** concluded.
- [x] Editor log scanned: no new errors/warnings (only known startup noise).

## Not tested

- Brush Raise/Lower: LMB raise, RMB lower, LMB+RMB flatten, Ctrl+LMB/MMB/RMB picking
  Ceiling/Height/Floor (blocked: the session's tool state reset to Brush Raise/Lower and time
  ran out; the strip itself was captured in `23-level-hover-status.png`).
- Brush settings: Brush Type footprints, Aspect, Rotation, Flatten on/off, Falloff, Peak,
  Grid Aligned, Grid Size, Quick Paint; Shift/Ctrl wheel size/strength and label refresh.
- Shift-add / Ctrl-remove / Alt-one-cell selection modifiers (screenshots `25`–`28` in the
  directory are from another writer, not verified here).
- Level height picking and actual flattening; Smooth Terraced vs Sloped and Strength; Bridge
  ramp/ease/width and its undo.
- Toolbar Collapse, Grid overlay toggle, tooltips.
- Undo after switching tools, after Save, after closing/reopening the scene; two-clean-stroke
  two-undo matrix in an uncontaminated session.
- 1280×800 strip clipping (modal blockage above).

---

## Addendum — interrupted second writer (same session)

This client started `r2-sculpt` at 04:33, was cut off by the platform restart, and was resumed at
04:40 to find a **replacement agent already driving the same session/display/port**. All input was
stopped as soon as that was detected (the replacement's own note above lists the collision). The
items below were captured by this client before/independently of the replacement and are offered
as corroboration; evidence numbers `25`–`32` are this client's crops, `01`–`07` its first-attempt
strips. No addon or scene file was edited, nothing was saved.

### Brush (Raise / Lower) strip clips the title and Brush Type row at 1600x900

- Severity: warning
- Steps: 1) Hex Rings terrain, Brush tool. 2) Set **Brush Mode** to `Raise / Lower` via the popup
  (control at 998,483 → item `Raise / Lower`). 3) Read the bottom strip; try to reach **Brush
  Type**.
- Expected: the whole strip is visible, or the strip scrolls, as `docs/03 Sculpting.md` lists
  **Brush Type** among the Brush settings.
- Actual: the flow content becomes 5 rows (175 px) inside the 140 px `ScrollContainer`
  (`MarchingSquaresToolAttributes`, 526,483 787×140) and is bottom-aligned, so the first row
  (`ToolTitle` = `Brush (Raise / Lower)`, 526,448 179×35, and `Brush Type`, 833,448) is laid out
  above the viewport and clipped. The visible strip starts at `Brush Mode` (631,483). No
  scrollbar is shown; **Brush Type** is unreachable until the user switches back to `Drag`.
  Re-confirms the 2026-11-20 finding.
- Evidence: `qa/screenshots/r2-sculpt/32-brush-raiselower-strip.png` (first visible row is
  `Brush Mode`); `qa/screenshots/r2-sculpt/ui/32-raiselower.json` (`ToolTitle`/`Brush Type`
  y=448, `Brush Mode` y=483).
- Undo: not applicable.

### Selection modifiers verified (Shift add, Ctrl remove, Alt one-cell)

- Severity: n/a (coverage)
- Steps: 1) Brush → Drag; drag a selection (`112 selected`, status crop `25`). 2) Shift+drag a
  second area → `225 selected` (`26`). 3) Ctrl+drag across part of the selection → `72 selected`
  (`27`). 4) Alt+drag from the selection → terrain digest `3064656926` → `1147063117`, undo-state
  `terrain height draw`, status ends `1 selected` (`28`).
- Expected: add / remove / one-cell move exactly as documented.
- Actual: all three behave as documented (Shift increases, Ctrl decreases, Alt edits a single
  cell). Note the Alt gesture replaces the selection with the single moved cell.
- Evidence: `qa/screenshots/r2-sculpt/25-selection-shift-base.png`,
  `26-selection-shift-add.png`, `27-selection-ctrl-remove.png`,
  `28-selection-alt-onecell.png`.
- Undo: not exercised for these three gestures (the Alt edit was left in the history; session
  ownership changed).

### Esc clearing the visible selection (refutes the 2026-11-20 symptom)

- After a fresh 51-cell selection (`29`) two `Esc` presses removed the `N selected` count and the
  status changed to `LMB: select cells` (`30`); a following RMB left it cleared (`31`). The old
  report's "white selection survives both Esc presses" did **not** reproduce here. The
  stale-gesture variant reported by the other writer above may still be real; treat that as the
  live issue, not "Esc does nothing".
- Evidence: `qa/screenshots/r2-sculpt/29-selection-fresh.png`,
  `30-esc-selection-after-two.png`, `31-rmb-selection-cleared.png`.

### Undo / redo (this client's observations)

- One Brush move stroke: `3064656926` → `1762056754` (`terrain height draw`); `Ctrl+Z` →
  `3064656926` with `has_redo: true`; the **first** `Ctrl+Shift+Z` did not change the digest, a
  second one restored `1762056754`. Because a second client was clicking the same editor at that
  moment, one swallowed shortcut press is not concluded as an addon defect; plain undo/redo
  otherwise worked.
- The `Raise / Lower` LMB-hold test (`33-rl-before.png`, `34-rl-raised.png`) produced no digest
  change and no undo entry because the replacement's `Save before closing?` modal was open and
  swallowed the press; it is **not** evidence about Raise/Lower behaviour.
- `Ctrl+Z` after switching tools / after clicking the 3D view was not re-run by this client after
  the collision was detected (the other writer's report covers part of this matrix).

### Log scan

- `/tmp/opencode/qa/sessions/r2-sculpt/editor.log` (13 lines): only the known startup noise
  (V-Sync on llvmpipe, debug-adapter / GDScript-LSP ports, missing `.uid` re-creation). No
  `ERROR` / `SCRIPT ERROR` / addon warning.

### Not tested by this client

- Brush Raise/Lower raise/lower/flatten/pick and the Ceiling/Height/Floor values.
- Brush Type footprints, Aspect, Rotation, Flatten, Falloff, Peak, Grid Size, Quick Paint.
- Shift/Ctrl wheel size/strength and the stale-label check.
- Level, Smooth (Terraced/Sloped, Strength), Bridge (ramp/ease/width) strokes and their undo.
- Toolbar Collapse, Grid overlay, tooltips, `1`–`9` keys (see the other writer's finding above).
- 1280×800 layout, Save/reopen undo, two-stroke two-undo matrix.
