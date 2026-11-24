# Capturing reference media and notes for a guide page

You drive the **real Godot editor** through the QA harness and produce (a) clear screenshots and
short video clips and (b) a detailed notes file that a writer will turn into an HTML tutorial page
for a Roblox-Studio artist who has never used Godot. Read `qa/README.md` first (harness commands),
then the files your brief names.

Repository root: `/home/clintonnguyen/g/code_examples/rrr` (run everything from there).

## Your session

```bash
qa/session.sh start <name> --display :NN --port PPPP --scene res://<scene>.tscn
source /tmp/opencode/qa/sessions/<name>/env.sh
python3 qa/drive.py info
```

Use exactly the display/port your brief gives. Never touch another agent's session. Stop your
session when done (`qa/session.sh stop <name>`). The editor is slow (llvmpipe, ~3–10 fps): wait
1–2 s after clicks, use 1–2 s drags. Work in the session copy of the project
(`/tmp/opencode/qa/sessions/<name>/project`); do not edit the repo's scenes or `project.godot`.

If you **create a scene** worth keeping for readers to follow along (allowed and encouraged when
the brief says so), save it in the session copy under `res://tutorial/guide/`, then copy the `.tscn`
(and any `.res`/`.tres` it needs, with paths unchanged) into the repo's `tutorial/guide/` and list
it in your notes.

Before pressing F5/F6 after changing a scene with harness commands, save it (`key s --ctrl`) — F6 runs the saved file. Check property writes with `drive.py prop` (older notes may say `set-prop` silently ignored arrays; it now converts and goes through undo).

## Media rules — clear, zoomed in, in frame

- Output folder: `docs/guide/media/<page>/` (your brief names `<page>`). Lowercase-hyphen names,
  numbered in walkthrough order: `03-brush-raise.mp4`, `03-brush-raise.png`.
- **Zoom in on what matters.** Frame the 3D camera close to the subject (select it, mouse over the
  3D view, press `f`, then scroll/drag the view so the subject fills most of the viewport).
  Crop UI shots to the panel/strip/menu being explained plus a little context — never ship a full
  1600×900 editor shot for a single slider. A full-editor shot is right only for orientation
  ("where is this panel"), and then add a red highlight box around the area (PIL, 3–4 px, `#ff3b30`).
- **In frame:** before recording, check with a still capture that the subject and the relevant UI
  are fully visible and not covered by popups, tooltips, or the cursor. Re-take anything blurry,
  clipped, half-rendered or showing a loading state. Look at every image you keep (Read it).
- Screenshots: `python3 qa/drive.py capture full.png`, then crop with PIL to the rect from
  `drive.py ui --filter ...` (rects are window coordinates) or `drive.py view3d` (3D viewport rect).
  Crops should usually be ≥ 500 px wide; upscale small UI crops 2× with `Image.NEAREST` so text is
  readable. Save PNG.
- Video: `qa/record.sh start OUT.mp4 --rect X,Y,W,H --fps 12 --max 40` → perform the action with
  `drive.py` (real input: drag/click/key) → `qa/record.sh stop OUT.mp4` →
  `qa/record.sh poster OUT.mp4 OUT.png`. Keep clips 3–15 s, one idea per clip, cropped to the area
  (3D viewport rect, or viewport + the strip being used). Move the mouse slowly/visibly
  (`drag --duration 1.5`). Watch the poster and a mid-frame
  (`/tmp/opencode/qa/tools/ffmpeg -ss 3 -i OUT.mp4 -frames:v 1 mid.png`) to confirm the action is
  visible; re-record if not.
- **Knob series** (for scrubber sliders): for an important numeric knob, capture the same framing
  at 4–7 values (`ps-1.png`, `ps-2.png`, …) — same crop, same camera, only the knob changes.
- **Before/after pairs**: same crop and camera, `-before.png` / `-after.png`.
- **UI tours** (hotspots): one clean crop of a panel; record for each control its centre as a
  percentage of the crop (`(cx - crop_x) / crop_w * 100`) using the `ui` rects.
- Undo anything you changed for a shot if later shots need the original state (Ctrl+Z, verify).
- Budget: aim for roughly 25–60 media files per page; quality over quantity. Delete rejects.
- **Video is required, not optional**: at least 10 MP4 clips per page (more for action-heavy pages
  like sculpting/builder) — every tool, gesture, handle drag, operation and animated effect a
  reader performs should have a short clip, not only a still. Stills are for panels and settings.

## Notes file — `docs/guide/notes/<page>.md`

This is the writer's only source of truth. Be exhaustive and factual (verified in the editor,
cross-checked with the source under `addons/MarchingSquaresTerrain/rust/crates/` where useful):

1. **Feature inventory** — every control in your area: name as shown in the UI, where it is,
   type (button/slider/dropdown/checkbox/menu item/shortcut), range and default, what it does,
   tooltip text, keyboard shortcut/modifiers, and gotchas. Group by panel/tool.
2. **Walkthroughs** — numbered step-by-step tutorials you actually performed (exact clicks/keys),
   each step pointing at the media that shows it.
3. **Media manifest** — table: file, kind (shot/clip/series/before-after/tour), caption, what it
   demonstrates; series with their values; tours with hotspot % positions and labels.
4. **Roblox bridges** — where this resembles/differs from Roblox Studio.
5. **Caveats** — anything confusing, slow, or that needs a specific setup.
6. **Scenes created** — paths, and what is in them.

Write the notes incrementally as you go (so nothing is lost if you are stopped).

## If something is broken — STOP

If a feature errors (`ERROR:`/`SCRIPT ERROR:`/panic in `editor.log`), crashes the editor, does
not do what its UI/tooltip says, or you cannot complete a capture because of a defect:

1. Do **not** work around it or fix code yourself. Do not edit anything under `addons/`.
2. Save what you have: notes so far (mark the point you reached), the defect screenshot and the
   relevant log lines.
3. Stop your session and finish your turn with a report that begins with `BLOCKED:` followed by:
   the defect title, exact reproduction steps (scene, node, clicks/keys), expected vs actual,
   log excerpt, evidence paths, and the list of what you still have left to capture.

The orchestrator will get it fixed and start a new agent to resume from your notes. Cosmetic
nits that do not stop the capture (a tooltip typo, slightly clipped label): note them under
Caveats and continue.

## When finished

Finish with a report beginning `DONE:` — media count, notes path, scenes created, caveats.
