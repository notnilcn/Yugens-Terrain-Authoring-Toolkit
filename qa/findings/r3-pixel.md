# R3 Pixel Look QA — MarchingSquaresTerrain

Status: partial pass — the agent was cancelled by a tool-server restart after checkpointing these
findings; the session `r3-pixel` (:34/8814) is no longer live. Findings below are from this pass.

Session: `r3-pixel`, display `:34`, port `8814`, Godot 4.7.2, llvmpipe, 1600×900; scene
`res://demo/hex_island.tscn`. Project copy:
`/tmp/opencode/qa/sessions/r3-pixel/project`; log
`/tmp/opencode/qa/sessions/r3-pixel/editor.log`. Addon untouched; all pixel edits discarded.

## Dawn preset renders a flat hot-pink sky; undo restores properties but not the preview

- Severity: warning
- Steps: 1) Select `PixelRenderer`. 2) Pixel World panel → **Dawn**.
  3) Wait 3 s, capture. 4) `undo-state` → `Apply Dawn preset` (one action).
  5) `Ctrl+Z`, wait 3 s, capture.
- Expected: Dawn gives a warm sunrise look; undo returns the viewport to the pre-preset blue sky
  (the previous pass reported a salmon tint; this build is worse).
- Actual: the entire sky becomes flat magenta/hot pink (clouds disappear); the terrain loses its
  palette shading. After one Ctrl+Z the Inspector values revert (`Ray Alpha` 0.5 → 1.0, sun
  colour back to yellow) and the undo action is on the redo stack, but the viewport stays pink
  and gains dithered grey/blue speckles over the terrain; 161,288 viewport pixels still differ
  from the pre-Dawn capture and 96,413 from the Dawn capture. Only a scene reload clears it
  (known from the 2026-11-20 pass; not re-reloaded in this pass).
- Evidence: qa/screenshots/r3-pixel/10-dawn-before.png (blue),
  `11-lighting-dawn.png` (flat pink), `12-dawn-undone.png` (still pink + speckle).
- Undo: broken (properties revert, rendered preview does not).

## Disabled Add entries in the Pixel World panel now look disabled (previous finding refuted)

- Severity: none (fixed/refuted)
- Steps: Pixel World panel scrolled to `Sun Shafts` / `Erosion` / `Wild Flowers`.
- Expected: disabled entries are greyed.
- Actual: in this build the whole row text and the leading icon are dimmed (max channel
  brightness 234 vs 627 for the enabled `Light Volume` row); `+ Add` is still clickable
  and looks like the other action buttons. The 2026-11-20 “looks enabled” finding no longer
  reproduces.
- Evidence: qa/screenshots/r3-pixel/08-pixel-world-disabled-rows.png,
  `07-pixel-world-bottom.png`.
- Undo: not applicable.

## Pixel World "+ Add" still closes the pixelizer UI (re-confirmed)

- Severity: ui
- Steps: 1) Select `PixelRenderer`. 2) Pixel World → `Sun Shafts` → **+ Add**.
- Expected: panel stays usable for a second add.
- Actual: `SunShafts` is created and selected; the left section toolbar and bottom strip
  disappear because the selection moved off the renderer. Undo removes it in one action
  (`Add PixelSunShafts`), redo re-adds it. The new node's `sun` property reads `null`
  (Sun slot `Assign...`), as in the previous pass.
- Evidence: qa/screenshots/r3-pixel/09-sunshafts-added.png.
- Undo: works (one action).

## Preview/Animate buttons have no pressed state; only the leading icon changes

- Severity: minor
- Steps: 1) Capture with Pixel Preview on (`02-renderer-section.png`, Preview checkbox On).
  2) Click **Pixel Preview** (viewport switches to full-res, grid/axes show).
  3) Capture. 4) Compare the 124×33 button rect.
- Expected: a toggle button shows on/off (pressed background or tick).
- Actual: the button background/text are identical in both states; only a 29×28 leading icon
  area differs (132 px). With the viewport elsewhere, there is no obvious way to tell whether
  the button is on without looking at the Inspector `Preview` checkbox. Same for **Animate**
  (no visible pressed state; verified while it was on during the two-shot motion test).
- Evidence: qa/screenshots/r3-pixel/02-renderer-section.png (on),
  `03-preview-off.png` (off), `04-preview-restored.png`.
- Undo: not applicable (editor toggle, no undo action).

