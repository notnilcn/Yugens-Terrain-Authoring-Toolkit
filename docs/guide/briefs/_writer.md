# Writer brief

You turn captured notes + media into one or more finished HTML pages of the MarchingSquaresTerrain
guide. Repository: `/home/clintonnguyen/g/code_examples/rrr`.

Read, in order: `docs/guide/AUTHORING.md` (audience, voice, page template, components — follow it
exactly), `docs/guide/assets/app.js` (the PAGES list: your page filename and its neighbours), the
notes file(s) your task names under `docs/guide/notes/`, and look at the media in
`docs/guide/media/<page>/` (view images to write accurate captions/alt text; check clip posters).

Write `docs/guide/<page>.html`. Requirements:
- Extensive: every control and feature in the notes is documented (reference cards with a filter
  box for long control lists), plus a guided tutorial, recipes, Roblox bridges, self-check.
- Use the media heavily and appropriately: clips for actions, scrubbers for knob series,
  compares for before/after pairs, hotspot tours for panels (use the notes' % positions),
  galleries for sets. Every media file in the folder that is good should appear somewhere;
  skip any that are blurry/broken (note which in your final message).
- Link to other pages by their filenames in app.js where the notes say "see page NN".
- Only reference files that exist (`ls` check every `src`). No external URLs for scripts/fonts.
- Do not edit `assets/`, notes, media, or other pages.
- Validate: `python3 -c "import html.parser,sys; html.parser.HTMLParser().feed(open(sys.argv[1]).read())" docs/guide/<page>.html` and grep that every referenced media path exists.

Finish with `DONE:` + page path, section list, media used/skipped counts, and anything in the
notes you could not document (or `BLOCKED:` if the notes are unusable).
