# Orchestration tracker (orchestrator-owned)

Gate: `qa/wave-cap.sh` before each launch (5 / 3@40% / 2@60% / 1@80%, min with resource monitor).
Capture agents: claude-opus-4-8 effort medium. Writers + fixers: claude-opus-5-5 effort medium.

| Area | Page(s) | Capture | Writer |
|---|---|---|---|
| C01 | 01, 02 | DONE (31 clips, 62 png; scene tutorial/guide/first_world.tscn) | DONE 01 + 02 html |
| C02 | 03 | DONE (119 files, 30 clips, erosion §8) | DONE 03 html (weak: aspect stills, bridge/smooth compares) |
| C03 | 04 | DONE (40 png + 10 clips) | DONE 04 html |
| C04 | 05 | DONE (16 clips + 50 png) | DONE 05 html |
| C05 | 06 | DONE (11 clips + 57 png) | DONE 06 html |
| C06 | 07 | DONE (55 media; gaps: no palette-preset shots, failed 25-debug-normals + 27-rayalpha-0.5, Pixel Object strip top cut, only 2 clips) | DONE 07-pixelizer.html (touch-up later if budget) |
| C07 | 08 | DONE + C07b recapture DONE (89 media, 13 clips; notes corrected) | DONE 08 html (241 cards) |
| C08 | 09 | DONE (54 png + 26 clips; follow-up done) | DONE 09-builder-greybox.html (7 clips show pre-F3 see-through parts → re-record in touch-up) |
| C09 | 10 | DONE (32 stills + 26 clips) | DONE 10-builder-modeller.html |
| C10 | 11 | DONE (10 clips + 39 png; scene tutorial/guide/imported_models.tscn) | DONE 11 html |
| C11 | 12 | DONE (16 clips + 25 png after C11b; scene model_uses.tscn enriched) | DONE 12-using-models.html (41 media) |
| C12 | 13 | DONE (72 media, 17 clips; W5 player walk; scene tutorial/guide/first_world_player.tscn) | DONE 13 html |
| final | index, 14, 15, search index, review | – | DONE index/14/15 html; review pending |

Fixes: F1 Shift+wheel zooms camera — FIXED (editor/plugin.rs claims_wheel; release events). F2 Ctrl+wheel hints + strength clamp — FIXED (tools.rs, plugin.rs).
Note: resource monitor exited on a disk-write alert at 03:07Z (session project copies); restarted 03:49Z.

Launch queue (window 3 from 12:49Z; paused 92% at 15:52Z until reset 17:49Z): F8 fixer (builder meshes see-through in hex_island: evidence docs/guide/notes/touchup-pageB-seethrough-box.png; binary DOES include F3) + writer 12 → integration writer (07 touch-up media; 09 after greybox re-record) → greybox re-record (C08c) → final review agent → rebuild search index.
F3 new parts dark/see-through — FIXED (part_rules.rs, roblox_panels.rs: Medium Stone Grey default). F4 `!is_inside_tree()` on Insert Part after switching scenes — FIXED (parts_model.rs live_terrain, placement.rs view_ray_hit skip out-of-tree terrain). Harness: capture hang in idle editor fixed by orchestrator (qa/harness/harness.gd queue_redraw before frame_post_draw); template refreshed.

F5 lighting-preset fog stuck on + weather blend frozen in editor — FIXED (effects/day_night.rs fog_switched_on, effects/weather.rs snap when clock stopped). Birds/LightVolume/SunShafts: not a bug (subtle by design; birds small, light pools at night, shafts sun-angle).

F7 'scene doesn't render as 2nd tab' — FIXED: camera_framing.rs zoom_out release events zoomed too (doubled steps since MST-BUG-0078) → camera parked ~404 m away.

Monitor keeper: /tmp/opencode/qa/tools/rmon-keeper.sh (restarts monitor after alerts).

Touch-up capture: 07 DONE (re-takes 15/25-*/26/27-*, palette series 80–85, clips 41–48; manifest notes/touchups.md). 09 BLOCKED (inserted parts see-through) → F8.

2026-11-24 ~19:00Z (window 4; machine had rebooted → /tmp rebuilt: monitor init+start, keeper re-created, ffmpeg via imageio-ffmpeg at /tmp/opencode/qa/tools/ffmpeg, template refreshed).
F8 — FIXED (roblox_panels.rs give_new_part_material clears slots copied from the selected mesh, greys default faces; root cause: create_shape_under copied Ruin's moss material → looked see-through). Evidence notes/f8-fixed-*.png.
Writer 12 DONE. 07 touch-up integration DONE (+ orchestrator corrected HueWeighted/Weights from pt_core/palette.rs). Search index rebuilt (393 sections).
Review pass DONE (briefs/_review.md): index,01–06 and 07,08,10–15 reviewed + fixed in place. Harness 'listening' print → print_verbose (qa/harness/harness.gd) so Output panel stays clean; template refreshed.
Running: C08c greybox re-record (8 clips) · recap-a 01/02/06 · recap-b 03/04 · recap-c 07/08 · recap-d 10/12 (briefs/_recapture.md).
Queue after: recap-e 11/13 (fox hidden in trunk 81/82; harness text 30/42/52; qa_tmp folder 05/52/55; crops 03/40/50 on 11; 13: /tmp paths 37/38, log lines 52, crop 35, weak 61-63, 54/64, 02) → 09 integration writer (replace 8 clips' captions; drop 'earlier build' notes; correct notes verdict 'INTENDED'; 15-troubleshooting line ~211 fixed-list wording) → rebuild index. (User 2026-11-24: NO more review passes.)

Recipes (user 2026-11-24: wants re-runnable capture): format + runner `qa/recapture.py` (tested live), guide `docs/guide/recipes/README.md`, per-page `docs/guide/recipes/<page>.json`. `qa/drive.py` now logs every call to `qa/cmdlog/port-<port>.jsonl` (gitignored). Running agents steered to write recipes (C08c→09, recap-b→03/04, recap-c→07/08, recap-d→10/12); recap-a re-tasked for 01/02/06 from its transcript. Remaining: recap-e 11/13 must write recipes too; the media no agent re-shot this pass (most of every page) has NO recipe yet — backfill from the original capture threads' transcripts + notes crop rects (needs a decision from user).

2026-11-24 ~20:45Z — USER: stop Claude agents, use OpenCode DeepSeek (deepseek/deepseek-flash, variant high) for capture/re-capture/backfill; Godot on NVIDIA GPU.
GPU: qa/gpu.sh (VirtualGL 3.1.3 EGL back end, unpacked to /tmp/opencode/qa/tools/vgl) wired into qa/session.sh; verified editor.log "Using Device: NVIDIA RTX 5060 Ti". MST_QA_GPU=0 disables.
Monitor re-inited (claude-usage-percent 100, disk-write-mb 4096) so its parallel number reflects machine load for the DeepSeek wave; keeper restarted. Claude cap rule (wave-cap.sh) still applies to any Claude agent (fixers).
Cancelled Claude agents (partial work landed: media 01/02/06 done, 07/08 mostly, 03 partial, 10/12 partial, 09 all 8 clips; recipes 01,02,07,08,09).
Running DeepSeek (brief briefs/_deepseek.md; displays :101-105 work, :111-115 recipe tests): DS-09 (verify clips, recipes, 09 HTML + 15 + notes verdict), DS-03/04, DS-07/08, DS-10/12, DS-11/13 — each also backfills recipes for all media on its pages.
Queue: DS-01/02/06 (test existing 01/02 recipes, write 06 recipe, backfill all three) · DS-05 (backfill 05) → rebuild search index (python3 -I docs/guide/tools/build_index.py). No reviews (user).
Bugs from DeepSeek agents → Opus 5.5 medium fixer per _fixer.md (check wave-cap first).
DS-11/13 DONE (recipes 11: 18 items, 13: 16 items; only 7 tested → queue a test-all follow-up). DS-07/08 re-woken (had ended turn with a background suite). DS-10/12 round 1 DONE (re-shoots + captions; recipes mostly placeholders); its cleanup deleted other agents' sessions (they were notified; brief rule added). DS-10/12 round 2 running: re-centre modelled_prop Box → 0,0,0 + model_uses instance Y 0, fit stone_tallB2 collider, film 61/63/58, rewrite page-12 origin gotcha, finish+test all 10/12 recipes.
Queue: DS-01/02/06 · DS-05 · DS-11/13 test-all recipes → index rebuild.
DS-10/12 round 2: scenes fixed (modelled_prop Box at origin; model_uses instance Y 0; stone_tallB2 collider fitted), page-12 gotcha rewritten. BLOCKED: F9 addon bug — scatter (foliage paint / Fill Terrain) with a PixelBuilderMesh prop scene adds instances but renders nothing (collects mesh before the builder builds). Fixer (Opus 5.5 medium) waits for Claude reset ~22:49Z (timer btdomob5k). 58-f6-run-game: drive.py `key --hold S` added → re-film with DeepSeek. Recipes 10/12 still mostly untested placeholders.
Queue: F9 fixer → re-film 61/63 (+ 58 with key --hold) + test-all 10/12 recipes (DeepSeek) · DS-01/02/06 · DS-05 · DS-11/13 test-all recipes → index rebuild.
DS-07/08 DONE: recipes 07 (22 items) + 08 (32 items, all 89 files), tested. Open: 07 `50-feature-switches` (Project Settings not scriptable — no recipe), 07 `81–85` quantize (now possible: harness set-prop supports `palette:quantize_mode`, verified live) → follow-up. Bugs for fixer: F10 Pixel World ▸ Light Volume ▸ +Add creates bare PixelLightVolume (no Light/Volume children → renders nothing; shipped effects/pixel_light_volume.tscn works); F11? PixelParticleField dust only sub-pixel haze in editor preview (check whether intended).
Harness: drive.py `key --hold`; set-prop `a:b:prop` paths + emit_changed; undo action name "Set <prop>" (was "QA set", showed in Output).
DS-09 DONE: 8 clips re-recorded on the fixed build + reproduced by recipe; 09 HTML/15/notes fixed; recipes 09: 38 items (13 green, ~25 unverified); helper docs/guide/recipes/09-frame.py (camera placement). Tower clips small (wide viewport) — lower editor VFOV next time.
Running: DS-03/04, DS-01/02/06 (recipes only). Fixer timer bw55oybzg → 23:52Z.
Queue: F9/F10 fixer → DS-12 re-film 61/63/58 + test-all 10/12 recipes · DS-05 backfill · DS-verify untested recipes 09/11/13 + 07 quantize 81–85 → index rebuild.
DS-03/04 DONE: re-shots (smooth, gradient, brush-type 10–15, 04 poster 23), 03 HTML restored (aspect scrubber, smooth + bridge compares, clip 49); recipes 03: 55 items (54 green; erosion needs MarchingSquaresErosion child), 04: 20/20 green.
Running: DS-01/02/06, DS-05 + 07 quantize 81–85. Fixer timer brh82nt2q → 23:52Z.
Queue: F9/F10 fixer → DS-12 re-film 61/63/58 + test-all 10/12 · DS-verify untested 09/11/13 → index rebuild.
2026-11-24 00:20Z: F9 FIXED (terrain/scatter.rs instance_mesh(): reassemble PixelBuilderMesh before reading mesh), F10 FIXED (editor_scenes.rs LIGHT_VOLUME_RIG + pixel_ui/actions.rs add_effect instances effects/pixel_light_volume.tscn), F11 FIXED (particle_field.rs layers built in enter_tree; terrain/node.rs enter_tree republishes height) — default dust faint by design (Beam 0 + Density 1 → visible). Evidence notes/fix-f9|f10|f11-*.png. Possible same tab-switch bug in PixelWeather rain sheets (unchecked).
Monitor had stopped 21:15Z (signal) + keeper died again → restarted.
Running: DS-01/02/06, DS-05+07q, DS-12 re-film + 10/12 recipes + 08 dust.
Queue: DS-verify untested recipes 09/11/13 → index rebuild. Optional: fixer check PixelWeather tab-switch.
Fixers (user: "launch the fixers"): F12 PixelWeather rain sheets lost after tab switch — FIXED (effects/weather.rs enter_tree). F13 audit — FIXED PixelSky cloud provider (effects/sky.rs), PixelFireSystem enabled flag (effects/fire.rs), PixelRenderer runtime re-enter (render/renderer.rs), PixelAnimationStepper (render/stepper.rs); NOT A BUG: PixelObject, PixelBuilderMesh, PixelWater, terrain, diagnostics, plugins. F14 — FIXED PixelDayNight counter (effects/day_night.rs enter_tree) + PixelModelerHud undo connect (builder/modeler_ui/hud_panel.rs enter_tree). Evidence notes/fix-f12|f13|f14-*.png. None committed.
DS-12/10/08 DONE: re-filmed 12 61/63/58, 08 51 dust (+ tip), HTML updated; recipes green: 12 builder-prop-paint/builder-fill, 08 51, 10 ribbon/mst-layout/modeler-layout/crate-*. Left non-green: 12 f6-run, palette-grey-vs-coloured, stone-boulders, pass1/pass2; 10 panels-and-dialogs, shortcut-clips, uv-editor, painting.
Running: DS-01/02/06, DS-05+07q, DS-verify-09 (:119), DS-verify-11/13 (:120).
Queue: DS-verify 10/12 leftovers → index rebuild.
DS-05+07q DONE: recipes 05: 37 items / 66 files, all green individually; 07: 80–85 palette modes via set-prop, 6/6 green.
Running: DS-01/02/06, DS-verify-09 (:119), DS-verify-11/13 (:120), DS-verify-10/12 leftovers (:121).
Queue: index rebuild when all done; final recipe coverage report (qa/recapture.py --list docs/guide/recipes).
DS-01/02/06 DONE: recipes 01 44/44, 02 22/22, 06 41/41 green (framing deviations noted in notes). qa/session.sh stop now kills process groups and waits for exit (fixes stop→start race); verified.
Running: DS-verify-09, DS-verify-11/13, DS-verify-10/12 leftovers.
DS-verify-11/13 pass 1: 11 9/18, 13 11/16 green (coordinate drift; 11/41 media shows old Create Root Node dialog). Pass 2 running (:120/:123) with runtime-located controls; may re-shoot 11/41.
DS-verify-09 DONE 33/38 (not green: 05-select-tool water click switches workspace; 25-28 + 30-03 media are pre-fix orange parts; 40-doorway-stills multi-select union; 06 corner-scale readout).
DS-verify-10/12 DONE: 12 5/7 (pass1/pass2 placeholders), 10 8/10 (painting clips 18–21, shortcut-clips open). f6-run needs --window-match 'Marching Squares Terrain (DEBUG)'.
Running: DS-verify-11/13 pass 2. Then: index rebuild + final coverage report.
User: re-shoot whatever needs it (via recipes). Running DS re-shoot (:124/:125): 09 pre-fix orange media (25-28, 30-03, 40-01, 50, + any other orange), tower clips 22–24 tighter (VFOV), 05/06 if possible; 07 16-object-outline pair. 11/41 handled by DS-verify-11/13 pass 2.
DS-verify-11/13 pass 2 DONE: 11 18/18, 13 16/16 green; 11/41 re-shot (inline Create Root Node menu); 13/42 records real 1280×720 cap. Helpers in docs/guide/recipes/_helpers/.
Running: DS re-shoot stale 09 + 07.
DS re-shoot DONE: 09 05/06/22–28/30-03/40-01/50 re-shot (grey parts, VFOV 40 tower, calibrated drags with inspector readouts), 07 16 outline pair closer; helpers 09-vfov.py, 09-grip.py. Running: DS re-shoot 40-doorway-03-result (recipe multi-select fix; union itself works).
DS re-shoot 40-doorway-03 DONE (recipe needed pointer back over 3D view before Ctrl+Shift+G). All queued work complete; index rebuilt.
