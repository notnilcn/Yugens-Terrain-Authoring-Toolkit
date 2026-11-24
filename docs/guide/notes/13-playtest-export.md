# Capture notes — page `13-playtest-export`

Area C12. Session `docs-play`, display `:82`, port `8882`. Scene used for runtime:
`res://demo/hex_island.tscn` (the richest sandbox; has the Knight demo player, a
`PixelDiagnostics` node, grass, water, birds). Editor runs under llvmpipe (software GL),
so frame rate is low (2–10 fps) — that is a capture-rig artifact, NOT the real game speed.

This page teaches a Roblox-Studio artist how to **playtest** the world they built (run it,
walk around with the demo character, read the overlays), how the **pixel look** renders in
the real game, how the game **window scales**, and how to **get the world out**: export the
terrain as a GLB model (for Blender/Roblox) and export a standalone playable build.

---

## 1. Feature inventory

### 1a. The Run bar (top-right of the editor) — how you launch the game
Rect of the whole bar ≈ `[1392,4,200,36]`. Buttons left→right (verified tooltips):

| Button | Pos (win coords) | Shortcut | Tooltip (verbatim) | Notes |
|---|---|---|---|---|
| Run Project (▶) | 1396,8 | **F5** | "Run the project's main scene." | Runs the scene set as *Main Scene* (Project Settings), NOT necessarily the one you're editing. |
| Pause (⏸) | 1428,8 | **F7** | "Pause the running project's execution for debugging." | Only active while a game is running. |
| Stop (⏹) | 1460,8 | **F8** | "Stop the currently running project." | Only active while running. |
| Remote Deploy (▾) | 1492,8 | — | "Remote Deploy" | One-click deploy menu to devices; not used here. |
| Play Scene (▶ small) | 1496,8 | **F6** | "Play the currently edited scene." | Runs the scene in the active tab (e.g. your own world) without changing the Main Scene. |
| Play Custom Scene | 1528,8 | **Ctrl+Shift+F5** | "Play a custom scene." | Opens a picker to run any scene file. |
| Movie Maker (▾) | 1560,8 | — | "Enable Movie Maker mode. The project will run at stable FPS and the visual and audio output is recorded to a video file." | For recording a movie, not normal play. |

When a game is running, the ▶ dims and ⏸/⏹ light up (see `07-run-bar-running.png`).

### 1b. Main Scene setting (which scene F5 runs)
`Project` menu → `Project Settings…` → `General` tab → `Application` → `Run` → **Main Scene**.
In this project it is `res://demo/hex_island.tscn`. (See capture in §Project-Settings below.)
F5 always runs the Main Scene; F6 runs whatever tab you have open, which is how a reader
tests THEIR own world without changing the project's Main Scene.

### 1c. The Game tab (embedded play) — top center main-view switcher
Main-view tabs: `2D | 3D | Script | Game | Asset Store`. The **Game** tab is where Godot can
embed the running game inside the editor. Its toolbar (`03-game-tab-toolbar.png`) controls the
embedded game: pause at SceneTree level, next-frame step, speed (`1.0×`) + reset, **Input**
(allow game input), **2D**/**3D** (suspend input and pick/inspect live nodes / move the live
camera), selectable-node tools, Toggle Selection Visibility, Mute game audio, Override in-game
camera, and (far right) Game Window Options.
> **Caveat / session limitation:** the QA harness launches the editor with `--single-window`,
> so the Game tab shows **"Game embedding not available in single window mode."**
> (`04-game-embed-message.png`). Embedding requires the editor's multi-window mode
> (Editor Settings → Interface → Editor → Single Window Mode OFF, default). This is a
> capture-rig limitation, not a defect — the message is accurate. In a normal editor the game
> renders inside this tab instead of a separate OS window.

### 1d. Demo character controls (shown on screen by the hint line)
The on-screen hint reads exactly:
`WASD move · Shift walk · Space jump · drag / arrows orbit · wheel zoom · F3 tuner · F1 help`
(`18-hint-text.png`). Verified against the input map in `project.godot` and `mst_demo` source:

| Control | Keys / input | Action | Source |
|---|---|---|---|
| Move | **W A S D** (physical) | Walk/run; character turns to face the move direction | `player_forward/back/left/right` = W/S/A/D |
| Walk (slow) | **Shift** held | Slower "walk" instead of run | `player_walk` = Shift |
| Jump | **Space** | Hop (~1 m); the follow-camera keeps the character centered so watch the background bob | `player_jump` = Space; `jump_velocity 4.5` |
| Orbit camera | **mouse drag** (L/M/R button) or **arrow keys** | Spin the camera around the character (yaw + pitch) | `camera_orbit_left/right/up/down` = arrows; drag handled in `player.rs` |
| Zoom | **mouse wheel** | Dolly the camera in/out (clamped) | `zoom_factor 1.15` |
| Help | **F1** | Re-show the hint line (auto-fades after ~10 s) | `MstDemoHint`, `toggle_key F1` |
| Diagnostics | **F3** | Toggle the Pixel tuner / diagnostics overlay | `PixelDiagnostics`, `toggle_key F3` |

- **Esc** is NOT handled by the demo player or hint; the demo does not capture the mouse, so
  there is nothing to release. In a windowed debug build the game is closed with the Stop
  button / F8 in the editor, or the window's close box. (The brief listed "Esc"; it has no
  demo-specific effect — noted here so the writer doesn't claim one.)
- The character model is interchangeable: `demo/player.tscn` is the **Knight**, `demo/fox_player.tscn`
  is a **Fox** (same `MstDemoPlayer` script + controls, different `Visual`). hex_island ships
  the Knight.

### 1e. F3 "Pixel tuner" / diagnostics overlay (only where a `PixelDiagnostics` node exists)
Press **F3** in the running game to toggle it (`20-f3-tuner-full.png`). It is a developer tool
provided by a `PixelDiagnostics` node in the scene (hex_island has one; a bare scene does not).
- **Status readout** (top-left, `21-f3-status.png`): `fps | draws | prims | F3 hides` /
  `pixel <N>px | low-res WxH | texel <m> | cam persp | outlines on | ids 0-255 | palette on |
  rays on` / `weather <type> (wet, snow) | time HH:MM | features used/total`.
- **Object inspector** (left): "Select Object" then click a mesh to inspect/tune how it is
  pixelized; "Clear".
- **Pixel tuner panel** (right, `22-f3-tuner-panel.png`): `Rescan`, a **Debug view** dropdown
  (default **Final**), tabs **Effects / Render / Scene**. The Effects tab has
  **Features (this session)** checkboxes — Clouds, God Rays, Outlines, Outline Ids, Water,
  Weather, Birds, Fireflies, Smoke, Fire, Particle Fields, Light Volumes, Sun Shafts, Grass —
  with **All on / Defaults**, then live per-effect sections (e.g. **PixelWeather** with
  Clear/Cloudy/Foggy/Rain/Snow/Storm, Instant, Snow overlay, Lightning, Transition Seconds,
  Ground Seconds, Strength). Changes are live-only (not saved) — a safe playground.

(Media manifest, walkthroughs, GLB/export, window scaling, Roblox bridges and caveats are
appended below as I capture them.)

---

## 2. Walkthroughs (performed)

### W1 — Run your world from the editor (F5 / F6 / F8)
1. Top-right **Run bar**: press **F5** (▶ "Run the project's main scene") — Godot launches the
   project's Main Scene in a game window. In single-window/QA mode the game opens as a separate OS
   window; in a normal multi-window editor it embeds in the **Game** tab. See `05-game-running-hint.png`
   (the running game with the hint line) and `06-editor-while-running.png` (editor + game window).
2. While running, ▶ dims and ⏸ (F7) / ⏹ (F8) light up (`07-run-bar-running.png`). The Scene dock
   also gains a **Remote / Local** switch to inspect the live tree.
3. Press **F8** (⏹) to stop. To run just the scene you're editing (not the Main Scene), use **F6**
   ("Play the currently edited scene") — this is how a reader tests their own `first_world` without
   changing the project Main Scene.

### W2 — Walk around with the demo character (clips 10–17, 19, 23)
Performed in `demo/hex_island.tscn` (which ships the Knight player + a `PixelDiagnostics` node):
- **Move**: hold **W/A/S/D** — the Knight runs and turns to face the direction (`10-move-forward.mp4`,
  `11-move-turn.mp4`).
- **Walk**: hold **Shift** while moving for a slower walk (`12-shift-walk.mp4`).
- **Jump**: tap **Space** (`13-jump.mp4`) — a modest hop; the follow-camera keeps the character
  centred, so watch the background bob.
- **Orbit camera**: drag with the mouse (`14-camera-orbit-drag.mp4`) or hold the **arrow keys**
  (`15-camera-orbit-keys.mp4`).
- **Zoom**: mouse **wheel** in/out (`16-camera-zoom.mp4`).
- **F1**: re-show the on-screen hint line (`17-f1-hint.mp4`, `18-hint-text.png`).
- **F3**: toggle the Pixel tuner / diagnostics overlay (`19-f3-diagnostics.mp4`, `20-f3-tuner-full.png`).
- The world is alive at runtime: clouds drift, grass sways, water foams, birds fly (`23-pixel-world-alive.mp4`).

### W3 — Export the terrain as a GLB model (Storage page)
1. Select the terrain, open the **Terrain Settings Tool** (left toolbar, shortcut **9**), click the
   **Storage** page tab (`34-storage-page-strip.png`, `35-settings-pages-tabs.png`).
2. The row **Export GLB: [Export]** writes the terrain to the terrain's **Export Path** property
   (default `res://terrain_export.glb`) as a single baked mesh with one texture atlas (slot textures,
   blending, walls baked in). Clicking it bakes chunk-by-chunk, then writes the file — the editor
   pauses for several seconds (`36-glb-export-progress.png` shows the inline "Baking… / Cancel" bar).
3. **Verified export**: exporting the hex_island terrain produced a valid **glTF 2.0** GLB
   (generator "Godot Engine v4.7.2.stable"): **1 mesh ("Terrain") under node
   "MarchingSquaresTerrainExport", 1 material, 1 baked-atlas texture, 1 primitive** (~39 MB, the atlas
   is high-res). This is the file you import into **Blender** (File → Import → glTF 2.0) or **Roblox
   Studio** (3D Importer → pick the .glb; it comes in as a MeshPart with the baked texture).
4. **Bake Meshes: [Bake]** (same row) bakes the live terrain into static MeshInstance3D geometry in
   the scene (visual meshes). **Import MST: [Import…]** re-imports a saved `.mst`/data file.

> **Crash-risk guard (from source `glb_export_dialogs.rs`):** if the Export Path is **inside the
> project folder** AND the renderer is **Compatibility** AND **Use Multiple Threads** import is on
> AND the atlas has **≥2 textures**, the editor pops a confirmation ("…the Godot editor can crash
> while importing a GLB that holds two or more textures… Export Anyway / Cancel") because Godot would
> try to re-import the GLB and can crash. Avoid it by exporting to a folder **outside** the project
> (or one containing a `.gdignore`), or turning off multi-threaded import, or using Forward+/Mobile.
> (In this capture the guard did not trigger for hex_island's atlas, so the export proceeded directly;
> I exported to an out-of-project path to prove it works safely.)

### W4 — Export a standalone playable build (Project → Export…)
1. `Project` menu (`30-project-menu.png`) → **Export…** opens the Export dialog (`37-export-dialog.png`).
2. `export_presets.cfg` defines **one** preset: **Linux (Runnable)**, export path
   `build/linux/marching-squares-terrain.x86_64`, architecture x86_64, embed PCK off, console wrapper
   "Debug Only". (No Windows/macOS preset is defined — the reader would add one with **Add…**.)
3. **Export templates are NOT installed** in this environment: the dialog shows red
   "No export template found at the expected path: …/export_templates/4.7.2.stable/linux_debug.x86_64"
   (and linux_release), so **Export Project…** is greyed out (`38-export-templates-missing.png`). Only
   **Export PCK/ZIP…** is enabled.
4. The **Manage Export Templates** link opens the Export Template Manager (`39-export-template-manager.png`):
   "No templates installed", offline. Download+Install All Templates would fetch them (NOT done here per
   brief). Once templates are installed, **Export Project…** builds the runnable binary.

### W5 — Add the demo player to your own first scene (WORKS — the player must be moved above the ground)
> **Resolved (2026-11-24):** there is **no addon bug**. The earlier "falls through" report was a
> reader-trap, not a defect: a freshly instanced `PlayerRig` sits at the World **origin (0, 0, 0)**,
> which for `first_world` is *inside/under* the hill, so the character body starts embedded in the
> terrain and immediately drops into the void. Runtime terrain collision works fine — place the
> player **above** the ground and it lands and walks normally (proven below and in `54-player-landing.png`).
> The earlier failure also happened because the harness `set-prop` used to ignore array writes, so the
> position never actually reached the saved scene; `set-prop` now converts arrays → Vector3 and goes
> through undo, and F5/F6 save the scene first.

Performed end-to-end in `res://tutorial/guide/first_world.tscn` (saved as `first_world_player.tscn`):

1. Open `res://tutorial/guide/first_world.tscn`. In the **Scene** dock select the **World** root.
2. Click **Instantiate Child Scene** (the chain-link icon at the top of the Scene dock, next to the
   `+`; tooltip *"Instantiate a scene file as a Node…"*). The **Select Scene** picker opens
   (`52-instantiate-player.mp4`, `50-select-scene-player.png`).
3. Pick **`res://demo/player.tscn`** (top of the list / type "player"). A **PlayerRig** node is added
   under World (`51-scene-tree-player-added.png`). **It spawns at the origin (0, 0, 0).**
4. **Move it above the terrain — this is the step readers must not skip.** With **PlayerRig** selected,
   in the **Inspector** expand **Transform → Position** and type a spot above the hill, e.g.
   **x 5, y 8, z 4** (`53a-position-before.png` → `53b-position-after.png`, side-by-side
   `53-set-position-compare.png`). (The green Y gizmo arrow in the 3D view does the same thing.)
   The hill peaks at ~5.3 world units near (5, 4), so y = 8 drops the character a short, safe distance
   onto solid ground.
5. **Save** the scene (**Ctrl+S**; here saved as a copy, `first_world_player.tscn`, so the reader's
   original `first_world.tscn` is untouched). F6 runs the *saved* file, so saving first is required.
6. Press **F6** ("Play the currently edited scene"). The Knight **lands on the terrain** in the forest
   clearing and you can walk around:
   - **Land**: `54-player-landing.png` (the character standing in the clearing, trees all around).
   - **Walk** (W/A/S/D) through the trees: `55-walk-wasd.mp4`.
   - **Orbit** the camera (mouse drag or arrows — pitches from eye-level to top-down): `56-camera-orbit.mp4`.
   - **Zoom** (mouse wheel) in/out: `57-camera-zoom.mp4`.
   - Wide look at the whole wooded hill: `58-world-overview.png`.
7. **If you forget step 4** (leave PlayerRig at 0,0,0): the character starts buried in the hill and
   falls straight through on the first frame — the follow-camera descends with it, so you see **only
   sky and sea, no ground** (`59-forget-to-move.mp4`, `59-forget-to-move.png`; side-by-side with the
   correct result in `59c-move-or-not-compare.png`). This is the single most common first-timer mistake;
   the fix is always "move the player up."

**Note — F3 does nothing in `first_world`:** the F3 Pixel-tuner overlay needs a `PixelDiagnostics`
node in the scene. `hex_island` has one; the reader's bare `first_world` does not, so F3 is inert there
until such a node is added (verified in-game). F1 (re-show the hint line) and all movement/camera
controls work identically to hex_island.

### W5b — The Fox character (optional, same steps)
`res://demo/fox_player.tscn` is the exact same `MstDemoPlayer` rig with a Fox `Visual` instead of the
Knight. Instance it the same way (or swap the instanced scene) and move it above the ground — it lands
and walks identically (`64-fox-variant.png`, `65-fox-walk.mp4`). Use whichever character you prefer;
the controls and the "move it above the ground" rule are the same.

### W5c — Editor Pixel Preview vs the raw 3D view (on `first_world`)
In the 3D-view toolbar, the **Pixel Preview** toggle switches the editor viewport between the raw 3D
scene and the game's pixel renderer:
- **OFF** — raw high-res 3D with the editor grid, axis lines and gizmos (`61-pixel-preview-off.png`).
- **ON** — the same view rendered through the `PixelRenderer`, i.e. what the game actually shows
  (`62-pixel-preview-on.png`). Side-by-side: `63-pixel-preview-compare.png`.
This is the editor-vs-runtime preview: Pixel Preview ON in the editor matches the F6 runtime look
(minus grid/gizmos).

---

## 3. Media manifest

| File | Kind | Shows |
|---|---|---|
| 01-run-bar.png | shot (highlight) | Run bar: Play(F5)/Pause(F7)/Stop(F8)/Remote/Play-Scene(F6)/Custom/Movie |
| 07-run-bar-running.png | shot | Run bar while a game runs (Pause/Stop active) |
| 02-main-tabs.png | shot | Main-view tabs 2D/3D/Script/**Game**/Asset Store |
| 03-game-tab-toolbar.png | shot | Game tab toolbar (pause/step/speed/Input/2D/3D/mute/camera-override) |
| 04-game-embed-message.png | shot (highlight) | "Game embedding not available in single window mode" |
| 05-game-running-hint.png | shot | Running game (Knight + hint line) — pixel look at runtime |
| 06-editor-while-running.png | shot (highlight) | F5 launches a separate game window over the editor |
| 10-move-forward.mp4/.png | clip | Walk forward (W), character runs, camera follows |
| 11-move-turn.mp4/.png | clip | A then D — character turns to face move direction |
| 12-shift-walk.mp4/.png | clip | Run then Shift-walk (slower) |
| 13-jump.mp4/.png | clip | Running jumps (Space) |
| 14-camera-orbit-drag.mp4/.png | clip | Orbit camera by mouse drag |
| 15-camera-orbit-keys.mp4/.png | clip | Orbit camera with arrow keys |
| 16-camera-zoom.mp4/.png | clip | Wheel zoom out then in |
| 17-f1-hint.mp4/.png | clip | F1 re-shows the hint line |
| 18-hint-text.png | shot (2×) | The hint text, readable |
| 19-f3-diagnostics.mp4/.png | clip | F3 toggles the Pixel tuner overlay |
| 20-f3-tuner-full.png | shot | Full F3 diagnostics overlay |
| 21-f3-status.png | shot (2×) | F3 status readout (fps/pixel/low-res/texel/weather/time/features) |
| 22-f3-tuner-panel.png | shot | F3 right panel: Debug view, Effects/Render/Scene, feature toggles, weather |
| 23-pixel-world-alive.mp4/.png | clip | Ambient runtime pixel world (sea foam, grass, clouds, birds) |
| 30-project-menu.png | shot | Project menu (Project Settings…, Export…) |
| 31-main-scene-setting.png | shot (highlight) | Application → Run → **Main Scene** = res://demo/hex_island.tscn |
| 32-window-stretch-settings.png | shot | Display → Window → Size (Viewport 1280×720, Windowed, Resizable) |
| 33-stretch-settings.png | shot | Stretch: Mode **disabled**, Aspect **keep**, Scale 1.0 |
| 34-storage-page-strip.png | shot (highlight) | Storage page: Storage Mode, Bake Meshes, Import MST, **Export GLB** |
| 35-settings-pages-tabs.png | shot (2×) | Settings page tabs: Grid/New Chunks/Grass/Display/LOD & Nature/Storage |
| 36-glb-export-progress.png | shot | Inline GLB bake progress + Cancel |
| 37-export-dialog.png | shot | Project Export dialog, Linux preset, options |
| 38-export-templates-missing.png | shot (highlight) | Templates missing; Export Project greyed; Manage link |
| 39-export-template-manager.png | shot | Export Template Manager (none installed, offline) |
| 40-window-1280x720.png | shot | Game at 1280×720 |
| 41-window-640x360.png | shot | Game at 640×360 (coarser pixels, same camera) |
| 42-window-1920x1080.png | shot | Game at 1920×1080 (finer pixels, same camera) |
| 43-pixel-scale-compare.png | compare panel | Same region at 640/1280/1920 — pixels get finer as resolution rises |
| 44-window-resize.mp4/.png | clip | Live window resize — pixel grid re-derives |
| 50-select-scene-player.png | shot (highlight) | "Select Scene" picker showing player.tscn & fox_player.tscn |
| 51-scene-tree-player-added.png | shot (highlight) | PlayerRig added under World in first_world |
| 52-instantiate-player.mp4/.png | clip | Select World → Instantiate Child Scene → pick player.tscn → PlayerRig added |
| 53a-position-before.png | shot (2×) | Inspector Transform → Position **0, 0, 0** (inside the hill) |
| 53b-position-after.png | shot (2×) | Inspector Transform → Position **5, 8, 4** (above the ground) |
| 53-set-position-compare.png | before-after | Inspector Position 0,0,0 vs 5,8,4, labelled |
| 54-player-landing.png | shot | Knight landed on the terrain in the forest clearing (runtime) |
| 55-walk-wasd.mp4/.png | clip | Walking W/A/S/D through the trees, camera follows |
| 56-camera-orbit.mp4/.png | clip | Orbit camera by mouse drag (eye-level → top-down) |
| 57-camera-zoom.mp4/.png | clip | Mouse-wheel zoom in then out |
| 58-world-overview.png | shot | Wide runtime view of the whole wooded hill/island |
| 59-forget-to-move.mp4/.png | clip | Reader-trap: PlayerRig left at origin → only sky + sea (fell through) |
| 59c-move-or-not-compare.png | before-after | Moved above ground (lands) vs left at origin (falls), labelled |
| 61-pixel-preview-off.png | shot | Editor 3D view, Pixel Preview OFF (raw 3D + grid/gizmos) |
| 62-pixel-preview-on.png | shot | Editor 3D view, Pixel Preview ON (pixel renderer = game look) |
| 63-pixel-preview-compare.png | before-after | Pixel Preview OFF vs ON on first_world, labelled |
| 64-fox-variant.png | shot | Fox character landed on the terrain (runtime) |
| 65-fox-walk.mp4/.png | clip | Fox walking through the trees |
| 60-editor-take-screenshot.png | shot (highlight) | Editor menu → Take Screenshot / Open Editor Data Folder |

---

## 4. Pixel look: editor preview vs runtime; window scaling
- **Editor vs runtime:** the editor's 3D view shows the pixel look when **Pixel Preview** (3D-view
  toolbar) is on — see the editor orientation (e.g. `storage-page.png`, `05/06` for the game). The
  runtime render is the same pixel pipeline but clean (no grid/gizmos). Compare `06-editor-while-running.png`
  (editor + game) with `05-game-running-hint.png` (clean runtime).
- **Window scaling (very important point for the reader):** the project's **Stretch Mode is
  `disabled`** (`33-stretch-settings.png`). So the game does NOT scale a fixed low-res image up to the
  window. Instead the `PixelRenderer` derives its internal low-res buffer from the *current* viewport
  size, so a **bigger window shows more, finer pixels** and a **smaller window shows fewer, chunkier
  pixels** — the camera framing stays identical. Proven at 640×360 / 1280×720 / 1920×1080 with the same
  camera (`40/41/42`, side-by-side `43-pixel-scale-compare.png`) and live (`44-window-resize.mp4`).
  The game window is **Resizable = On** (`32`).

---

## 5. Roblox bridges (for the writer)
- **Running:** Roblox Studio has **Play** (F5, play as your avatar at spawn), **Play Here** (play at
  the camera), and **Run** (F8, simulate with no character). Godot's equivalent is **F5 = Run the Main
  Scene** and **F6 = Play the edited scene**; there is no avatar-vs-camera distinction — the scene
  decides what spawns (here, the demo player you add). Godot's **F8 = Stop** (Roblox uses Shift+F5/the
  Stop button). Godot's **Game tab** is like Roblox's split Edit/Play view — you can select and inspect
  live nodes while the game runs (Input/2D/3D toggles).
- **Camera & movement:** the demo uses a follow/orbit camera (drag or arrows to orbit, wheel to zoom)
  like a classic over-the-shoulder Roblox game; WASD+Space+Shift are the Roblox-standard defaults.
- **Exporting:** Roblox **Publishes** your place to the Roblox cloud; Godot **Exports** a standalone
  app (Project → Export…) that runs outside any launcher. There is no cloud host — the GLB/terrain and
  the exported binary are files you own. **Export GLB** is the bridge back INTO Roblox: bake the terrain
  to a .glb and import it in Studio's 3D Importer as a MeshPart (with the baked texture atlas).
- **Screenshots:** Roblox has a screenshot button; Godot's editor has **Editor → Take Screenshot**
  (saved to the Editor Data Folder, `60-editor-take-screenshot.png`). The demo game has **no** built-in
  screenshot key (the hint lists none) — use your OS screenshot tool, or Godot's Movie Maker mode
  (Run bar) to record video.

---

## 6. Caveats / setup notes
- **llvmpipe (software GL):** the whole QA rig renders in software, so the F3 overlay shows ~2 fps and
  scenes take 10–40 s to load. This is NOT the real game speed; do not quote the fps in the page.
- **Single-window mode:** the harness runs the editor with `--single-window`, so the **Game tab cannot
  embed** ("Game embedding not available in single window mode"). A normal editor (multi-window, the
  default) embeds the game in the Game tab. Documented as a session limitation, not a bug.
- **F3 overlay needs a `PixelDiagnostics` node:** hex_island has one, so F3 works there. A bare scene
  (like the reader's first_world) has none, so F3 does nothing until a `PixelDiagnostics` node is added.
  Worth telling the reader.
- **Esc** does nothing in the demo (no mouse capture to release); stop a windowed debug build from the
  editor (Stop/F8) or close its window.
- **GLB export crash guard** — see W3; export outside the project to be safe.
- **Export templates** are not installed here; a real build export needs them (Manage Export Templates),
  and per the brief must go through the resource-gate. Not performed.

---

## 7. Scenes created / modified
- **`res://tutorial/guide/first_world_player.tscn`** — created and copied into the repo
  (`tutorial/guide/first_world_player.tscn`). It is exactly `first_world.tscn` **plus** one
  instanced `res://demo/player.tscn` (node **PlayerRig**) at **Position (5, 8, 4)**. Everything else
  (terrain, Camera, Sun, WorldEnvironment, DayNight, Water, Birds, PixelRenderer) is unchanged from
  `first_world.tscn`; DayNight is at its defaults. This is the follow-along scene for W5 — run it with
  **F6** and the character lands and walks. (The editor reassigns internal resource IDs on Save As, so
  the ext_resource `id=` suffixes differ cosmetically from `first_world.tscn`; all references are
  self-consistent.)
- **`res://tutorial/guide/first_world.tscn`** — **unchanged** (the reader's own page-02 scene is never
  modified; the player goes into the separate `_player` copy).
- Throwaway session-only scenes used to capture the reader-trap and the Fox (NOT copied to the repo):
  `first_world_trap.tscn` (PlayerRig at 0,0,0) and `first_world_fox.tscn` (fox_player at 5,8,4).

---

## 8. Resolved: the "player falls through" was a reader-trap, not a bug
**Status: NOT a defect. Runtime terrain collision works.** The earlier BLOCKER has been reproduced,
explained, and fully captured. There is nothing to fix in the addon.

**What was actually happening**
- A freshly instanced `PlayerRig` has no transform, so it sits at the **World origin (0, 0, 0)**. For
  `first_world`, origin is *below/inside* the hill (the surface near origin is ~3 units up). The
  `CharacterBody3D` therefore **starts embedded in the terrain**; `move_and_slide` under gravity cannot
  resolve a body that begins fully inside the collision mesh, so it drops straight through on frame 1.
  The follow-camera descends with it → you see only sky + sea. That is the entire "falls through."
- The earlier investigation also could not make a fix "stick" because the harness `set-prop` used to
  **silently ignore array writes** (so the position never reached the saved scene, and F6 ran a scene
  that still had the player at origin). `set-prop` now converts `[x,y,z]` → `Vector3` and goes through
  undo, and the scene is saved before F6, so the position actually applies.

**Proof it works (this session)**
- Instanced `player.tscn`, set **PlayerRig.position = (5, 8, 4)**, saved, F6 → the Knight **lands on the
  terrain** and walks/orbits/zooms normally: `54-player-landing.png`, `55-walk-wasd.mp4`,
  `56-camera-orbit.mp4`, `57-camera-zoom.mp4`, `58-world-overview.png`.
- Left at **(0, 0, 0)** → falls through, sky+sea only: `59-forget-to-move.mp4/.png`. Side-by-side
  comparison: `59c-move-or-not-compare.png`.
- Same result with the Fox (`fox_player.tscn`): lands and walks (`64-fox-variant.png`, `65-fox-walk.mp4`).
- The old collision facts still hold and are consistent with "works": terrain `impact_active = true`,
  player `collision_mask = 5` (1 & 5 = 1), `get_chunk_collision_faces([0,0])` → 4096 faces. No
  `lod_focus` is needed for collision. No errors in `editor.log`.

**Writer takeaway (put this in the page as the key warning):** after you *Instantiate Child Scene →
player.tscn*, the character appears at the world origin, usually **inside your terrain**. You **must**
move it up (Inspector → Transform → Position, e.g. y = 8, above your highest ground) **before** you
press F6, or it will fall through and you will only see sky and water. This is the #1 first-timer
mistake; it is not a bug. (See W5 step 4 and `53-set-position-compare.png` / `59c-move-or-not-compare.png`.)

The misleading `_defect/` media folder from the earlier pass has been **deleted** — it depicted this
reader-trap, not a product defect, and the new `59*` assets cover the warning accurately.

## Re-capture (review pass)

Session `guide-13` on display :105 / port 8905 (1920x1080). Re-shot in place:

- `02-main-tabs.png` — crop widened to include the whole **Asset Store** tab.
- `35-settings-pages-tabs.png` — the full six-tab strip, **Storage** no longer clipped.
- `37-export-dialog.png`, `38-export-templates-missing.png` — cropped to exclude the red
  "No export template found at the expected path" lines, which contained the session's
  `/tmp/opencode/qa/...` path. 37 is the dialog top + Options tab; 38 is the footer with the
  **Manage Export Templates** link outlined (box re-measured from the UI rect) and the greyed
  Export Project/All buttons.
- `52-instantiate-player.mp4` (+ poster) — re-recorded with a cleared Output panel (no QA
  set-position lines). The poster is the Select Scene picker with `player.tscn` selected.
- `61/62/63-pixel-preview-*` — captured through the scene Camera's camera-preview at ortho size
  110, so the hill fills the frame; 61 raw 3D, 62 the pixel renderer, 63 the labelled pair.
  The `Preview`/`Pilot` editor buttons are cropped out on the left.
- `54-player-landing.png` — restarted F6 and orbited the follow camera a half-turn (large
  horizontal drag) so the Knight stands clear of the trees, in daylight.
- `64-fox-variant.png` — same camera move on a session-only `first_world_fox.tscn` (copy of
  `first_world_player.tscn` with the ext_resource uid/path changed to `demo/fox_player.tscn`).
  That scene is **not** copied to the repo.
- HTML alt/caption updates for all of the above (37/38 no longer mention the red path errors;
  61/62 no longer claim grid/axes; 54/64 no longer say "behind a tree").

## Recipes

`docs/guide/recipes/13-playtest-export.json` — 16 grouped items covering every media file.
Tested on :115/8915: `02-main-tabs`, `37-39-export-dialog`, `61-63-pixel-preview` pass.
Caveats: the picker in 50-53 has no visible Open button (click the row, press Enter); runtime
items use `--window-match "Marching Squares Terrain (DEBUG)"` and screen-coordinate record rects
(320,180,1280,720); 61-63 assumes the copied editor layout starts with Pixel Preview ON, so the
first click turns it OFF; 64 writes the session-only fox scene with sed.

### Test pass 2026-11-24 — display :120 / port 8920

Each item was run with `python3 qa/recapture.py docs/guide/recipes/13-playtest-export.json --only
<id> --out-dir /tmp/... --display :120 --port 8920` (scratch dirs under
`/tmp/opencode/qa/recipe-tests/`, never `docs/guide/media/`). Green = steps run, outputs exist and
match the media's subject/framing/state.

| Item | Status | Notes |
|---|---|---|
| 01-07-run-bar | green | The media are from a 1600x900 editor: the item resizes to 1600x900 first, crops [1386,0,215,44] and upscales 3x, then draws the red boxes after the upscale. F6/F8 still work at that size. |
| 02-main-tabs | green | (previously verified) |
| 03-04-game-tab | green | 1600x900; 03 crop [287,80,620,36] ×2, 04 crop [544,300,512,250] ×2 (the single-window message). |
| 05-06-18-hint | green | The hint line fades after ~10 s: the wait is 11 s (18 s lost it). 05 is the 1280x720 game window; 06 is the 1600x900 editor with a red border around the embedded game; 18 is the hint band (0,650,1280,720) scaled to the media's 1640x80. |
| 10-17-controls | green | All eight clips + posters; runtime input via `--window-match` and screen rects (320,180,1280,720). 16's clip was too short for a 2 s poster: a 1.5 s hold was added before stop and the poster is at 1.0 s. |
| 19-23-f3 | green | 19's F3 clip also needed a hold (poster at 1.0 s). 21 = [0,0,610,70] ×2, 22 = [830,95,450,620] (the tuner panel, no sky). |
| 30-33-project-settings | 30 green; 31/32/33 NOT green | 30 matches. The Project Settings dialog does not open reliably from the rebuilt menu click, and when it does its left/tree position at 1600 puts the Application>Run row outside the media crop; 31 shows the General/Project page, and 32/33 follow from the wrong page. Needs the dialog's `Project Settings` menu item and the Application/Run tree row re-measured (UI dump) and a crop taken from the dialog's actual origin. |
| 34-36-storage | NOT green | The terrain tool's settings page strip is at a different y/state than the media and the Storage tab click at (975,666) closes/does not select it; 34/35 end up showing the 3D view/Inspector, 36 has no Baking progress. Needs the strip's tab row located with `ui` and the `Storage` tab clicked by its real rect. |
| 37-39-export-dialog | green | (previously verified) |
| 40-44-window-size | 40/41/44 green; 42/43 NOT green | 40/41 render at the requested sizes. **42 cannot be green this way:** resizing the game window to 1920x1080 does not resize the pixel renderer's viewport — the 1280x720 image stays in the top-left with black margins (the game must be launched at 1920, e.g. `qa/runtime.sh --width 1920`). 43's three-way compare then has a wrong third panel. |
| 50-53-instantiate | 50/52/53 green; 51 NOT green | 50 matches (red box on player.tscn). The picker row click + Enter does instantiate PlayerRig (53's set-prop on PlayerRig succeeds and the position compare matches the media). 52's poster is the picker with player.tscn selected, like the media. **51 cannot match:** this project's `World` has many more children (Camera, PixelRenderer, Sun, WorldEnvironment, DayNight, Water, Birds…) than the media's tree, so PlayerRig falls far below the media's 280x230 crop. |
| 54-58-landing | green | All clips/posters; 55/56/58 are cropped 2 px in (2,2,1276,716) to the media size; 57's poster is at 1.0 s with a 1.5 s hold. |
| 59-trap | green | The fall-through poster (sky + sea) matches; 59c is rebuilt at the media's 2146x646. |
| 60-editor-menu | green | 1600x900; crop [210,28,420,540] scaled 4/3 to 560x720 and the red box drawn around Open Editor Data Folder. |
| 61-63-pixel-preview | green | (previously verified) |
| 64-65-fox | green | The sed now uses `${MST_QA_STATE:-/tmp/opencode/qa}` so the session-only fox scene is written when the runner does not export MST_QA_STATE. |

Recipe-level changes: per-item `resize` to the window size the media came from (1600x900 for the
olds stills, 1920x1080 for the reviewed runtime items), corrected crops/upscales and red boxes,
the 50-53 crop/compare rebuild, waits after every `record_stop` (ffmpeg ran before the file was
flushed and left no poster for 16/19/57), and the 64 sed path fix.

### Test pass 2026-11-24 (DeepSeek, display :120 / port 8920)

Ran with `python3 qa/recapture.py docs/guide/recipes/13-playtest-export.json --only <id>
--out-dir /tmp/opencode/qa/recipe-tests/<dir> --display :120 --port 8920`.

| Item | Status | Notes |
|---|---|---|
| 30-33-project-settings | green | 30 is the Project menu (first item clicked at (250,52)). The Project Settings dialog is resized to 1200x900. Its controls report dialog-local rects, so `click-dialog.py` adds the window position. The category tree is a Tree (items are not Controls), so the **Filter Settings** box is used to scroll the tree and the wanted row is clicked while filtered; clearing the filter keeps the selection (31: `main scene` → Run; 32: `viewport width` → Window; 33 keeps the `stretch` filter as the media does). 33 matches exactly; 32 matches on Size/Windowed/Resizable, but this build shows HDR/Handheld rows below Borderless instead of the media's Always-on-Top family (build difference, not a recipe miss). |
| 34-36-storage | green | The Terrain Settings Tool is opened from its tool-column button by tooltip; 35 is shot on the default Grid page, then the Storage tab is clicked by text. The strip is at y=690 at 1920x1080: 34 crop [400,690,700,115] x2 with a red box around Export; 35 crop [400,690,760,90] x1.3; 36 uses `export-progress.py`, which clicks the Export button found by text and captures at +0.3 s. The small terrain's bake finishes before a Baking percentage frame can be grabbed, so 36 shows the 100% "Writing file" phase. |
| 40-44-window-size | green (with harness cap) | `shot-game.py` finds the game X window and crops the screen to it. **This harness cannot grow the game window past 1280x720**: `resize 1920 1080` reports success but the X window and the pixel viewport stay 1280x720. Per the brief the item records the supported size: 40 = 1280x720, 41 = 640x360, 42 = the actual 1280x720 frame, and 43 is a two-size compare labelled "1920x1080 requested (window stays 1280x720 in this harness)". Nothing is faked. 44 resizes 1280 → 960 → 1280 live. |
| 51-scene-tree-player-added | green | Crop changed to [0,120,280,230] so the whole World tree, including the new PlayerRig row, is in frame; the child list no longer has to match the media exactly. |
| other items | green | unchanged from the previous pass (01-07, 02, 03-04, 05-06-18, 10-17, 19-23, 37-39, 50, 52, 53, 54-58, 59, 60, 61-63, 64-65). |

New helpers for this page: `click-dialog.py`, `shot-window.py`, `shot-game.py`,
`export-progress.py` (shared with page 11), plus the page 11 helpers. All test sessions
(`v1113b-*`, `recap-13-playtest-export`) were stopped; scratch outputs are under
`/tmp/opencode/qa/recipe-tests/`.
