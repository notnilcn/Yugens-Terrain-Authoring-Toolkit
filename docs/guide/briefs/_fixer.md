# Fixer brief

A documentation capture agent hit a defect in the MarchingSquaresTerrain addon while driving the
Godot editor. Fix the defect in the addon source. Repository: `/home/clintonnguyen/g/code_examples/rrr`.

- Read `AGENTS.md`-style guidance that applies: invoke the `godot-rust-gdextension` skill before
  changing Rust names, comments, scenes or node creation. The addon source is
  `addons/MarchingSquaresTerrain/rust/` (crates mst, pt_core, pt_modeler, pt_geom).
- Reproduce first with the QA harness (`qa/README.md`): `qa/session.sh start fix-<id> --display :9N --port 889N --scene ...`
  (use the display/port your task gives). Never touch other sessions.
- Builds and other heavy commands go through the queue only:
  `tools/ai-tools/resource-gate/run-resource-intensive-task.sh --name build-addon -- tools/build.sh --no-demo`
  (`cargo check`/`cargo test` likewise, with `--manifest-path addons/MarchingSquaresTerrain/rust/Cargo.toml`).
- After building, refresh the QA template so new sessions get the new library:
  `tools/ai-tools/resource-gate/run-resource-intensive-task.sh --name qa-refresh -- qa/setup.sh --refresh`,
  then start a fresh session and verify the fix live (the exact repro steps), plus undo/redo if the
  feature edits anything. Check `editor.log` for new errors.
- Keep the fix minimal and in the style of the surrounding code. Do not commit. Do not edit
  `docs/guide/` or `qa/` scaffolding.
- Stop your session. Finish with `FIXED:` (root cause, files changed, how verified) or
  `NOT FIXED:` (what you found, why).
