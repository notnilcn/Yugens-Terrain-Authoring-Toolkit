# QA methodology — how to run another pass

This is the orchestrator's runbook for a full MarchingSquaresTerrain QA pass: how the pieces fit,
how to split the work across subagents, how to drive the real editor, and how to collect and merge
evidence. The harness command reference is in [`README.md`](README.md); the coverage matrix that
the last pass used is in [`FEATURE-MAP.md`](FEATURE-MAP.md).

The last full run took a day of wall-clock time with nine subagent passes, ~650 screenshots and
~900 MB of evidence. A focused re-run of one or two areas takes 30–60 minutes per area.

## 1. What the pass produces

| Output | Purpose |
|---|---|
| `ISSUES.md` (repo root) | every error/warning/UI issue found, with steps, expected/actual, screenshot + log evidence, and undo status |
| `FEEDBACK.md` (repo root) | UI review: measured layout matrix, resizability policy, priority fixes |
| `qa/findings/<area>.md` | one report per subagent pass: matrix, findings, Covered / Not-tested checklists |
| `qa/screenshots/<area>/` | PNG evidence, plus `ui/` JSON control dumps |
| `qa/SCREENSHOTS.md` | index of the evidence tree |

The addon itself is never edited. All project copies, saves and scene changes happen in disposable
session copies under `MST_QA_STATE`.

## 2. Prerequisites

- Godot **4.7+** on `PATH` (`godot --version`), with the addon's prebuilt `bin/libmarching_squares_terrain.so`
  present (`qa/setup.sh --check` verifies the basics).
- Linux with X11 tools: `Xvfb`, Python 3 with `python-xlib` and `Pillow`.
  `--headless` uses the dummy renderer and **cannot capture the UI**, so a virtual X display is
  the headless-capable setup that still renders and screenshots.
- `rsync` (or `tar`) for the project template.
- The ai-tools checkout for the wave guard, either set `MST_AI_TOOLS` or keep it at
  `../Yugens-Terrain-Authoring-Toolkit/tools/ai-tools` (the wrapper falls back to
  `../rr/tools/ai-tools` and `tools/ai-tools`).

Resource profile from the 2026-11-20 run (16 cores, 14 GB): each Godot editor used ~1.3–1.5 GB
and ~2.6 cores under llvmpipe; each runtime game ~1.6 GB and ~2.7 cores. Three editors at once
was the practical ceiling; the monitor recommended 2–5 depending on the rest of the machine.

## 3. Architecture at a glance

```
repo/qa/
  harness/plugin.cfg, harness.gd   @tool EditorPlugin, local TCP control surface
  drive.py                         harness client + X11/XTest input + screenshots
  session.sh                       isolated editor session (Xvfb, XDG, project copy)
  runtime.sh                       plain game session (no harness), same isolation
  setup.sh                         prerequisites + generated project template
  resource-monitor.sh              ai-tools wave guard wrapper
  README.md, FEATURE-MAP.md        harness manual + coverage matrix
  METHODOLOGY.md                   this file
  findings/, screenshots/          evidence (screenshots are git-ignored)
```

- The harness is enabled **only** in a session copy: `session.sh` appends an `[editor_plugins]`
  entry to the copy's `project.godot`. The repository's `project.godot` stays untouched.
- `drive.py` injects **real X11 input** via XTest, so clicks/keys exercise the same path as a
  person. The harness supplies the control tree and rects; the two coordinate systems match
  because both use window-content coordinates and the window is placed at `(0,0)`.
- Sessions are detached with `setsid`; they survive the shell that launched them.
  Do **not** run `pkill godot` / `killall` — it kills every agent's session.

## 4. One-time setup and pre-flight

```bash
cd /home/clintonnguyen/g/code_examples/rrr

qa/setup.sh --check                 # prerequisites + template status
qa/setup.sh                         # builds $MST_QA_STATE/project_base if missing
qa/setup.sh --refresh               # rebuild after changing repo files (excludes .git,
                                    # rust/target, qa/screenshots, qa/findings)

qa/resource-monitor.sh init         # limits: CPU 85%, RAM 80%, disk 90%, streak 3
qa/resource-monitor.sh start        # detached JSON samples
qa/resource-monitor.sh parallel     # read the wave width before spawning
```

Build the template **before** launching a wave. `session.sh` can build it lazily, but two agents
starting together would race on the same directory.

Smoke-test the whole loop once:

```bash
qa/session.sh start warmup --display :41 --port 8821
source /tmp/opencode/qa/sessions/warmup/env.sh
python3 qa/drive.py ping
python3 qa/drive.py info
python3 qa/drive.py select --class MarchingSquaresTerrain
python3 qa/drive.py capture /tmp/warmup.png
qa/session.sh stop warmup
```

If `ping` fails, read `$MST_QA_STATE/sessions/warmup/editor.log` before spawning agents.

## 5. The run plan

Split by **surface**, not by file. Each area gets one subagent, its own session, its own finding
file and its own screenshot directory. Do not run two agents on the same session/display/port.

| Wave | Area | Session suggestion | Why this order |
|---|---|---|---|
| 1 | Terrain sculpting (Brush, Level, Smooth, Bridge, modifiers, undo) | `sculpt` :31/8811 | core; establishes the undo protocol |
| 1 | Paint, 16 slots, presets, Foliage, grass | `paint` :32/8812 | independent panel/strip |
| 1 | Inspect, Chunks, Settings, Storage, Stamp, Gradient, Select, Tint | `manage` :33/8813 | management + extras |
| 2 | Pixel look (Renderer, Palette, Object, Preview/Animate, Pixel World) | `pixel` :34/8814 | depends on understanding the pixelizer UI |
| 2 | Effects (Sky, Day/Night, Weather, Water, atmosphere, erosion, flowers) | `effects` :35/8815 | long pass; add missing nodes |
| 2 | Builder (PixelBuilderMesh) | `builder` :36/8816 | separate layout family |
| 3 | UI/resizability audit | `uiresize` :37/8817 | needs every surface to exist |
| 3 | Deep undo/redo matrix | `undodeep` :38/8818 | fills wave-1 gaps; can run after wave 2 |
| 3 | Runtime demo/tutorial scenes | `runtime` :44/8824 | no editor; game on its own display |

Practical rules:

- **3 concurrent editor agents** is the ceiling on this machine. The runtime agent is lighter and
  can overlap if resources allow, but watch RAM.
- Wave 1 and 2 can interleave; wave 3's UI audit should wait until the surfaces it measures are
  stable (it can start after wave 1).
- If an agent dies, free its display/port and restart it before spawning a replacement; never
  reuse a live session.
- Give each agent a unique `--display`/`--port` and tell it never to touch another session.
- Use `qa/resource-monitor.sh parallel` before every wave. If it says 2, run 2; the guard exists
  because llvmpipe is CPU-hungry. Three editors is safe when the monitor still shows headroom.

## 6. Subagent brief template

Every brief must be self-contained; the child cannot see the orchestrator conversation. Include:
goal, read-first files, exact session commands, mission bullets, evidence format, rules, timebox,
and the report shape. Copy this and fill the placeholders:

```text
You are QA-testing the MarchingSquaresTerrain Godot addon at <repo>. Test only; never edit addon
code.

READ FIRST:
1. qa/README.md (harness manual; generated state MST_QA_STATE)
2. qa/FEATURE-MAP.md section "Area N — <title>"
3. <relevant docs/ pages>

SESSION (exclusive; never touch other sessions/ports/displays; never run pkill):
  cd <repo>
  qa/setup.sh --check
  qa/session.sh start <name> --display :NN --port 88NN
  source /tmp/opencode/qa/sessions/<name>/env.sh
  python3 qa/drive.py info
  python3 qa/drive.py open-scene res://demo/hex_island.tscn   # wait 8-10 s
Editor log: /tmp/opencode/qa/sessions/<name>/editor.log. Stop with `qa/session.sh stop <name>`.

POPUP RULE: embedded popups do not get injected keyboard focus. Open the control, run
`python3 qa/drive.py popups`, then click x=pos[0]+size[0]/2,
y=pos[1]+8+(index+0.5)*(size[1]-16)/count. Verify visually.

MISSION — <specific surfaces and behaviours>.
- Undo protocol for every editing action: record `terrain` digest and `undo-state`, make exactly
  one change, confirm the digest changed, Ctrl+Z (`key z --ctrl`) → restore, Ctrl+Shift+Z → redo.
  Use the `undo`/`redo` commands only to isolate a failure.
- Check clipping/resizability with `ui --root all`, `capture`, `windows`, `window-resize`.
- Watch the editor log for new ERROR/SCRIPT ERROR lines; ignore the known startup noise.

EVIDENCE:
- qa/screenshots/<area>/NN-description.png (>=N), qa/screenshots/<area>/ui/*.json
- qa/findings/<area>.md in the qa/README.md format + "## Covered" + "## Not tested"
- Do not write ISSUES.md/FEEDBACK.md; the orchestrator merges.

RULES: only write under qa/screenshots/<area>/ and qa/findings/<area>.md. No subagents.
Work <timebox>; if a step blocks >8 min, record it and move on.

REPORT: session stopped?, screenshot count, findings by severity, top 5, untested and why.
```

Brief-writing details that matter:

- Tell the child the **session name/display/port** and that they are exclusive.
- Tell it the **scene** and how to select the target (`select --class`, `select --path`).
- Tell it the **known popup limitation**; otherwise it will burn time on dead keyboard input.
- Tell it **one drag = one undo step**, and that the harness `key` releases focus by default
  (`raw '{"cmd":"key",...,"unfocus":false}'` to test a focused LineEdit).
- Tell it to **wait 1–2 s after page/tab switches**; llvmpipe can catch a transient two-page frame
  under load.
- Tell it to **not use `pkill`** and to stop its session at the end.
- Give a **timebox** and permission to report blocked steps rather than grinding.

## 7. Testing protocols

### Undo / redo (the priority check)

1. `python3 qa/drive.py terrain` → digest + heights.
2. Make exactly one gesture / change.
3. `terrain` again → digest must differ; `undo-state` must name the action.
4. `key z --ctrl` → digest must return to step 1.
5. `key z --ctrl --shift` → digest must return to step 3.
6. Repeat for two gestures + two undos, after switching tools, after clicking the 3D view, after
   selecting another node, after Save, and after closing/reopening a scene.

Terrain digests do not cover tint, scatter counts or texture slot data. Use the matching node
methods (`call get_scatter_count 0`, `prop tex_assign`) and screenshots for those.

### UI / resizability

- `resize W H` is clamped to **1024×600** (Godot's editor minimum). 800×600 is unstable; do not
  force it. Record the clamp.
- At each size: `capture` the whole window, `ui --root all --save` the dump, then check
  `rect`/`min`/`clipped`/`outside_root`. Keep the PNG authoritative if a dump taken immediately
  after a resize still shows the old split.
- Dialogs: `windows` for title/rect/resizable/min, then
  `window-resize W H --title "..."`; check content clipping and whether the action row survives.
- Popups: `popups` for item text and popup pos/size; click coordinates as above. Verify the
  selected value on screen.
- The 3D viewport floor used in the last pass: **480×300** (so the strip must scroll/collapse,
  not just grow).

### Visual checks

- `capture` is a clean in-engine editor screenshot; `shot` includes the Xvfb desktop and the
  window frame. Use `capture` for UI evidence and `shot` for window-size/resizability evidence.
- `--crop x,y,w,h` gives panel close-ups; crop coordinates are window-content pixels.
- For motion (Animate, clouds, water, birds), take two shots ~2 s apart rather than trusting a
  single frame.

### Runtime (game, no editor harness)

```bash
qa/runtime.sh start hex --display :44 --scene res://demo/hex_island.tscn --width 1280 --height 720
source /tmp/opencode/qa/runtime/hex/env.sh      # sets DISPLAY + MST_WINDOW_MATCH
sleep 28                                        # llvmpipe needs ~20-25 s to reach the game
python3 qa/drive.py shot qa/screenshots/runtime/01-hex.png
python3 qa/drive.py key F1
qa/runtime.sh stop hex
```

The game window title is "Marching Squares Terrain (DEBUG)", not "Godot Engine"; the env file sets
`MST_WINDOW_MATCH`. Stop one runtime before starting another on the same display.

## 8. Evidence and findings

- One directory per area: `qa/screenshots/<area>/NN-description.png`; UI dumps in `ui/`.
- One markdown per area: `qa/findings/<area>.md` with the finding format from `README.md` and a
  `## Covered` / `## Not tested` checklist.
- Include the exact action name from `undo-state` and the digest pair where relevant.
- Include log lines for errors/warnings, but strip the known startup noise:
  V-Sync on llvmpipe, debug-adapter/GDScript-LSP ports, missing `.uid` recreation.
- Note harness-induced errors explicitly (e.g. `debug_placement` argument conversion); they are
  not addon defects.

## 9. Merging into ISSUES.md / FEEDBACK.md

1. Read every finding file; don't take a summary on trust. Spot-check the top-severity claims
   yourself (a short session or code inspection).
2. Assign a stable number; keep a summary table row (`# | Severity | Area | One-line issue`).
   Put crashes/blockers at the top, even if found late.
3. Group duplicates: e.g. "strip overflow" has tool-specific manifestations, but one systemic
   issue plus references beats five near-identical rows.
4. Every section: severity, steps, expected, actual, evidence, undo status. Cross-reference
   related numbers.
5. Keep a "Undo / redo — verified working" section and a "Testing limitations" section. Say what
   was **not** tested.
6. FEEDBACK: separate measured facts from opinions. Include the size matrix, an explicit
   resizable/not-resizable policy, and a prioritized fix list. Link ISSUES numbers.
7. Add the area/pass/count table to ISSUES coverage and keep `qa/SCREENSHOTS.md` in sync.

## 10. Failure playbook

| Symptom | Action |
|---|---|
| `ping` refused / port closed | The editor died. `qa/session.sh stop <name>`, read `editor.log` tail; if it starts with a layout OOM (the 1024×600 Level/Smooth/Tint crash), remove the session's `home/` and start again at 1600×900; record it as a finding, don't hide it. |
| Editor dies after a resize | Keep ≥1024×600. The harness clamps now; older sessions may not. Reset the session `home/` (saved editor layout can re-crash on launch). |
| Harness answers but commands time out | The editor may be in a modal/file dialog; `key esc`, `windows` to see it, then retry. |
| Click lands on the wrong control | Re-run `ui --text ...` immediately before clicking; page tabs/strips move. Prefer `click-text` (it re-dumps and auto-scrolls) over raw coordinates. |
| Popup selection does nothing | Embedded popups don't get keyboard focus; click the item with `popups` coordinates. |
| Screenshot is black / stale | Wait 1–2 frames (`sleep 1`) and `capture` again; a resize/perf spike can return the previous frame. |
| Two agents' sessions collide | They are using the same display/port. Stop one, give it a new pair. Never reuse another agent's session. |
| RAM/CPU alert | `qa/resource-monitor.sh status`; stop the newest/largest wave, wait for two clean samples, halve the wave. |

## 11. Lessons from the 2026-11-20 run

- `setsid` is required for session processes; without it, a cleanup of the launching shell can
  take Xvfb and the editor with it.
- Build the template once, before the wave, and exclude `qa/screenshots`/`qa/findings`; otherwise
  every session copy grows to ~1 GB.
- The editor's own minimum is 1024×600; forcing 800×600 destabilises the layout.
- A 1024×600 layout saved after a crash can re-crash the next launch; wipe the session `home/`
  for a clean run.
- Fixed-position page-tab clicks miss because the strip grows upward. Use text/rect lookups.
- Transient two-page frames under llvmpipe are not defects; wait and re-capture.
- Check the top findings yourself: the settings-enum bug, the builder 39 px viewport and the
  1024 crash were all reproduced independently before being reported.
- Screenshots are the bulk of the evidence (~1 GB for nine passes); keep them git-ignored and
  commit the text reports.

## 12. Re-run checklist

```bash
# 0. Environment
cd /home/clintonnguyen/g/code_examples/rrr
qa/setup.sh --check
qa/setup.sh --refresh
qa/resource-monitor.sh init
qa/resource-monitor.sh start
qa/resource-monitor.sh parallel

# 1. Smoke test (then stop it)
qa/session.sh start warmup --display :41 --port 8821
source /tmp/opencode/qa/sessions/warmup/env.sh
python3 qa/drive.py ping && python3 qa/drive.py info
qa/session.sh stop warmup

# 2. Wave 1: spawn <= parallel agents with the brief template (section 6)
#    sculpt / paint / manage

# 3. Read findings, fix blockers, spawn Wave 2 (pixel / effects / builder)
# 4. Spawn Wave 3 (uiresize / undodeep / runtime)

# 5. Merge and verify
python3 qa/drive.py ...   # spot-check the top severity claims in a fresh session

# 6. Cleanup
qa/session.sh stop <each>
qa/runtime.sh stop <each>
qa/resource-monitor.sh stop

# 7. Deliverables
#    ISSUES.md, FEEDBACK.md, qa/findings/*.md, qa/SCREENSHOTS.md
#    (screenshots stay local under qa/screenshots/, git-ignored)
```
