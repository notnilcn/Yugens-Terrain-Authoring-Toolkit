# Common brief for capture agents

You are one of several agents building an extensive, beginner-friendly HTML guide to the
**MarchingSquaresTerrain** Godot 4.7 addon (`addons/MarchingSquaresTerrain`, Rust GDExtension).
Readers are aspiring artists/3D modellers who have **never used Godot** and have only used
**Roblox Studio's** building tools. Your job is NOT to write HTML; it is to gather clear,
zoomed-in, in-frame reference **screenshots and videos** of the real editor plus an exhaustive
**notes file** that a writer agent turns into the page.

Repository: `/home/clintonnguyen/g/code_examples/rrr` (cd there first).

Read before starting, in this order:
1. `docs/guide/CAPTURE.md` — your protocol (media rules, notes format, the STOP-on-bug rule). Follow it exactly.
2. `qa/README.md` — the harness (`qa/session.sh`, `qa/drive.py`) you drive the editor with.
3. `docs/guide/AUTHORING.md` — skim, so you know which interactive components the writer can use
   (scrubber knob series, before/after compares, hotspot tours, clips) and capture for them.
4. The extra files your area brief lists.

Rules:
- Do not edit anything under `addons/`, `qa/` (except reading), `docs/guide/assets/`, or other agents' media/notes folders.
- Do not run cargo builds or `qa/setup.sh`. The project template already exists.
- Bugs/defects: STOP and report `BLOCKED:` as CAPTURE.md says. Do not work around or fix them.
- The "Roblox UI" toggle (3D view toolbar, next to Pixel World) switches the builder to a Roblox
  Studio–style ribbon. Wherever your area looks different with it on, document the Roblox UI
  version first (the reader's comfort zone) and the other layouts after.
- Write notes incrementally. End your final message with `DONE:` or `BLOCKED:` as CAPTURE.md says.
