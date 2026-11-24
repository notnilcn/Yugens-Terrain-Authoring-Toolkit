# Re-capture brief (after the review pass)

Reviewers flagged specific media on finished pages as broken, weak or showing test-tool residue.
You re-shoot exactly those files. Follow `docs/guide/briefs/_common.md` and `docs/guide/CAPTURE.md`
(media rules, STOP-on-bug), drive the editor with `qa/README.md`, and read the page's notes file
`docs/guide/notes/<page>.md` (it records the scene, framing and crop rects the originals used).

- **Overwrite the same filenames** (and the clip's poster PNG) in `docs/guide/media/<page>/`, keeping the
  same subject so the page still fits. Footage must be clear, zoomed in and in frame: the thing
  the caption talks about must be big and obvious, and the control being used must be visible.
- Clips: verify the poster and a mid-frame of every clip (`/tmp/opencode/qa/tools/ffmpeg`, it is not on
  PATH). A poster must show the effect, not an empty "before" frame — pick a telling frame with
  `qa/record.sh poster` or extract one with ffmpeg at the right timestamp.
- Keep test-tool residue out of frame: no `/tmp/opencode/...` paths, no `qa_tmp` folders, no harness
  log lines in Output (clear the Output panel with its trash button before shooting if needed).
- You MAY edit the page HTML, but only the `<figure>`/`<img>`/`<video>`/scrubber/compare elements
  that reference files you re-shot: update alt text and captions to match the new picture, and
  re-insert media the review removed when your re-shot version is now good. Validate the page with
  the html.parser one-liner and check every `src` exists. Touch nothing else.
- Append a `## Re-capture (review pass)` section to the page's notes file listing each file, what it
  now shows, and anything you could not fix.
- Stop your session. Finish with `DONE:` (per file: fixed / not fixed + why) or `BLOCKED:`.
