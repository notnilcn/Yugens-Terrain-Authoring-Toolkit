# Writing a guide page

Audience: an aspiring artist / 3D modeller who has **never opened Godot** and has only used
**Roblox Studio's** built-in building tools (Parts, Move/Scale/Rotate, Union/Negate, Terrain Editor,
Explorer, Properties, Toolbox). Write for that person. Assume no programming.

## Voice and depth

- Plain, friendly, direct. Second person ("you"). Short paragraphs. Explain every Godot word the
  first time it appears on a page (node, scene, Inspector, resource, `.tscn`, `res://`, dock, viewport).
- **Extensive**: every button, menu entry, slider, dropdown, checkbox, shortcut and modifier in the
  area gets documented — what it does, where it is, its range/default where known, and when you'd
  use it. Use the capture notes (`docs/guide/notes/<page>.md`) as the source of truth; never invent
  controls or behaviour that the notes do not describe. If the notes flag something as broken or
  unverified, say so honestly (a `.callout.warn`).
- Structure each page as: lede → "What you'll learn" → short guided tutorial (steps) → full
  reference (every control) → recipes / tips → "Check yourself" (2–4 `details.check`) → related pages.
- Constantly bridge from Roblox: add a `.callout.roblox` wherever a Roblox Studio habit applies
  ("In Roblox you'd use the Scale tool; here…").
- Use **lots** of media: every concept gets a picture or clip. Use the interactive components
  below wherever they help: scrubbers for knobs captured at several values, before/after compares,
  hotspot tours of UI panels, tabs for alternatives (e.g. tools), filterable reference cards.

## Files

- Page: `docs/guide/<NN-name>.html` (the filename is fixed in `assets/app.js` `PAGES`).
- Media: `docs/guide/media/<NN-name>/...` (already captured; reference with relative paths
  `media/<NN-name>/file.png`). Do not reference files outside `docs/guide/`.
- Never edit `assets/style.css` / `assets/app.js`. Page-specific CSS goes in a `<style>` in the page,
  using the CSS variables (`var(--accent)` etc.) so dark mode keeps working.

## Page template

```html
<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Sculpting Terrain – MST Guide</title>
<link rel="stylesheet" href="assets/style.css">
<script src="assets/search-index.js" defer></script>
<script src="assets/app.js" defer></script>
</head>
<body>
<main><div class="content">
  <h1>Sculpting terrain</h1>
  <p class="lede">One-sentence promise of the page.</p>
  <div class="meta-row"><span class="pill">⏱ 20 min</span><span class="pill">Beginner</span></div>
  ...
</div></main>
</body>
</html>
```

The shell (top bar, chapter sidebar, search, on-page contents, prev/next) is added by `app.js`.

## Components (copy these)

```html
<!-- Callouts: roblox | tip | warn | try -->
<div class="callout roblox"><span class="label">Coming from Roblox</span><p>…</p></div>
<div class="callout tip"><span class="label">Tip</span><p>…</p></div>
<div class="callout warn"><span class="label">Watch out</span><p>…</p></div>
<div class="callout try"><span class="label">Try it</span><p>…</p></div>

<!-- Keys and UI names -->
<kbd>Ctrl</kbd>+<kbd>Z</kbd>   <span class="ui">Pixel Preview</span>

<!-- Screenshot (click to enlarge) -->
<figure class="shot"><img src="media/03-terrain-sculpting/brush-strip.png" alt="…"><figcaption>…</figcaption></figure>

<!-- Video clip (muted, loops, speed buttons; data-autoplay plays it while on screen) -->
<figure class="clip"><video src="media/03-terrain-sculpting/raise.mp4" poster="media/03-terrain-sculpting/raise.png" data-autoplay></video><figcaption>…</figcaption></figure>

<!-- Before / after drag-compare (both images the same size) -->
<div class="compare" data-before="Off" data-after="On"><img src="a.png" alt=""><img src="b.png" alt=""></div>

<!-- Scrubber: a slider stepping through frames captured at different values of one knob -->
<div class="scrubber" data-label="Pixel Size" data-start="1"
  data-frames='[{"v":"1","src":"media/07-pixelizer/ps-1.png","note":"Crisp, near full resolution"},{"v":"3","src":"media/07-pixelizer/ps-3.png"}]'></div>

<!-- Hotspot tour: positions are % of the image (the notes give them) -->
<div class="hotspots"><img src="media/x/panel.png" alt="…">
  <button class="spot" style="left:12%;top:30%" data-title="Brush" data-text="Raise and lower ground.">1</button>
</div>

<!-- Tabs -->
<div class="tabs"><div class="panel" data-tab="Brush">…</div><div class="panel" data-tab="Level">…</div></div>

<!-- Steps (readers can tick them; progress is remembered) -->
<ol class="steps" data-id="first-hill"><li>…</li><li>…</li></ol>

<!-- Reference cards with a live filter -->
<input class="filter-box" data-target="#brush-ref" placeholder="Filter controls…">
<div id="brush-ref">
  <div class="control"><h4>Size</h4><div class="where">Bottom strip · Brush tool</div>
    <p>…</p><dl><dt>Range</dt><dd>1 – 50</dd><dt>Shortcut</dt><dd><kbd>Shift</kbd>+wheel</dd></dl></div>
</div>

<!-- Self check -->
<details class="check"><summary>How do you undo a stroke?</summary><p><kbd>Ctrl</kbd>+<kbd>Z</kbd> …</p></details>
```

Keep the HTML valid (close every tag; escape `&`, `<`). Every `<img>` needs a meaningful `alt`.
Before finishing, check every `src=` you used exists (`ls`), and open nothing in a browser server.
