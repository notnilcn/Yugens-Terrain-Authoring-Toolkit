# r7 — V1: editor shell verification (r7 fixer claims + regression sweep)

Session `r7-v1` (display `:91`, port `8891`, window 1600×900, resized to 1280×720 and
1024×600 and back). Build `/home/clintonnguyen/g/code_examples/rrr-wt/r4base`, HEAD
`3474990` (`qa/runtime.sh` import fix on top of `4b6811e`, the merge of `work/r7`).
Godot 4.7.2, llvmpipe, editor scale 1.0, scene `res://demo/hex_island.tscn`.
Template was pre-refreshed; nothing was built. Session **stopped** at the end.

Every number below is read back from `qa/drive.py ui --root all` dumps (copied to
`qa/screenshots/r7-editor/ui/`), from `qa/drive.py windows`, from `control_at` probes over a
persistent harness socket (`qa/scripts/r7-editor/probe.py`), or from the state file
`.godot/editor/mst_editor_state.cfg`.

Four fresh sessions were needed: R6-1 (first paint) and R6-4 (Settings high-water) are both
first-entry-in-a-session effects, and this box runs the editor at 1–30 fps depending on load.

## Headline

1. **R6-1 is NOT fixed and is not "not reproduced".** The zero-size first paint
   (`39×0 / 0×35 / 250×0`) still happens whenever the first `Pixel*` selection lands while the
   editor is still in its slow first frames (fps 1–2). In four fresh-session samples the pixelizer
   appeared at **+0.98 s** (fps 29), **+2.3 s** (fps 24), **+4.7 s** (fps 1–2, zero frame still
   present at +2.2 s), and in one sample it was **still absent at +5.74 s** with the 3D view
   spanning the full main screen — only a second `select` built it. See **E1-1**.
2. **R6-2 passes**: the first page-tab build is two rows `405×66`, `Debug Views` inside the
   strip, nothing clipped at 1600 or 1280.
3. **R6-3 passes**: Settings → Layout at 1600 is two columns, **no H scrollbar**, zero controls
   past the strip's right edge (r6: 26 px clip on a 929 px flow).
4. **R6-4 only half-passes**: *reopening* from Paint gives **138 px** exactly as claimed, but the
   **first** Settings open of a session still inherits Paint's **175 px** in two independent
   samples — the case R6-4 actually raised.
5. **R6-5 and R6-6 pass**: at 1024 the page tabs are one 33 px `OptionButton` (9 items) and the
   body is fully usable; the Paint `Rotation` spin slider wheel-steps 0→2→3→2.
6. **N24 unchanged** (as claimed): `EditorInspector [891,180 220×416]`, right edge **1111** in a
   1024 window → 87 px overhang.
7. **Boot noise unchanged**: 79-line log, 2× `create_frustum_points` `ERROR:` + 16 plane + 16
   vector warnings, all before any selection; **no new log line of any kind** during the pass.

## r7 verification

| # | Item | Verdict | Measured |
|---|---|---|---|
| 1 | **R6-1** first `Pixel*` paint, fresh session, 1600×900 | **FAIL** (claim "not reproduced" is wrong) | 4 samples (each a new session copy): (a) settled session at fps 29: panel content hit at **+0.98 s**, not at +0.39 s; (b) fps 24: view still `[287,121 1026×732]` (full width) at +0.27 s, tabs already at y=678, panel present by +2.3 s; (c) select fired right after the scene appeared, editor at ~1 fps: `select` itself blocked 2.50 s, then **no rail/strip/panel at all** at +2.49/+3.38/+4.79/+5.74 s (`r61-t00…t07.png`, the 3D view spans the whole main screen), later re-`select` builds it in <3 s (`r61-reselect.png`); (d) fps 1–2: probe at (1100,300) hits `@SubViewportContainer` (**outside** the panel) at +2.21 s and a panel `Button` at +4.71 s. Settled rects when it does build: rail `39×732`@287,121, strip `731×173…175`@329,678, panel `250×732`@1063,121, tabs `405×66`. |
| 2 | **R6-2** first-build tab row | **PASS** | 1600: `SectionPages [432,678 405×66]`; row 1 `Renderer 77 / Outlines 71 / Color / Palette 111 / Rays 104 / Sun 42` (y=678), row 2 `Sheets 77 / Clouds 71 / Depth Fade 111 / Debug Views 104` (y=711); last tab right edge **795** vs strip right **1060**, 0 controls past it. 1280: identical rects, strip `632×…`, right edge 795 vs 961. 1024: single dropdown (item 5). |
| 3 | **R6-3** Settings → Layout at 1600 | **PASS** | strip `[410,678 903×175]`; `SettingsPage 657×175`, `SettingsColumns 657×175` (two columns); HScrollBar count **0**; 0 controls past x=1313; `@MstFlow 889×212` (35 px tabs + 177 px body) → vertical scroll only (`_v_scroll [1299,678 14×175]`). Only `Blend Mode:` (`[410,855 …]`, strip bottom 853) sits below the fold and is reachable by scrolling. |
| 4 | **R6-4** Settings strip height | **PARTIAL** (claim true only on reopen) | first open from Paint: **175** (`min 175`) in two separate fresh sessions; every page 175. Reopen from Paint: **138** (`min 138`) — matches the claim. Chunks → Settings (Storage remembered): **107**. High-water retention after visiting Layout (175) is then 175, because the two-column Layout page is now 175 tall (r6: 142). |
| 5 | **R6-5** pixelizer at 1024 | **PASS** | strip `[221,454 632×99]`; `SectionPages` is an **OptionButton** `[324,455 168×33]`, 9 items (Renderer … Debug Views); body rows `Enabled/Mode/Pixel Size` + `Fixed Height/Textel Snap/Sub-textel Scroll` fully inside the 66 px body; switching the dropdown to `Depth Fade` rebuilds the body (`Start Depth 5.9 / Depth Fade 8.5 / Depth Power 1.35`) in the same 99 px strip. Rail `39×399`, panel `32×399`. |
| 6 | **R6-6** spin-slider wheel | **PASS** | Paint `Rotation` `EditorSpinSlider [490,751 80×29]`, value `0.0` → wheel up ×2 → **2.0** → wheel up ×1 → **3.0** → wheel down ×1 → **2.0** (one step per notch, snapped). Sibling HSliders (`Size` 15.0, `Aspect` 1.0085) unchanged by the same probes. |
| 7 | **N24** Inspector minimum at 1024 | **UNCHANGED** (claim: blocked by Godot) | 1024×600: `EditorInspector [891,180 220×416]`, right edge **1111** → 87 px past the window; `InspectorDock [897,82 208×508]`; inspector rows visibly cut in `r65-01-pixelizer-1024.png`. The claimed cause (709 px main-screen minimum / 701 px 3D toolbar) cannot be read back through the harness — `drive.py windows` never lists the main editor window, so only the overhang is measured. |
| 8 | **N6/N31** boot noise | **UNCHANGED** (r7: "not attempted") | `editor.log` = **79 lines** at boot and **79 at session end**: 2× `ERROR: Condition "!res" … create_frustum_points (rendering_light_culler.cpp:682)` (lines 28, 62) + **16** `Plane cannot be normalized` + **16** `Vector3 cannot be normalized` (lines 12–78), all after `MST QA harness listening` and before any selection. Zero further `ERROR:`/`SCRIPT ERROR:` for the whole pass (two session starts sampled identically). |

## Findings

### E1-1. First `Pixel*` selection still paints a zero-size pixelizer UI — warning
- **Severity:** warning (r6 R6-1 still open; the r7 claim "not reproduced" does not hold)
- **Steps:** 1) Fresh session, 1600×900, `hex_island`. 2) `select --path PixelRenderer` as soon
  as the scene appears (editor still compiling/restoring, `info` fps 1–2). 3) Screenshot every
  ~0.5 s, or probe `control_at 1100 300`.
- **Expected (claim):** the first rendered frame is already laid out.
- **Actual:** the rail/strip/panel do not exist yet; the main screen shows the plain 3D view
  full width (`@SubViewportContainer [287,121 1026×732]`) and the tab row is either absent or
  present while the panel is not. Timing by sample: **≤1 s** at fps 29, **≈2.3 s** at fps 24,
  **≈4.7 s** at fps 1–2, and in one run **>5.74 s with nothing built** — a second `select` then
  builds the correct `39×732 / 731×175 / 250×732` layout in under 3 s. Nothing is logged.
- **Evidence:** `qa/screenshots/r7-editor/r61-t00.png` … `r61-t07.png` (select at +2.49 s, still
  absent at +5.74 s), `r61-reselect.png` (same session after one more `select`),
  `r61-early-t1/t4/t8.png`, `r61-quiet-timeline.txt`, `qa/scripts/r7-editor/{probe,firstpaint,early_firstpaint}.py`.
- **Undo:** not applicable (layout).

### E1-2. Settings' first open in a session still inherits Paint's 175 px strip — minor
- **Severity:** minor/UI (r6 R6-4's exact scenario; the r7 claim only covers the reopen)
- **Steps:** 1600×900, select the terrain, Paint tool (strip `650×175`), then Settings → any page.
- **Expected:** Settings' own high-water (Layout = 175 after the two-column rework, or the
  remembered page height: Storage 107).
- **Actual:** **175** (`min [0,175]`) on every page on the first entry, in two independent fresh
  sessions; only the second entry onward uses the tool's own height — Paint → Settings again
  gives **138**, Chunks → Settings gives **107**.
- **Evidence:** `r63-01-settings-first-open.png`, `ui/r63-settings-first-open.json`,
  `r64-03-first-open-second-sample.png`, `ui/r64-firstopen-2ndsample.json`,
  `ui/r64-settings-reopen-138.json`, `ui/r64-settings-second-entry-107.json`.
- **Undo:** not applicable.

### E1-3. Settings → Layout needs one wheel notch to show its last row at 1600 — note
- **Severity:** minor/UI (not a regression; the r7 claim says "212 px content in 175 px scrolls
  vertically", so this is expected)
- **Actual:** the fifth row `Blend Mode:` sits at `[410,855 303×35]`, 2 px below the strip bottom
  (853); 15 controls cross the bottom edge and are clipped until scrolled (`_v_scroll` only).
  r6's horizontal 26 px clip is gone.
- **Evidence:** `r63-02-settings-layout-1600.png`, `ui/r63-settings-layout-1600.json`.
- **Undo:** n/a.

## Covered

- [x] R6-1: 4 fresh-session first-paint samples (probe timeline + screenshot timeline +
      `select`-blocked timing); settled rects at 1600; reselect recovery.
- [x] R6-2: first page-tab build at 1600 (two rows, no overflow, `Debug Views` inside the strip)
      and at 1280 (identical rects); dropdown form at 1024 incl. a live page switch.
- [x] R6-3: Settings → Layout at 1600 — columns, HScrollBar count, right-edge overflow, vertical
      scroll, last-row position.
- [x] R6-4: first open from Paint (175, twice), reopen from Paint (138), entry from Chunks (107),
      full six-page sweep at 175.
- [x] R6-5: strip/rail/panel rects at 1024, dropdown items, body rows, page switch to Depth Fade.
- [x] R6-6: Rotation spin slider wheel up ×2 / up ×1 / down ×1 with value readback.
- [x] N24: Inspector dock and `EditorInspector` rects at 1024.
- [x] N6/N31: boot log counted at two different session starts; no log growth during the pass.
- [x] Regression: Settings six pages (no H scrollbar, no overflow), Chunks (`903×105`, hint +
      status above the strip, 295 chunks), Storage page (Storage Mode / Bake / Import / Export),
      Import file dialog (1050×700, min 500×300, Esc closes it — see delta), Paint hint wrap at
      1600 and 1280, side-panel grip 250→320 + collapse 32 + expand 320 with
      `mst_editor_state.cfg` read after each step, Save Preset (420×179, min 340×178, 400×150 →
      400×179, remembered on reopen, empty name → inline error + disabled OK, Cancel), rail
      collapse 120→40→120, #19 Features ▸ Birds off → History "Turn Birds feature off" → Esc →
      Ctrl+Z → Birds back on.

Not tested: editor restart for N4 (would destroy the state file under test); other window sizes
beyond 1600/1280/1024; HiDPI (scale 1.0); builder/pixel-effects/runtime surfaces (other briefs).

## New issues

- **E1-1** (warning, was R6-1) First `Pixel*` selection still produces a zero-size pixelizer UI
  when the editor is in its first slow frames; 1 s (warm) to >5.7 s (one run, needed a reselect).
- **E1-2** (minor, was R6-4) Settings' first open in a session still inherits the previous tool's
  175 px strip; the tool's own high-water (138) only applies from the second entry.
- **Delta vs r6** (neither a win nor a regression to chase): the Import file dialog now
  **closes on Escape** (r6 recorded that it ignored Esc; Cancel still works).
- **Delta vs r6**: the pixelizer strip now lays out at `731×173` in one sample and `731×175` in
  another (min follows), i.e. the tool's high-water is no longer always the 175 cap; no visual
  defect observed.

## UI feedback (measured)

1. **R6-1 is the only item a user still feels.** Under an idle editor the pixelizer now appears in
   ≤1 s, so the fixer's "not reproduced" is understandable — but the first selection right after
   a project opens (the common path: open project → click the Pixel node) hits the zero window
   every time the editor is at 1–2 fps, and one run never recovered without a second click. A
   "Building pixelizer…" placeholder, or building on `edit()` rather than on a deferred call,
   closes it. r6's `39×0 / 0×35 / 250×0` signature is still exactly reproducible.
2. **The two-column Layout reads much better** (`Grid Type/Merge Mode`, `Ring Radius/Edge Style`,
   `Cell Size/Instanced`, `Wrap Chunks/Extra Collision Layer`): no horizontal scrollbar at 1600,
   and at 1024 nothing overflows either. But it is now 175 px tall in a 175 px strip, so
   `Blend Mode:` is always one notch below the fold. Shaving the row height by 2 px (or the strip
   cap to 177) would remove the only scroll on the page.
3. **Strip heights are still 175/138/107 depending on how you arrive** (E1-2). Resetting the
   high-water on tool entry — as the r6 feedback asked — is still the missing piece; a first open
   should read 138, not Paint's 175.
4. **1024 is now usable inside the pixelizer**: a single `Renderer` dropdown takes 33 of the 99 px
   and the body rows are complete. The Inspector's 87 px overhang is the last thing that makes
   1024 feel unfinished (unchanged, Godot-side per the claim).
5. **Spin sliders now match the HSliders**: `Rotation` steps 0→2→3→2 on the wheel with the value
   label in sync. Its tooltip still says "Alt+wheel", which no longer describes the behaviour
   (plain wheel works) — worth a wording pass.
6. **Harness notes for the next driver:** `control_at` is the only cheap probe (~0.5 s); a
   `ui --root all` dump costs **2.56 s** at ~21 k controls and stalls the deferred build, so it
   cannot be used to time first paint. `select` itself can block 2.2–2.5 s when the build is
   deferred. Popup submenus (Pixel World ▸ Features) open on **click**, not on `move` + `Right`;
   `popups` does not return item rects, so drive OptionButtons with `key Down`+`Enter` and
   popup submenus by clicking the parent row (`Features` at 1320,743) then the item
   (`Birds` at 1128,718). `key Esc` is the spelling (`escape` is rejected).

## Files written

- `qa/findings/r7-editor.md` (this file)
- `qa/screenshots/r7-editor/*.png` (36 captures: `r61-*` first paint, `r62-*` tabs at 1280,
  `r63-*` Settings, `r64-*` strip heights, `r65-*` 1024, `r66-*` spin slider, `reg-*` sweep)
- `qa/screenshots/r7-editor/ui/*.json` (26 dumps), `r61-quiet-timeline.txt`
- `qa/screenshots/r7-editor/editor.log.txt` (full 79-line session log; `editor-boot.log.txt` is
  the same boot half from the first start)
- `qa/scripts/r7-editor/{uiq.py,probe.py,firstpaint.py,early_firstpaint.py,settings_pages.py}`
