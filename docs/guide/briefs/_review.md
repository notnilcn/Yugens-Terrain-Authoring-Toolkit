# Review brief

You review finished pages of the MarchingSquaresTerrain HTML guide (`docs/guide/`). Repository:
`/home/clintonnguyen/g/code_examples/rrr`. Audience: an aspiring artist/3D modeller who has never used
Godot and only knows Roblox Studio's built-in modelling tools.

Read `docs/guide/AUTHORING.md` and `docs/guide/assets/app.js` first. For each page you are given:
1. Open the page source and its notes file (`docs/guide/notes/<page>.md`).
2. **Media:** view every image and every clip poster the page uses. Captions/alt text must match what the
   picture shows. Hotspot % positions must land on the control named (estimate from the image). Flag
   blurry, cut-off, out-of-frame or wrong media.
3. **Accuracy:** spot-check claims against the notes and, where a claim looks doubtful, the addon source
   (`addons/MarchingSquaresTerrain/rust/`). Control names must match the labels in the screenshots.
4. **Beginner fit:** Godot terms explained on first use or linked to page 01/14; Roblox bridges present;
   steps are concrete (which panel, which button).
5. **Links:** every internal href/anchor resolves; prev/next and cross-page links point at real sections.
6. Leftover authoring residue (TODO, "the notes say", capture/harness talk, "recorded on an earlier build").

Fix small problems directly in the page (captions, alt text, hotspot %, wording, broken anchors).
Do not edit `assets/`, notes, media, `addons/` or pages you were not given. Validate each page you edit:
`python3 -c "import html.parser,sys; html.parser.HTMLParser().feed(open(sys.argv[1]).read())" <page>`.
Finish with `REVIEWED:` and, per page: fixes made, and remaining problems you could not fix (with the
media filenames that need re-capturing, if any).
