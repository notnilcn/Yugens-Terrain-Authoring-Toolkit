---
name: godot-best-practices
description: Apply Godot 4 engine best practices from the official best-practices tutorials when writing, reviewing or refactoring a Godot project — Node vs Resource vs RefCounted vs Object, scene vs script, node-tree structure and dependency injection, lifecycle and notification hooks (_init/_ready/_enter_tree/_notification, _process vs _physics_process vs Timer vs input callbacks), node references and preload/load, data structures and enums, autoloads vs regular nodes, project layout and naming, and version control (ignore list, Git LFS). Use when touching .tscn/.tres/.gd/project.godot, Godot scenes, nodes, signals, resources, autoloads, GDScript or GDExtension behavior, project folder structure, or .gitignore/.gitattributes in a Godot project.
---

# Godot best practices

Actionable rules distilled from the official Godot best-practices tutorials. Read
this once before editing, then use §9 as the review pass. Rules are imperative: if
you deviate, say why in the change description.

## 0. How to use this skill

- **Project convention wins.** `AGENTS.md`, the project's style guide and the code
  around you override this skill. Do not "fix" a deliberate local convention to
  match this file; note the conflict instead.
- **Not GDScript? Keep the rules, translate the idiom.** In projects where behavior
  lives in C#, Rust/GDExtension, or engine plugins, everything structural still
  applies (scene vs script, tree shape, ownership, data classes, layout, VCS).
  Never paste GDScript into a codebase whose guide forbids it — express the same
  engine intent in the project's language.
- **Four gates before writing engine code**, each answered by a section below:
  1. What kind of object is this? (§1)
  2. Where does it live in the tree, and who owns its dependencies? (§2)
  3. Which lifecycle hook should carry this logic? (§3)
  4. What shape is the data, and how is it referenced? (§5, §6)
- **Out of scope here** (the source tutorials don't cover it): testing, profiling
  workflow, asset import settings, UI theming, rendering budgets, networking.
  Follow the project's own guidance for those.

## 1. Pick the object type, then scene-vs-script

Cost ladder, lightest to heaviest — choose the cheapest tier that fits:

| Need | Tier | Deciding factor |
|---|---|---|
| Plain data in a custom class | **`RefCounted`** | auto-freed by reference count; the default for custom data classes |
| Data that must serialize to a `.tres`, or be edited/exported in the Inspector | **`Resource`** | only tier giving Inspector editing + serialization; "nearly as light as RefCounted" |
| Absolute lightest object, narrow API, manual memory | **`Object`** | you must call `free()`; a bare `Object` reference can go invalid without warning |
| Anything that exists in the scene tree, has a transform, or needs engine callbacks | **`Node`** | heaviest; logic cost grows per node |

- Prefer **many lightweight data objects owned by one node** over one node per datum
  (the `Tree`/`TreeItem` pattern). Keep their API deliberately small.
- For a **data tree**, derive from your own `Object`/`RefCounted` type, not `Node`.
  Free children from `_notification(NOTIFICATION_PREDELETE)`.
- **Scenes are declarative, scripts are imperative.** A scene declares composition,
  initialization and signal connections; it cannot define behavior. A scene is
  always an extension of the script on its root node — read the two as one class.
- Every script that will sit on a node **must** declare `extends`; without it the
  script is a `RefCounted` and cannot attach to a `Node`.
- **Build larger or more complex objects as scenes.** Script-built hierarchies are
  slower (each call is a scripting-API round trip) and get worse as they grow.
  - If you must build nodes from code, set `owner` on every child or it will not
    be saved with the scene.
- Instantiate with the right API — never swap them:
  - `MyScene.instantiate()` for a `PackedScene`; `.new()` for a script class.
  - `MyScene.instantiate(PackedScene.GEN_EDIT_STATE_MAIN)` when the instance will
    become the base of a scene that inherits from it.
- Refer to scenes/scripts through `const MyScene = preload("res://my_scene.tscn")`
  (`GD.Load<T>` in C#).
- Register a reusable type with **`class_name`**, not `add_custom_type`. Custom types
  are editor-only: no runtime typename, no inheritance, no type awareness.

| | `add_custom_type` | `class_name` |
|---|---|---|
| Runtime typename | no | yes |
| Inheritance | unsupported | full, editor-aware |
| Language coverage | all script languages | per-language |

- Name a scene by giving a `class_name` script a `const` holding it. Add
  `extends RefCounted` so the namespace class stays out of the node-creation dialog.
- Use a scene for anything specific to the game; a script-only type is for generic
  tooling aimed at non-programmers, where editor setup would be a burden.

## 2. Design the tree relationally, not spatially

- **Target zero dependencies.** A scene should instantiate correctly in any context.
  If it cannot work alone, make it work with others *anonymously* — injected from
  above, never reached for.
- Parent-child means "element of", not "nearby". Test: *if the parent is removed,
  should the children be removed too?* If no, they are siblings, not children.
- Godot's tree is **aggregation, not composition** — you may move nodes freely, but
  design so you never need to.
- Choose the loosest binding that works:

  | Child must… | Mechanism | Note |
  |---|---|---|
  | let the parent react | **signal** | response only, never to start behavior |
  | be started by the parent | **`Callable` property** | preferred over a method-name string; ownership unnecessary |
  | be started by the parent, old-style | method-name string + `call()` | child must own the method |
  | use a specific object | node/object reference property | direct, tight |
  | use whatever it is pointed at | `NodePath` property | loosest, resolved late |

- Signals are past-tense facts: `entered`, `skill_activated`, `item_collected`.
  Emit; never use a signal as a command channel.
- **Siblings never talk directly.** An ancestor mediates: `$Left.target =
  $Right.get_node("Receiver")`, or the parent connects them.
- Keep data in-house rather than in any external dependency, however loose.
- If a node requires setup it cannot do itself, implement
  **`_get_configuration_warnings()`** on a `@tool` script, return a non-empty
  `PackedStringArray`, and word each string as an instruction ("Assign a target
  node."). It surfaces in the Scene dock; it does not create the dependency.
- Give the game a **`Main` entry point** that owns a `World` branch and a `GUI`
  branch. Change levels by swapping `World`'s children, not by reloading the scene.
  - Small-game alternative: a `Game` autoload calling `change_scene_to_file()`,
    keeping `World` as the main node — less control, and the GUI must then be a
    singleton, a child of the root, or transitory, or it is deleted on transition.
- Autoload **only** when all three hold: it tracks all its own data, it must be
  globally reachable, and it exists in isolation. A system that mutates *other*
  systems' data is a script/scene, never an autoload.
- Give each subsystem its own branch of the tree. Parent-child only where the
  parent's removal implies the child's.
- Do not split a working scene into sub-scenes after the fact while code holds hard
  references — node paths and editor-made connections break on reuse.
- To stop transform inheritance, insert a plain `Node` between the two nodes, or set
  `top_level` on the `CanvasItem`/`Node3D`.
- In networked games, keep authoritative-server-only nodes in a branch separate from
  `World`.
- Apply SOLID plus DRY/KISS/YAGNI to scenes as well as scripts. Prefer restructuring
  the tree over documenting special-case handling.

## 3. Wire the lifecycle correctly

Initialization order, per instantiation:

1. property default — **setter not called**
2. `_init()` assignments — setter called
3. Inspector/exported value — setter called

- Therefore **never read an exported value in `_init()`**; it is applied afterwards.
  Reading exports belongs in `_ready()`.
- **Set properties before `add_child()`** so setters run outside the tree; the
  setters can cascade and are slow enough to wreck procedural generation. Exception:
  values that only exist in the tree, such as `global_position`.
- Tree callbacks: `_init` runs as each node is instantiated (top-down); `_enter_tree`
  cascades down from the root; `_ready` fires for a node only after **all of its
  children** are ready, so it cascades back up. Only children-first is guaranteed —
  never rely on sibling or unrelated-branch ordering.
- Nodes and standalone scenes created but not added to the tree get `_init` only.
- Use `_init()` for a script's own subtree when it has no scene, and for any
  property/SceneTree-independent state. Use `_ready()` when other nodes' state is
  needed. Use `NOTIFICATION_PARENTED` for behavior that must also run on nodes never
  part of the active scene — and disconnect in `NOTIFICATION_UNPARENTED`.
- `NOTIFICATION_PREDELETE` is the destructor hook; `NOTIFICATION_POSTINITIALIZE` is
  not reachable from scripts.

Notification → hook mapping. Override the dedicated virtual when one exists;
override `_notification()` only for `PARENTED`, `UNPARENTED`, `POSTINITIALIZE`,
`PREDELETE` and custom notifications.

| Notification | Virtual |
|---|---|
| `NOTIFICATION_READY` | `_ready()` |
| `NOTIFICATION_ENTER_TREE` / `NOTIFICATION_EXIT_TREE` | `_enter_tree()` / `_exit_tree()` |
| `NOTIFICATION_PROCESS` / `NOTIFICATION_PHYSICS_PROCESS` | `_process(delta)` / `_physics_process(delta)` |
| `NOTIFICATION_DRAW` | `_draw()` |

Per-frame work, in order of preference:

- **`_process(delta)`** — framerate-dependent delta; state that must update as often
  as possible; recurring logic checks and caching.
- **`_physics_process(delta)`** — framerate-independent delta; kinematic and
  transform work.
- **`Timer.timeout`** — anything that does not need to run every frame. A 2 Hz job
  in `_process` is 58 wasted frames a second.
- **`_input` / `_unhandled_input` / `_gui_input`** — all input. Never poll
  `Input.is_action_pressed` inside `_process`; input callbacks only run on frames
  with input. Use `get_process_delta_time()` when an input callback needs delta.

## 4. Reference and load things efficiently

Node lookup, slowest to fastest:

```gdscript
get_node("Child")          # slowest — string lookup every call
$Child                     # faster — resolved once, still a lookup
@onready var child = $Child            # fastest; survives the node moving in the tree
@export var child: Node                # fastest; survives moving in the Scene dock,
                                       # but must be assigned in the Inspector and can be null
```

Cache repeated lookups. Never put `get_node("...")` in a hot loop.

Loading:

| Situation | Use |
|---|---|
| static path, always needed | `const MyScene = preload("res://...")` — earliest load, editor autocompletion |
| only importing a class | a preloaded constant |
| class may change / swap at runtime | property + `@export` or `load()` |
| a scene may fill the value | property with a `null`/invalid default |
| many dependencies, memory pressure | property + `load()`, set to `null` to free (`RefCounted` frees itself) |
| load timing unknown | avoid `preload` — extra, unexpected loads |

- `const X = load("...")` is an **error**: `load` is a runtime lookup.
- `preload()` in a non-`const` property buys nothing: the value is not reachable
  from the script alone and is copied only at `.new()`.
- `preload()` plus `@export` is a wasted load — scene instantiation overwrites it.
- Constants can only be freed by unloading the whole script.
- Name preloaded constants PascalCase (`const MyScene = ...`); name property-typed
  references snake_case (`@export var my_scene: PackedScene`).

Validate every reference handed in from outside, at the point of use, escalating:

```gdscript
if not prop: return                      # silent
if not prop: printerr("'prop' not set"); return   # logged
assert(prop, "'prop' not set")           # terminate — debug builds only
```

- `assert()` never runs in release export templates: use it for invariants, not
  validation.
- Check capability, not type, for one-off calls: `if child.has_method("set_visible")`.
  Cast once (`if child is CanvasItem`) when making several calls. Prefer injecting a
  `Callable` (`var fn = null` … `if fn: fn.call()`) when you want no structural
  dependency at all.
- Node names (`$Quest`) and groups (`is_in_group("quest")`) are valid interfaces only
  if the project documents the convention.
- If you implement `_set`/`_get`, also implement `_get_property_list()` or symbol
  access (`obj.prop`) reports the property missing.
- Reach a true autoload by path: `get_node("/root/Sound")`; the global identifier
  hides the dependency.
- C# has no dynamic symbol access — use `Set("visible", false)`, not `.visible =`.

## 5. Choose data structures deliberately

| Workload | Choose | Why |
|---|---|---|
| iterate, index get/set, append/pop at the end | `Array` | contiguous, O(1) |
| insert/erase/move anywhere, keyed get/set | `Dictionary` | hashing, constant-time, memory overhead by design |
| search by value | neither (unsorted) | `find` is the slowest operation in both |
| invariants, stable API, signals, reactive behavior | class deriving `Object` | control, clarity, convenience |
| a tree of custom nodes | your own `Object`-derived type | `Node` drags in behavior a data structure doesn't need |

- Dictionary iteration is **insertion order** — reliable to iterate.
- Inserting/removing at the **front** of an `Array` copies the tail every time. If
  you must do many, invert the array, mutate at the end, re-invert: two copies total.
- Repeated value→index search: keep the array sorted and write an order-aware
  search (logarithmic), re-sorting after edits.
- Object property access costs more as the inheritance chain deepens (script →
  ClassDB → base classes). Bulk property reads in a hot loop are exactly the "the
  scripting API is slow" case — use arrays/dictionaries for the inner loop.
- `enum` as **ints** when comparisons dominate (constant-time); as **strings**
  (`@export_enum`) when the values are mainly printed or grouped for readability.
- Animation, in order of capability: `AnimatedTexture` (fixed FPS, no per-frame
  logic, batched, works in `TileSet`) → `AnimatedSprite2D` + `SpriteFrames` (flip,
  speed, offset) → `AnimationPlayer` (trigger side effects, cut-out and mesh
  animation) → `AnimationTree` (blending, hierarchy).
- Measure scale before optimizing: linear-time is fine when the data set is small
  *and* the operation is infrequent. Millions of items per frame is not.

## 6. Autoloads are the last resort

Try, in order:

1. state and the nodes using it in the **same scene**;
2. shared **behavior** → a `class_name` `Node` subtype;
3. shared **data** → a `Resource` type, or an object reachable from the scene root
   via `owner`;
4. shared **functions** → `class_name` + `static func`;
5. shared **variables** → `static var` (Godot 4.1+);
6. an **autoload**, only for wide-scope systems that own their data.

Autoload hazards to state when you add one: global *state* (one object owns
everyone's data), global *access* (any caller, bug source unknowable), global
*allocation* (a preallocated pool is either too small or too large). An autoload is
a node under `/root`, not a language singleton — nothing stops duplicate instances.

## 7. Project layout and naming

- **Colocate assets with the scenes that use them**; keep a separate folder for
  genuinely shared base assets (sprites, meshes, materials, music) and a separate
  `levels/` tree for built levels.
- `snake_case` for every folder and file. Exception: C# scripts, named after their
  PascalCase class. `PascalCase` for node names, matching built-in nodes.
- Third-party code goes in a top-level `addons/` even when it is not a plugin.
  Exception: third-party art built for one character belongs in that character's
  folder.
- `.gdignore` (empty file, leading dot required) excludes a folder from import. It
  takes **no patterns** and its contents are ignored; ignored files cannot be
  `load()`ed at runtime.
- Exports are case-sensitive (PCK virtual filesystem) even though Windows/macOS dev
  filesystems usually are not — lowercase names are the portable answer.
- On Windows, make the project folder case-sensitive to catch mismatches early:
  `fsutil file setcasesensitiveinfo <project path> enable`.

## 8. Version control hygiene

Pin this to the engine version — the Godot 3.x/4.0 and 4.1+ lists are "entirely
different". For **4.1+** ignore exactly:

```gitignore
.godot/
*.translation
```

- `export_presets.cfg` may contain credentials on 3.x/4.0 — check before committing.
  This is version-specific; verify rather than assume.
- Generate `.gitignore`/`.gitattributes` from the editor (Project → Version Control →
  Generate Version Control Metadata) rather than hand-writing them.
- If you write them by hand on Windows, set `git config --global core.autocrlf input`;
  with `true`, Git reports files modified from LF→CRLF conversion. If the generated
  metadata is present, do not change Git config — it already enforces LF.
- Configure **Git LFS before the first commit** of any binary asset:
  `git lfs install`, then `git lfs track "*.glb"` per type.
- Start `.gitattributes` with `* text=auto eol=lf`, then the LFS groups (3D models,
  images, audio, fonts, and binary Godot resources: `.scn`, `.res`, `.material`,
  `.anim`, `.mesh`). Text `.tscn`/`.tres` stay in normal Git.
- If binaries are already committed, create a fresh LFS repo and copy files across;
  `git lfs migrate` works but is expert-only.
- Every clone must run `git lfs install` or it gets pointer files.
- Do not invent extra ignores: check the editor's generated metadata or the project's
  own list first.

## 9. Review pass

Run this on any Godot diff before you call it done.

- [ ] Every node script declares `extends`; every scene has a script on its root.
- [ ] New object uses the cheapest tier (Object/RefCounted/Resource/Node) that meets
      its needs; data trees derive from `Object`, not `Node`.
- [ ] Complex node hierarchies are scenes, not code-built trees; code-built children
      set `owner`.
- [ ] `.instantiate()` for scenes, `.new()` for scripts.
- [ ] The scene instantiates in isolation, or its dependencies are injected from a
      parent. No child reaching out for context.
- [ ] Signals only report; behavior is started by a `Callable`, method, or injected
      reference. Signal names are past tense.
- [ ] Siblings communicate through an ancestor.
- [ ] Parent-child relations pass the "parent removed ⇒ children removed" test.
- [ ] Required setup is surfaced by `_get_configuration_warnings()` on a `@tool`
      script, worded as instructions.
- [ ] Autoload passes all three conditions (own data, global scope, isolation), and
      does not mutate other systems' data.
- [ ] State is initialized in `_init` only when SceneTree-independent; exports are
      read in `_ready`, never `_init`.
- [ ] Properties are set before `add_child()`.
- [ ] No sibling/unrelated ordering assumptions; no per-frame work that a `Timer`,
      signal, or input callback could carry.
- [ ] No input polling in `_process`/`_physics_process`.
- [ ] Dedicated virtuals used instead of `_notification` where one exists.
- [ ] No `get_node("...")` in hot paths; references are cached (`@onready`/`@export`).
- [ ] `preload` only for static, always-needed paths; no `const X = load(...)`; no
      `preload` on exported values a scene fills.
- [ ] Externally supplied references are validated (`return` / `printerr` / `assert`),
      and `assert` is not load-bearing in release.
- [ ] Array vs Dictionary vs Object matches the access pattern; no `find` in loops;
      no bulk Object property reads per frame.
- [ ] Enums are ints for comparison-heavy code, strings for display.
- [ ] Names: snake_case files/folders, PascalCase nodes, `addons/` for third-party,
      colocated assets.
- [ ] Ignore list matches the engine version; binaries are LFS-tracked; EOL is LF.
- [ ] Project-specific conventions in `AGENTS.md` were followed where they differ.

## Source

Distilled from `godot-docs/tutorials/best_practices/`:
`introduction_best_practices`, `what_are_godot_classes`, `scene_organization`,
`scenes_versus_scripts`, `autoloads_versus_regular_nodes`, `node_alternatives`,
`godot_interfaces`, `godot_notifications`, `data_preferences`, `logic_preferences`,
`project_organization`, `version_control_systems`.

When this file and the tutorials disagree, the tutorials win — re-read the relevant
`.rst` and update this skill.
