# DeepSeek agent brief (capture + recipe work on the MarchingSquaresTerrain guide)

Repository: `/home/clintonnguyen/g/code_examples/rrr` — `cd` there first. You work on an HTML guide
for the MarchingSquaresTerrain Godot 4.7 addon, written for an aspiring artist/3D modeller who has
never used Godot and only knows Roblox Studio's built-in modelling tools.

Read before starting:
1. `docs/guide/briefs/_common.md` and `docs/guide/CAPTURE.md` — capture rules (media quality, STOP-on-bug).
2. `qa/README.md` — the harness: `qa/session.sh` (start/stop an isolated editor session on its own
   Xvfb display, rendered on the NVIDIA GPU), `qa/drive.py` (talk to the editor, real X11 input,
   screenshots), `qa/record.sh` (MP4 clips).
3. `docs/guide/recipes/README.md` and the docstring at the top of `qa/recapture.py` — recipe format.
4. The notes file(s) for your page(s): `docs/guide/notes/<page>.md` (scenes, framing, crop rects).

Media quality: footage must be clear, zoomed in and in frame — the thing the caption talks about is
big and obvious, and the control being used is visible. Verify every still by viewing it, and every
clip by viewing its poster plus a mid-frame (extract with `/tmp/opencode/qa/tools/ffmpeg`; not on PATH).
Posters must show the effect, not an empty "before" frame. No test-tool residue in frame
(`/tmp/opencode/...` paths, `qa_tmp` folders, harness log lines).

Hard rules:
- Use ONLY the display/port pairs your task gives you. Never touch other sessions. Stop your sessions
  when done (`qa/session.sh stop <name>`), and delete their dirs (`rm -rf /tmp/opencode/qa/sessions/<name>`).
- Only stop/delete sessions YOU started (track their names). Never loop over `/tmp/opencode/qa/sessions/*`:
  other agents' live sessions are in there.
- Never `pkill -f`/`pgrep -f` with a pattern that appears in your own command line (it kills your shell).
- Never save `res://demo/*.tscn`. Save dialogs in recipes must be cancelled. If a scene gets emptied by a
  stray click: Scene → Reload Saved Scene → Don't Save.
- `Delete` on a builder part opens a confirm dialog whose buttons have no rect in the harness — avoid it.
- Slider drags overshoot under synthetic input; set values with `set-prop` or by typing into fields.
  `set-prop` also reaches into resources: `set-prop palette:quantize_mode 5 --class PixelRenderer`
  (enum popups on sub-resources don't take synthetic keys — use this instead).
- Hold a key (walk a game character): `qa/drive.py key w --hold 2.0`.
- Do not edit `addons/`, `qa/` (except `qa/cmdlog`, which is written for you), `docs/guide/assets/`.
- No cargo builds, no `qa/setup.sh`.
- **Bugs:** if the addon misbehaves (crash, wrong result, broken UI), STOP and finish with `BLOCKED:` +
  exact repro steps + evidence screenshot path. Do not work around or fix addon bugs.
- Never end your turn while something you started still runs in the background (recipe suites,
  recordings): nothing wakes you afterwards. Wait in the foreground (sleep-and-check loop) instead.
- Every `qa/drive.py` call you make is logged to `qa/cmdlog/port-<port>.jsonl`; use it to build recipes.

Recipes: for every media file you produce or are asked to backfill, write an item in
`docs/guide/recipes/<page>.json` (merge into an existing file; keep other items untouched). Test each
item with `python3 qa/recapture.py <recipe> --only <id> --out-dir <scratch dir> --display <test display>
--port <test port>` (the test pair from your task), view the outputs next to the real media, and fix
the recipe until it reproduces the same subject, framing and state. Never let a recipe test write
into `docs/guide/media/` (always pass `--out-dir`). Validate JSON with `python3 -m json.tool`.

HTML edits (only when your task allows): only the `<figure>`/`<img>`/`<video>`/scrubber/compare
elements for files you re-shot — alt text and captions must match the picture. Validate with
`python3 -c "import html.parser,sys; html.parser.HTMLParser().feed(open(sys.argv[1]).read())" <page>`
and check every `src`/`poster` exists.

Append what you did to the page's notes file under `## Re-capture (review pass)` or `## Recipes`.
Finish with `DONE:` (per file / per recipe item: result) or `BLOCKED:`.
