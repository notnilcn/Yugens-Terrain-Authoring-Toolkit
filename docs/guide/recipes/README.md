# Capture recipes

One JSON file per guide page (`<page>.json`, e.g. `09-builder-greybox.json`) that replays the
exact editor actions which produce that page's reference media. The format and step kinds are
documented at the top of [`qa/recapture.py`](../../../qa/recapture.py), which runs them:

    python3 qa/recapture.py docs/guide/recipes/09-builder-greybox.json --out-dir /tmp/stage
    python3 qa/recapture.py docs/guide/recipes/09-builder-greybox.json --only 18-intersect
    python3 qa/recapture.py --list docs/guide/recipes

The runner starts its own session (`--display`/`--port`, default `:79`/`8879`), runs `setup`
then each item's `steps`, and writes `recapture-report.json` (ok / failing step per item) into
the output dir. Use `--out-dir` to stage a run for checking before overwriting `media/`.

## Writing a recipe (capture agents)

- Write the item **after** the media is verified good, from the commands that actually produced
  it. Every `qa/drive.py` call is also logged automatically to `qa/cmdlog/port-<port>.jsonl`
  (argv + rc); use it to recover the exact sequence, but drop probes, retries and mistakes.
- Items must be **self-contained from their declared start state**: either `fresh_session: true`,
  or a precondition the previous item leaves (say so in `notes`). Prefer `fresh_session: true`
  for anything that edits the scene, since items should be replayable one at a time with `--only`.
- Prefer stable selectors: `click-text`, `select <node path>`, `set-prop`, `plugin-call` over raw
  pixel `click x y`. When a pixel click is unavoidable, add an `expect` step (e.g. `ui --filter`
  containing the expected label) right after it so drift fails loudly instead of capturing junk.
- Fixed pixel coordinates are the #1 cause of recipes going stale (window size, dock widths and
  dialog positions change). Locate at run time: `ui --filter` to read a rect, then click inside it
  from a small helper in `docs/guide/recipes/_helpers/`; camera via `09-frame.py`; properties via
  `set-prop` (incl. `a:b:prop` into resources). Pin the session size in the recipe's `session`.
- Add `wait` steps where the editor needs to settle (scene open, preview refresh, animation).
- Clip posters: a `poster` step with `at` = the second that shows the effect.
- Fill `check` with what a verifier must see in the output (one sentence), and `notes` with
  caveats (e.g. "slider drag overshoots under XTEST; value stepped with set-prop").
- Don't touch the scene on disk: never save `demo/*.tscn`; recipes that need a saved scene say
  which `tutorial/guide/*.tscn` they start from.
- Test it: run `python3 qa/recapture.py <recipe> --only <ids> --out-dir <scratch dir>` on a free
  display/port and compare the outputs with the media. Fix the recipe until it reproduces them.
- Validate JSON: `python3 -m json.tool <recipe> >/dev/null`.
- If two agents write the same page's recipe, merge into the file: keep other items untouched.
