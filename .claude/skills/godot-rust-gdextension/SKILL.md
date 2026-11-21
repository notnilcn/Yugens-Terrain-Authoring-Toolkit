---
name: godot-rust-gdextension
description: Conventions for a Godot 4 game whose behavior is written entirely in Rust (GDExtension) and whose composition lives in .tscn scenes — header-only comments, the explanatory header required for every custom node type, exporting node values by default, blast radius for names the Rust compiler cannot check, when nodes may be created in code, and Godot engine rules translated to Rust. Use when adding, renaming, moving or removing a Rust class, field, exported property, signal, callable method, node, scene or module; when defining a node type or adding a field or tuning value to one; when writing or editing a comment in a Rust file; when spawning or building nodes from Rust; and when reviewing or refactoring Rust GDExtension code or its scenes. Not needed for a small edit inside one function that changes no names, fields, tuning values, comments, scenes or node creation.
---

# Godot with Rust GDExtension

Rules for working on a Godot 4 game whose behavior is written entirely in Rust
(GDExtension) and whose composition lives in `.tscn` scenes. There is no GDScript: never
add a `.gd` file or an embedded script to a scene.

**The code is the source of truth.** Everything below exists so that nothing has to be
kept in sync with the code by hand. This skill follows its own rule: it states policy and
records nothing about a real project. Discover the project's crates, folders, classes and
versions from the project on every task.

Every name, number and file in an example below is made up for illustration.

## 1. Precedence and deviations

- An explicit instruction from the user wins over this skill. So do the project's own
  `AGENTS.md` and its established conventions. The API docs of the Rust binding crate
  pinned in the manifest win over any API detail remembered or implied here.
- Every rule in this skill may be broken when following it would make the result worse.
  Breaking one silently is not allowed.
- Report each deviation in your final message and in the change description, in this form:
  `Deviation: <rule> at <file or scene>. <why following the rule was worse here>.`
- Never put the justification for a deviation in a code comment. That recreates the thing
  §2 removes.
- "It was easier" and "the surrounding code already does it" are not justifications.
- Three things are not deviations: defining a custom node type, creating nodes in code
  for one of the reasons in §4, and keeping a node's value unexported for one of the
  reasons in §5. Their justifications go in the file header, as §2, §4 and §5 describe.

Reports that are acceptable:

- `Deviation: comments (§2) at the collision query module. A safety note sits beside the
  unsafe block because the project's lint settings fail the build without one.`
- `Deviation: additive-only at a hub (§3) at the shared damage event. The new field has no
  safe default, so the signature changed and all fourteen call sites were updated here.`

Reports that are not:

- `Deviation: comments. Added notes to explain the tricky parts.` It names no place, and
  "the code is tricky" is what every comment claims. Rename or restructure instead.
- `Deviation: blast radius at the player class. Renaming the property was quicker than
  adding one.` Quicker for the agent does not make the rule worse to follow.

## 2. Comments

**The only comment an agent may write is one header at the very top of a file, and it may
only say, broadly, what the file is for or why it exists.** Two kinds of file must have
one: a file that defines a custom node type, whose header explains the node at length
(below), and a file that creates nodes in code, whose header says why (§4).

- This covers every comment form: `//`, `/* */`, `///`, `//!` and `#[doc = "..."]`. No
  comments on items, fields or lines. No `TODO`, `FIXME`, `NOTE` or `SAFETY` notes, no
  section banners, no commented-out code, no change history.
- In a `.rs` file the header is one `//!` block before any item. In other hand-written
  files (manifests, ignore files, scripts) it is that format's comment at the top.
- Files the editor rewrites get no comments at all: `.tscn`, `.tres`, `project.godot`,
  `.import`. A comment there does not survive a save.
- Leave shebangs and license headers alone; they are not yours.

Put what a comment would have said somewhere the compiler or the engine checks:

| You want to say | Say it with |
|---|---|
| what this does | a name: function, type, variable, test |
| what values are valid | a type: enum, newtype, `Option`, `Result` |
| why a lint is silenced | `#[expect(lint, reason = "...")]` |
| what must hold here | an assertion with a message, or a test |
| what the scene author must set up | a configuration warning on the node |
| why the change was made | the change description |

| Don't | Do |
|---|---|
| `// speed in pixels per second` above `speed: f32` | `speed_pixels_per_second: f32` |
| `// 0 = idle, 1 = running, 2 = jumping` above `state: i32` | `enum Locomotion { Idle, Running, Jumping }` |
| `// must be called after the inventory loads` | take the loaded inventory as a parameter, so it cannot be called earlier |
| `#[allow(dead_code)] // the engine calls this by name` | `#[expect(dead_code, reason = "the engine calls this by name")]` |
| `/// Returns the nearest enemy.` on `fn nearest_enemy` | nothing; the name already says it |
| `// TODO: handle an empty list` | handle it, or say it is unfinished in your report |
| `// fixes the double-jump bug` | the change description, plus a test named for the behavior |
| `// --- movement ---` banners | split the file, or leave it unbanded |
| old code kept in a comment | delete it; version control has it |

### Header checklist

For most files the default outcome is **no header**. Go through these in order; one "no"
means no header.

1. Would a reader fail to guess why this file exists from its path, its name and its
   public items?
2. Is the text about the file as a whole (its purpose, a constraint on it, or the reason
   it is separate) and not a list of what it contains?
3. Would it still be true if every item in the file were rewritten, renamed or reordered?
4. Is it free of anything that expires: names of functions, fields, types, nodes or other
   files; counts; dates; tickets; authors; "currently", "for now", "temporary"?

For the two required headers, item 1 is already answered and items 2 to 4 still apply to
every word. A "no" there means rewrite the header, not drop it.

| Header | Verdict |
|---|---|
| `//! Keeps damage rules free of engine types so they can be tested without a running game.` | passes |
| `//! The only place that knows the save format, so a format change stays here.` | passes |
| `//! Inventory logic.` in a file named for the inventory | fails 1: the name already says it, so no header |
| `//! Contains the Player struct and its three movement helpers.` | fails 2, 3 and 4: a list of contents, with a type name and a count |
| `//! Rewritten in March to fix the double-jump bug.` | fails 2 and 4: history and a date |
| `//! Temporary until the input rework lands.` | fails 4: it expires |
| `//! See the spawner file for how these are created.` | fails 2 and 4: it points at another file |

### Headers for custom node types

**A file that defines a custom node type must open with a header that explains the node
at length.** A custom node type is a Rust class registered with the engine whose base is
a node.

The header answers five questions in prose:

1. **Intent.** What is this node for, in the game's own terms? What does the person
   placing it in a scene expect it to do for them?
2. **Responsibility.** What does it own, and what does it deliberately leave to others?
3. **Why a node.** Why does it need a place in the tree, instead of being a plain type or
   a resource (§6)?
4. **Why a new type.** Why is a built-in node configured in a scene, or an existing
   custom type, not enough?
5. **What is not exported.** Which kinds of state does it keep out of the Inspector, and
   why (§5)? A node that keeps nothing out needs no answer here.

- There is no upper limit on length. A header that skips one of the first four questions
  is too short, so a one-liner always fails.
- Write intent, not mechanism. The test: **the header changes only when what the node is
  for changes, never when how it works changes.** The one other trigger is a new kind of
  unexported state (§5).
- Checklist items 2 to 4 apply in full. Do not name the type, its base class, its
  properties, methods, signals or child nodes. No numbers, no tuning values, no callers.
  Describe everything by role.
- It goes in the `//!` file header, not in `///` on the struct.
- Prefer one node type per file, so the header is about one thing. If the file also
  creates nodes in code, the same header carries that justification (§4).
- If you cannot answer question 3 or 4, the type should not exist. Use a plain type, a
  resource, or a built-in node in a scene.
- Write it when you create the type. Rewrite it when a change alters what the node is
  for. Do not add headers to node files you are not otherwise changing.
- Adding an unexported value to an existing node type that has no header is the point at
  which you write the whole header (§5).

Do:

```rust
//! A hazard that hurts whatever stays inside it.
//!
//! Intent: damage over time should be something a level designer drops into a level
//! and reshapes without touching code. Placing one of these is the whole job.
//!
//! Responsibility: it decides who is being hurt and how often. It does not own anyone's
//! health and does not decide what being hurt means; it only tells its victims that it
//! happened.
//!
//! Why a node: it needs a place in the world, and it needs the engine to tell it what
//! is overlapping it.
//!
//! Why a new type: no built-in node pairs a region with a repeating effect, and putting
//! the rule on the victims would repeat it in every kind of victim.
//!
//! Not exported: who is inside it at the moment and when each was last hurt. That only
//! means something while the game runs, so there is nothing for a designer to set.
```

Don't:

```rust
//! LavaPool extends Area2D. Exports `damage_per_tick` (default 5) and `interval`.
//! Emits `burned` every half second to bodies in the "damageable" group.
//! Used by the third level.
```

It names the type and its base, a property, a default, a signal, a group and a caller.
Each of those goes stale on an ordinary edit, and none of them says what the node is for
or why it exists.

Don't:

```rust
//! The lava pool.
```

It answers none of the questions.

### Existing comments

- When you edit a file that has a header, run the checklist on it again. An optional
  header that now fails is fixed or deleted; deleting it is always acceptable. A required
  header that now fails is rewritten until it passes.
- Any other comment your change makes false is deleted, not rewritten.
- Do not sweep comments out of code you are not otherwise changing.

The same reasoning applies to prose outside the code: do not write architecture notes,
module lists or changelogs unless the user asks for them.

## 3. Blast radius

A change should force edits only where the change is the reason. Before editing anything
other code or scenes depend on, find out how far the edit reaches.

### Two kinds of reference

- **Compiler-checked:** Rust referring to Rust. A break shows up as a build error.
- **String-bound:** everything the engine resolves by name at load time or run time. The
  compiler sees none of it. A rename builds cleanly and breaks the game.

The string-bound surface of a Rust class and its scenes:

- the registered class name and its base class
- exported property names and types, and callable method and signal names and signatures
- **exported defaults:** a scene does not store a property equal to its default, so
  changing a default silently changes every scene that relied on it
- **exported enum order:** scenes store the integer of an integer-backed enum, so
  reordering or inserting variants silently changes meaning
- node names, node paths, scene-unique names and groups
- input actions, autoload names, layer names, animation names
- `res://` paths and resource UIDs

A build that passes is evidence about the first kind only. It says nothing about whether
the scenes still load.

Every change in this table builds cleanly:

| Change | What breaks with no build error | Do |
|---|---|---|
| rename exported `max_health` to `health_cap` | the value each scene stored under the old name is no longer applied | search scene and resource files for the old name; update them in the same change |
| rename a method a scene connection calls | `[connection ... method="on_player_died"]` no longer finds it | search for the name as a string, not only as an identifier |
| change an exported default from 200 to 300 | every scene that never stored the value now behaves differently | keep the default; set the new value in the scenes that should change |
| insert an enum variant in the middle | scenes that stored the old integer now mean a different variant | append the variant at the end |
| rename the class | every `type="OldName"` in a scene stops resolving | treat it as a rename on a shared surface |
| rename a node in a scene | Rust lookups, connections and animation tracks that use the old path stop resolving | search Rust and scenes for the old name |

### Measure, then act

Count direct dependents: the distinct Rust files **and** scene or resource files that
reference the thing. Search for them every time; never work from a remembered list.

| Direct dependents | Treat as | Rule |
|---|---|---|
| 0–2 | local | edit freely |
| 3–10 | shared surface | list the dependents in your report, prefer an additive change |
| more than 10 | hub | additive only, unless the hub is the point of the change |

- **Additive** means a new type, method, signal or module, or a new field or exported
  property whose default keeps existing behavior.
- A rename, removal, signature change, default change or enum reorder on a shared surface
  is its own change, with every Rust and scene reference updated in that same change.
- A hub is legitimate when it carries vocabulary or contracts and not decisions, has one
  reason to change, and depends on less than depends on it.

| Don't | Do |
|---|---|
| decide a type is local because you remember two users | search and count |
| add a parameter to a method that fourteen files call | add a second method that takes it; leave the first alone |
| add a boss-only field to the shared health type | keep it in the boss's own module |
| rename a shared signal while fixing an unrelated bug | fix the bug; do the rename as its own change |
| report "builds, done" after renaming an exported property | report which scene files were searched and updated, and whether they were loaded |

### Keeping the radius small

- Put behavior in the smallest module or scene that can own it.
- Dependencies point one way. Game-specific code depends on generic code, never the
  reverse, and a generic module never contains a feature's name or policy.
- Keep rules and state in plain Rust that knows nothing about the engine, with a thin
  layer of engine classes on top. The plain part has no string-bound surface.
- Where both ends of a connection are Rust, prefer wiring the compiler can check over a
  name stored in a scene.
- Moving a Rust type between modules does not touch scenes as long as its registered
  class name is unchanged. To move it without a flag day: move it, leave a `pub use` at
  the old path, migrate callers, then delete the re-export.
- Moving or renaming a scene or resource file changes its path. Update every reference
  in the same change.
- No `utils`, `helpers` or `common` modules. If code is shared, the thing it does has a
  name.
- Before adding a dependency, check the manifest for one that already covers it. A
  dependency added to a hub is added to everything above it.

| Don't | Do |
|---|---|
| a generic health component that checks whether its owner is the boss | the boss configures the health component from above |
| a damage formula written inside a node's method | a plain function taking plain values, called by the node |
| a scene connection naming a Rust method, between two Rust classes you own | connect them in Rust, where a rename is a build error |
| move a type and fix every import in one change | leave a `pub use` at the old path, migrate, then remove it |
| a `utils` module holding an angle helper and a time formatter | one module named for angles, one for time display |
| a second math crate because its API is nicer | the one already in the manifest |

## 4. Nodes created in code

**Declare nodes in scenes. Never create a node in code when a scene could have declared
it.** "Created in code" means both constructing a node from Rust and instantiating a
packed scene at runtime.

The test: code creation is justified only when **what exists, how many exist, or when
they exist cannot be known while authoring the scene.**

Cases that pass:

- **Runtime-variable count:** projectiles, nearby enemies, other players, pickups, damage
  numbers, rows of a list built from data.
- **Streamed or procedurally generated:** world chunks, generated levels, content loaded
  on demand.
- **Decided by data unknown at authoring time:** a save file, a server message, a mod or
  other user content.
- **Lifetime independent of any authored parent:** a transient effect or one-shot sound
  that must outlive the thing that caused it, or a dialog opened on demand.
- **Pooling:** instances created ahead of time and reused, where the pool size is tuned
  or grows at runtime.
- **Swapping whole scenes:** loading the level or screen chosen at runtime.
- **Editor tooling:** a tool class that generates nodes for the scene author to keep.
- **Tests:** a test that builds the nodes it needs to exercise a class in isolation.

Not reasons:

- A fixed set of nodes known while authoring. That is a scene.
- It avoids editing a `.tscn`, or it was quicker to write.
- It avoids a node name or path stored in a scene. §3 prefers compiler-checked wiring
  between Rust and Rust; it does not license moving composition out of scenes.
- The values vary. Varying values are exported properties on a declared node.

| Don't | Do |
|---|---|
| build a health bar and its label in `ready` | declare both in the scene |
| create a timer in code because it is "an implementation detail" | declare the timer in the scene |
| create the same three spawn points in code on every run | place three markers in the scene |
| create a node in code so its color can come from data | declare the node; set the color through an exported property |
| assemble a bullet from a sprite and a collision shape in Rust | author a bullet scene; instantiate it when the weapon fires |
| declare fifty hidden bullets in a scene to avoid spawning any | spawn them; their number is not known while authoring |

### Justify it in the header

- Every node created in code is covered by the header of the file that creates it. The
  header says which kind of thing is created and which reason above, or another reason
  that passes the test, applies.
- Describe the nodes by role and reason, not by identifier, so the header still passes
  checklist items 2 to 4.
- If you cannot write a justification that passes the checklist, the node belongs in a
  scene.
- When the code creation is removed, remove its justification with it.
- If creation is a small side job of a large file, move it to a file whose purpose is the
  spawning. One file, one reason, one header.
- A file that defines a node type and also creates nodes has one header covering both
  (§2).

| Header | Verdict |
|---|---|
| `//! Spawns projectiles at runtime because their number is not known until play.` | passes |
| `//! Builds one row per saved game because the list comes from disk.` | passes |
| `//! Creates nodes in code.` | fails: no reason given |
| `//! Instantiates bullet.tscn in fire() and parents it to /root/World/Bullets.` | fails 3 and 4: names a file, a function and a path, and gives no reason |
| `//! Builds the HUD in code so the scene file stays small.` | not a reason: a fixed set of nodes belongs in a scene |

### When you do create nodes in code

- Instantiate a scene instead of assembling nodes one by one. The code decides when and
  how many; the scene still decides what.
- Hold the scene through a reference the editor tracks, such as an exported property,
  not a `res://` string in Rust. A path string is string-bound surface (§3).
- Decide who frees each instance at the moment you create it.
- If generated nodes must be saved with a scene, set `owner` on each of them.

## 5. Exported properties

**Every value on a custom node that a scene author could meaningfully set before the game
runs is an exported property.** That covers fields, and it covers tuning constants and
literals: a hard-coded number that shapes behavior is an unexported variable in disguise.

The test: **could someone placing this node in a scene want a different value, and would
that value mean something before the game starts?** If yes, export it.

- Give each export a default that makes the node work untouched. A reference has no
  default, so a missing one is reported through a configuration warning (§6).
- An export's name, type and default become string-bound surface (§3) as soon as a scene
  uses them. That is the accepted cost of this rule. Choose them carefully the first time.
- Turning a private value into an export whose default is the old value is an additive
  change (§3).
- Exported values arrive after construction. Read them in `ready`, not before (§6).

### Linking exports

A value that depends on another value is not a reason to hide it.

- A setter on one exported property can clamp it, or update another exported property,
  whenever it changes.
- For the link to hold while someone edits in the Inspector, and not only at run time,
  the class has to run in the editor.
- Link two values only when an author may legitimately set either one. If one is always
  computed from the other, do not store it at all.

### What stays unexported

Values that fail the test, and so stay private:

- **Run-time state:** what the node is doing right now, such as its current velocity, the
  time since it last fired, its present target or the state of a state machine. A
  *starting* value that a designer could choose is still an export.
- **No Inspector form:** a closure, a channel, a thread handle, random-number state, a
  collection of plain Rust values. If the data is authored, make it a resource and
  export the resource.
- **A cache computed from exports:** a stored copy can disagree with its source.
- **Handed over at run time:** a target given by whoever spawned the node, a network
  peer, a session.
- **Secrets:** exported values are written into scene files as plain text.
- **A fact, not a choice:** a constant the logic is only correct for, such as how many
  neighbors a hexagonal cell has, or a file format version.

The handle to its own base that the binding requires on every node is exempt and needs
no justification.

Not reasons:

- "It must stay consistent with another value." Link them.
- "It only makes sense within a range." Export it with that range, or clamp it in its
  setter.
- "Nobody will tune it." The default covers everyone who does not.
- "It clutters the Inspector." Group the properties.
- "It is a reference to a child node." Export the reference; the editor then keeps it
  correct when the node is renamed or moved.
- "It is structured data." Make it a resource.
- "A constant is simpler," or "it adds string-bound surface."

| Don't | Do |
|---|---|
| a `const` jump speed in a node's file | an exported jump speed whose default is that value |
| a literal `0.25` for a fade time inside a method | an exported fade time |
| a private handle to a child, filled by a path lookup in `ready` | an exported node reference, assigned in the scene |
| keep a maximum private because it must never drop below a minimum | export both; each setter clamps against the other |
| export a radius and a diameter with nothing tying them together | link them, or export one and compute the other |
| leave a chance unexported because it only makes sense from zero to one | export it with that range |
| a loot table built as a private list in code | a resource holding the entries, exported |
| twenty exports in one flat list | the same exports, grouped |
| export the node's current velocity | keep it private; cover that kind of state in the header |
| export a cached square of the radius beside the radius | compute it from the radius |
| export an access token so each scene can set it | never; scene files are plain text |

### Justify it in the header

- Every unexported value on a custom node, apart from the exempt handle, is covered by
  the "not exported" part of that node's header (§2).
- Justify by **kind of state**, not field by field, and describe each kind by role. One
  sentence covers every field of that kind, so another field of a covered kind needs no
  header edit. A new kind does.
- If you cannot write a justification that passes the checklist, export the value.
- This applies to values you add or change. Do not sweep through node types you are not
  otherwise changing.
- Custom resources are outside this rule. Their fields exist to be authored, so exporting
  them is already the norm, and they need no header for it.

| The "not exported" part of a node header | Verdict |
|---|---|
| `//! Not exported: what it is chasing right now, which only means something while the game runs.` | passes |
| `//! Not exported: the connection it is handed at run time, which no scene author can supply.` | passes |
| ``//! Not exported: `velocity`, `cooldown` and `target`.`` | fails 4: names fields, and gives no reason |
| `//! Not exported: its tuning values, to keep the Inspector tidy.` | not a reason: group them |
| `//! Not exported: its speed limit, because it must match the animation.` | not a reason: link them |

## 6. Engine rules

Hook names below are the Rust virtuals; they map to Godot's underscore-prefixed ones.

**Pick the cheapest kind of object that fits.**

| Need | Use |
|---|---|
| data and logic the engine never sees | a plain Rust type |
| data the engine must hold or pass around | a `RefCounted` class |
| data saved to a file or edited in the Inspector | a `Resource` class |
| something in the tree, with a transform or engine callbacks | a `Node` class |

- Prefer many plain values owned by one node over one node per datum.
- A new node type justifies being a node, and being a new type, in its file header (§2).
- Nodes and other non-reference-counted objects are freed by hand. A node you create and
  never add to the tree is yours to free. A handle to a freed object is invalid.

| Don't | Do |
|---|---|
| one node per inventory item | one inventory node owning a list of plain item values |
| a node type that only holds a damage formula | a plain function |
| a resource for state that is never saved or edited in the Inspector | a plain Rust type |
| create a node, never add it to the tree, and drop the handle | free it, or do not create it |

**Scenes declare, Rust decides.**

- A scene holds composition, property values and connections. Its root node's class and
  the scene are one unit; read them together.
- Nodes are declared in scenes, not created in code, except as §4 allows.
- Edit `.tscn` text minimally and in the shape the editor writes. Keep resource ids
  consistent within the file. Never invent a UID; omit it and let the editor assign one.

| Don't | Do |
|---|---|
| change a root node's Rust class without opening its scene | read both, then change |
| reorder or reformat a `.tscn` while changing one property | change the one line |
| hand-write a `uid="uid://..."` you did not read from the resource it names | omit it |
| attach a `.gd` script "just for this one callback" | write the callback in Rust |

**Shape the tree by ownership, not by position.**

- Parent and child means "part of". Test: if the parent is removed, should the child go
  too? If not, they are siblings.
- A scene should work when instantiated alone. What it needs is injected from above, never
  reached for upward or sideways.
- Calls go down, signals go up. A signal reports a past-tense fact (`died`,
  `item_collected`); it is never a command.
- Siblings do not talk to each other. Their common ancestor connects them.
- A node that needs setup it cannot do itself reports it through a configuration warning
  worded as an instruction.
- An autoload is the last resort: only for a system that owns all its own data, must be
  reachable everywhere, and works in isolation.

| Don't | Do |
|---|---|
| bullets parented to the gun that fired them | bullets parented to something that outlives the gun |
| a child calls up to its parent and casts it to a specific type | the parent hands the child what it needs, or the child emits a signal |
| an enemy reaches for a sibling to find its target | whoever spawns the enemy gives it the target |
| the HUD finds the player through an absolute path from the root | their common ancestor connects the player's signal to the HUD |
| a signal named `open_door` | a signal named `lever_pulled`; the owner of both decides the door opens |
| a node quietly does nothing when its target is unassigned | a configuration warning: "Assign a target." |
| an autoload that adjusts the player, the enemies and the UI | an autoload that plays music and owns only its own state |

**Use the right hook.**

- Construction runs first, exported values are applied after it, then `ready`. Never read
  an exported value during construction.
- `ready` runs after all children are ready. Assume nothing about sibling order.
- Set properties on a node before adding it to the tree, except values that only exist in
  the tree, such as global position.
- `process` for per-frame visuals, `physics_process` for movement and physics, a `Timer`
  for anything that does not need every frame, and the input callbacks for input. Do not
  poll input in `process`.

| Don't | Do |
|---|---|
| set current health from the exported maximum during construction | set it in `ready` |
| a node's `ready` relying on a sibling having already run its own | the parent, whose `ready` runs after both, wires them together |
| add a child, then set its properties | set its properties, then add it |
| re-scan for the nearest target every frame | a timer that re-scans a few times a second |
| check every frame whether pause was pressed | handle it in an input callback |

**Cross the engine boundary carefully.**

- Hold a node through an exported reference wherever a scene can supply it (§5).
  Otherwise look it up once and keep the handle. No path lookups per frame.
- Anything a scene supplies can be missing: an unassigned exported reference, a child
  that was renamed. Handle the absent case; do not `unwrap` it.
- A panic is a bug, never control flow.
- Engine calls can re-enter your code: emitting a signal, adding a child, anything that
  fires a callback. Do not hold a borrow of any Rust object across such a call if the
  callback can reach that object. The usual victim is the object whose method is running.
- Use Rust collections and types in inner loops. Convert to and from engine types at the
  boundary, not per element per frame.

| Don't | Do |
|---|---|
| look a child up by path inside `process` | take it as an exported reference, or look it up once in `ready` and keep the handle |
| `unwrap` an exported reference the scene may not have assigned | handle the missing case: warn the scene author and return |
| panic to say "no target found" | return `Option` |
| emit a signal partway through changing state its listeners will read back | finish the change, then emit |
| push values into an engine array one call at a time, every frame | work in a `Vec`; convert once at the boundary |

**Keep the project tidy.**

- `snake_case` for files and folders, `PascalCase` for node names. Exports are
  case-sensitive even where the development filesystem is not.
- Keep assets beside the scenes that use them, and genuinely shared ones in one place.
- The engine's cache folder and the Rust build output are never committed. Do not add
  other ignore rules without checking what the project already has.

| Don't | Do |
|---|---|
| `PlayerController.tscn`, `Enemy Sprites/` | `player_controller.tscn`, `enemy_sprites/` |
| a node named `health_bar` | a node named `HealthBar` |
| every texture in one top-level folder | each texture beside the scene that uses it |
| commit the engine cache or the build output | leave them ignored |

## 7. Before you finish

- [ ] No comments were added outside a file header, and every header passes the checklist.
- [ ] No comment was left behind that this change made false.
- [ ] Every new custom node type has a header that answers each question that applies
      and names nothing that expires.
- [ ] Every value on a custom node that a scene author could set is exported, tuning
      constants and literals included, and each kind of unexported state is justified in
      the header.
- [ ] No node is created in code that a scene could have declared, and every node that is
      created in code is justified in its file's header.
- [ ] Dependents were counted, in Rust and in scenes, for every shared thing touched.
- [ ] Every renamed, removed or retyped name was searched for in scene and resource files.
- [ ] No exported default or enum order changed by accident.
- [ ] Behavior sits in the smallest module or scene that can own it.
- [ ] The build passes, and the affected scenes were loaded in the engine. If you could
      not run the engine, the report says so instead of claiming the scenes work.
- [ ] Every deviation from this skill is reported with its justification.
