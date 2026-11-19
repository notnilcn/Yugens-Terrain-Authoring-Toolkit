// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Khvor' Gates contributors
//
//! `PcamRecorder` — the parity harness observer (the port source repository's parity harness).
//!
//! No oracle counterpart: the upstream addon has no recording node, so this file
//! ports nothing. It is the *measurement* half of the parity harness, deliberately
//! kept out of `pcam2d`/`pcam3d`/`host` so it can observe **either** backend —
//! the GDScript oracle installed by that harness or the Rust
//! classes in this crate. Every Phantom Camera access is therefore duck-typed
//! through `Object::try_call()`; this module must never import `crate::pcam2d`,
//! `crate::pcam3d` or `crate::host` (they are written by other owners), and it
//! must work when those classes are not registered at all.
//!
//! Oracle anchors for the state sampled here (the frame pipeline whose observable
//! behaviour is the thing being compared):
//! - `scripts/phantom_camera_host/phantom_camera_host.gd:850-903` —
//!   `_process` / `_tween_follow_checker`: `process_logic(delta)`,
//!   `get_transform_output()`, the follow/tween writes into the real camera and
//!   the 2D noise offset. The camera transform recorded on frame *f* is the result
//!   of that frame's host pump (the host sits before the recorder in tree order).
//! - `scripts/phantom_camera_host/phantom_camera_host.gd:970-1218` —
//!   `_pcam_tween`: the `tween_started` / `is_tweening` / `tween_completed` emit
//!   order captured by the signal stream.
//! - `scripts/phantom_camera/phantom_camera_2d.gd:1312-1318,1366,1434,1931` —
//!   `get_transform_output()`, `get_noise_transform()`, `get_priority()`,
//!   `is_active()`, `has_noise_resource()`: the pcam fields the recorder samples.
//!
//! # Why `try_call` and not `get_class()`
//!
//! The oracle's `phantom_camera_2d.gd:1961` overrides `get_class()` so the
//! GDScript node reports `"PhantomCamera2D"`. A *ptrcall* (`Gd::get_class()`)
//! bypasses GDScript's method lookup and would answer `"Node2D"`; only a varcall
//! (`try_call("get_class")`) honours the script override. The recorder uses the
//! varcall everywhere so both backends answer identically — that is what makes
//! `require_class` a real proof of *which* implementation ran.
//!
//! # Output schema (`pcam-record/v1`, one JSON object per line)
//!
//! Line 1 is the header:
//!
//! ```json
//! {"kind":"header","schema":"pcam-record/v1","observer":"rust",
//!  "godot":"4.7.2.stable.official.ed1daf0b","godot_hex":262918,"max_fps":60,
//!  "dims":2,"camera_class":"Camera2D","camera_node":"/root/Harness/Demo/Camera2D",
//!  "host_class":"PhantomCameraHost","host_node":"/root/Harness/Demo/Camera2D/Host",
//!  "pcam_class":"PhantomCamera2D","pcam_node":"/root/Harness/Demo/Camera2D/Player",
//!  "require_class":"PhantomCamera2D","frames":240,
//!  "out":"C:/.../pcam/2d_example_scene.jsonl"}
//! ```
//!
//! Every following line is one frame, with a fixed key order and fixed
//! 9-decimal precision (so two runs diff byte-for-byte, not just numerically):
//!
//! ```json
//! {"f":0,"d":0.016666667,"pos":[x,y],"rot":[r],"scale_zoom":[zx,zy],
//!  "pcam":{"active":true,"priority":10,"transform":[6 floats],
//!          "noise":[6 floats],"has_noise":false},
//!  "signals":["became_active@3"]}
//! ```
//!
//! Per dimension, to keep the static type of each array fixed:
//!
//! | field | 2D | 3D |
//! |---|---|---|
//! | `pos` | `[x, y]` camera global transform origin | `[x, y, z]` |
//! | `rot` | `[radians]` (x-axis angle of the global transform) | `[qx, qy, qz, qw]` |
//! | `scale_zoom` | `[zoom.x, zoom.y]` | `[fov, size, near, far, h_offset, v_offset]` |
//! | `pcam.transform` | `Transform2D` → 6 floats | `Transform3D` → 12 floats |
//! | `pcam.noise` | `get_noise_transform()` → 6 floats | 12 floats, `null` when absent |
//!
//! `signals` are `name@frame` strings for the eight tapped signals, buffered and
//! flushed with the next line, so the stream is exactly reproducible.
//!
//! A failed run (no camera, unopenable file, `require_class` mismatch) prints
//! `PCAMREC: ...` through `godot_error!` and exits with code `2`; a completed run
//! prints `PCAMREC: wrote {n} frames to {path}` and exits `0`.

use std::cell::{Cell, RefCell};
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::rc::Rc;

use godot::classes::{Camera2D, Camera3D, Engine, ProjectSettings};
use godot::prelude::*;

/// Schema tag written into the header line; `tools/pcam_compare.py` refuses a
/// stream it does not know.
pub const SCHEMA: &str = "pcam-record/v1";

/// Signals tapped on the active Phantom Camera. `has_signal()` gates each one, so
/// a partially ported backend simply produces a shorter stream instead of an
/// error. Order here only affects connection order, not the recorded stream.
const TAPPED_SIGNALS: [&str; 8] = [
    "became_active",
    "became_inactive",
    "tween_started",
    "is_tweening",
    "tween_completed",
    "dead_zone_changed",
    "dead_zone_reached",
    "noise_emitted",
];

/// Decimals kept for every recorded float (fixed precision ⇒ byte-stable JSONL).
const PRECISION: usize = 9;

/// One frame of camera + Phantom Camera telemetry.
#[derive(GodotClass)]
#[class(base=Node, tool)]
pub struct PcamRecorder {
    base: Base<Node>,

    /// JSONL destination. `user://` and `res://` are globalized through
    /// `ProjectSettings`; an OS path is used verbatim.
    #[export]
    out_path: GString,

    /// Frames to record, then quit with code 0. `0` records until killed.
    #[export]
    frames: i64,

    /// Camera to observe (`Camera2D` or `Camera3D`). Empty = first camera found
    /// under the current scene root.
    #[export]
    camera_path: NodePath,

    /// `PhantomCameraHost` to poll for the active Phantom Camera. Empty = first
    /// node in the tree exposing `get_active_pcam()`.
    #[export]
    host_path: NodePath,

    /// Class name the active Phantom Camera must report (`PhantomCamera2D` /
    /// `PhantomCamera3D`). This is how the harness proves *which* backend ran: the
    /// GDScript oracle and the Rust classes both answer through `get_class()`.
    #[export]
    require_class: GString,

    /// Frame index shared with the signal sinks so `name@frame` is stamped with
    /// the frame the signal belongs to.
    frame: Rc<Cell<i64>>,
    /// Signals received since the last written line.
    emitted: Rc<RefCell<Vec<String>>>,

    out: Option<File>,
    /// Globalized destination, echoed in the header and the log line.
    out_display: String,
    camera: Option<Gd<Node>>,
    /// `2` or `3`; fixed once the camera is resolved.
    dims: u8,
    host: Option<Gd<Node>>,
    /// Phantom Camera currently reported as active.
    pcam: Option<Gd<Node>>,
    /// Phantom Camera the signal sinks are attached to. Kept apart from `pcam` so a
    /// switch can disconnect the *previous* node after `pcam` already points at the
    /// new one.
    linked: Option<Gd<Node>>,
    /// `(signal, callable)` pairs connected to `linked`, for a clean reconnect.
    links: Vec<(StringName, Callable)>,
    written: i64,
    done: bool,
}

#[godot_api]
impl INode for PcamRecorder {
    fn init(base: Base<Node>) -> Self {
        Self {
            base,
            out_path: GString::from("user://pcam_record.jsonl"),
            frames: 240,
            camera_path: NodePath::from(""),
            host_path: NodePath::from(""),
            require_class: GString::from(""),
            frame: Rc::new(Cell::new(0)),
            emitted: Rc::new(RefCell::new(Vec::new())),
            out: None,
            out_display: String::new(),
            camera: None,
            dims: 0,
            host: None,
            pcam: None,
            linked: None,
            links: Vec::new(),
            written: 0,
            done: false,
        }
    }

    fn ready(&mut self) {
        // A `tool` node also runs in the editor, where recording a scene open in
        // the docks would measure editor gizmo state instead of a game frame.
        if Engine::singleton().is_editor_hint() {
            self.done = true;
            godot_warn!("PCAMREC: editor hint set, not recording (run the scene headless)");
            return;
        }

        if let Err(message) = self.start() {
            self.abort(&message);
        }
    }

    fn process(&mut self, delta: f64) {
        if self.done || Engine::singleton().is_editor_hint() {
            return;
        }

        if let Err(message) = self.tick(delta) {
            self.abort(&message);
        }
    }
}

impl PcamRecorder {
    /// Resolve the observed nodes, open the output and write the header line.
    fn start(&mut self) -> Result<(), String> {
        self.open_output()?;

        let (camera, dims) = self.resolve_camera()?;
        self.camera = Some(camera);
        self.dims = dims;
        self.host = self.resolve_host();
        self.pcam = self.host.as_ref().and_then(active_pcam_of);

        self.check_require_class()?;
        self.bind_pcam();

        let header = self.header_line();
        self.write_line(&header)
    }

    /// One frame: follow an active-camera switch, then write the observation.
    fn tick(&mut self, delta: f64) -> Result<(), String> {
        let active = self.host.as_ref().and_then(active_pcam_of);
        let changed = match (&self.pcam, &active) {
            (Some(previous), Some(next)) => previous.instance_id() != next.instance_id(),
            (None, None) => false,
            _ => true,
        };

        if changed {
            self.pcam = active;
            self.check_require_class()?;
            self.bind_pcam();
        }

        let line = self.frame_line(delta);
        self.write_line(&line)?;
        // Signals are flushed with the line they were stamped on, exactly like
        // `_pending.clear()` in the baseline observer; anything emitted later in
        // this frame lands on the next line.
        self.emitted.borrow_mut().clear();

        self.frame.set(self.frame.get() + 1);
        self.written += 1;

        if self.frames > 0 && self.written >= self.frames {
            self.finish();
        }

        Ok(())
    }

    /// Globalize the destination, create its directory and truncate the file.
    fn open_output(&mut self) -> Result<(), String> {
        let global = ProjectSettings::singleton()
            .globalize_path(&self.out_path)
            .to_string();
        let path = Path::new(&global);

        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "cannot create output directory {}: {error}",
                    parent.display()
                )
            })?;
        }

        let file = File::create(path)
            .map_err(|error| format!("cannot open output file {global}: {error}"))?;

        self.out = Some(file);
        self.out_display = global;
        Ok(())
    }

    /// Camera from `camera_path`, else the first `Camera2D`/`Camera3D` in the
    /// current scene (tree order), with its dimension.
    fn resolve_camera(&self) -> Result<(Gd<Node>, u8), String> {
        if !self.camera_path.is_empty() {
            let node = self
                .base()
                .get_node_or_null(&self.camera_path)
                .ok_or_else(|| format!("camera_path '{}' does not resolve", self.camera_path))?;
            let dims = camera_dims(&node).ok_or_else(|| {
                format!(
                    "camera_path '{}' is a {}, not a Camera2D/Camera3D",
                    self.camera_path,
                    varcall_class(&node).unwrap_or_else(|| "?".to_string())
                )
            })?;
            return Ok((node, dims));
        }

        let root = self.scene_root()?;
        for node in descendants(&root) {
            if let Some(dims) = camera_dims(&node) {
                return Ok((node, dims));
            }
        }

        Err("no Camera2D/Camera3D under the scene root".to_string())
    }

    /// Host from `host_path`, else the first node exposing `get_active_pcam()`.
    fn resolve_host(&self) -> Option<Gd<Node>> {
        if !self.host_path.is_empty() {
            return self.base().get_node_or_null(&self.host_path);
        }

        let root = self.scene_root().ok()?;
        descendants(&root)
            .into_iter()
            .find(|node| node.has_method("get_active_pcam"))
    }

    /// Enforce `require_class` against the active Phantom Camera (and, when none
    /// is active yet, against any matching node in the tree). A crate that has not
    /// registered its classes yields "no node of class ...", which the shell driver
    /// reports as `SKIP (classes not registered)` rather than a mismatch.
    fn check_require_class(&self) -> Result<(), String> {
        if self.require_class.is_empty() {
            return Ok(());
        }

        let wanted = self.require_class.to_string();

        if let Some(pcam) = self.pcam.as_ref() {
            let found = varcall_class(pcam).unwrap_or_else(|| "?".to_string());
            if found != wanted {
                return Err(format!(
                    "require_class={wanted} but the active Phantom Camera is '{found}'"
                ));
            }
            return Ok(());
        }

        let root = self.scene_root()?;
        let found = descendants(&root)
            .into_iter()
            .any(|node| varcall_class(&node).is_some_and(|class| class == wanted));

        if found {
            // Present but not active yet: the host may still switch to it, and the
            // frames before the switch record `pcam: null` on both sides.
            return Ok(());
        }

        Err(format!(
            "no node of class '{wanted}' in the scene tree (classes not registered?)"
        ))
    }

    /// (Re)connect the signal sinks to the current Phantom Camera.
    fn bind_pcam(&mut self) {
        self.unbind_pcam();

        let Some(pcam) = self.pcam.clone() else {
            return;
        };

        for name in TAPPED_SIGNALS {
            if !pcam.has_signal(name) {
                continue;
            }

            let emitted = Rc::clone(&self.emitted);
            let frame = Rc::clone(&self.frame);
            let signal = name.to_string();
            let sink = Callable::from_fn("pcam_recorder_sink", move |_args: &[&Variant]| {
                let stamp = format!("{signal}@{}", frame.get());
                emitted.borrow_mut().push(stamp);
                Variant::nil()
            });

            let mut target = pcam.clone();
            target.connect(name, &sink);
            self.links.push((StringName::from(name), sink));
        }

        self.linked = Some(pcam);
    }

    /// Disconnect every sink attached to the previously bound Phantom Camera.
    fn unbind_pcam(&mut self) {
        let Some(mut pcam) = self.linked.take() else {
            self.links.clear();
            return;
        };

        for (name, sink) in std::mem::take(&mut self.links) {
            if pcam.is_instance_valid() {
                pcam.disconnect(&name, &sink);
            }
        }
    }

    /// The scene root the camera/host are searched under: the scene the runner
    /// started (`Harness` in the generated scene), else the top of this node's tree.
    fn scene_root(&self) -> Result<Gd<Node>, String> {
        let tree = self.base().get_tree();

        if let Some(current) = tree.get_current_scene() {
            return Ok(current);
        }

        let mut node = self.to_gd().upcast::<Node>();
        while let Some(parent) = node.get_parent() {
            node = parent;
        }
        Ok(node)
    }

    /// First node in the scene reporting the `require_class` name, used for the
    /// header before the host has activated anything.
    fn fallback_pcam(&self) -> Option<Gd<Node>> {
        if self.require_class.is_empty() {
            return None;
        }
        let wanted = self.require_class.to_string();
        let root = self.scene_root().ok()?;
        descendants(&root)
            .into_iter()
            .find(|node| varcall_class(node).is_some_and(|class| class == wanted))
    }

    /// Header line: the provenance the comparer reports when a scene diverges.
    fn header_line(&self) -> String {
        let engine = Engine::singleton();
        let version = engine.get_version_info();
        let version_string = version
            .get(&Variant::from("string"))
            .and_then(|value| value.try_to::<GString>().ok())
            .map_or_else(|| "unknown".to_string(), |value| value.to_string());
        let version_hex = version
            .get(&Variant::from("hex"))
            .and_then(|value| value.try_to::<i64>().ok())
            .unwrap_or_default();

        let camera_class = self
            .camera
            .as_ref()
            .and_then(varcall_class)
            .unwrap_or_else(|| "?".to_string());
        let camera_node = self
            .camera
            .as_ref()
            .map_or_else(String::new, |node| node.get_path().to_string());
        let host_class = self
            .host
            .as_ref()
            .and_then(varcall_class)
            .unwrap_or_else(|| "?".to_string());
        let host_node = self
            .host
            .as_ref()
            .map_or_else(String::new, |node| node.get_path().to_string());
        // The host usually activates its first Phantom Camera on its first frame,
        // after this node's `ready()`. Fall back to the node that satisfies
        // `require_class` so the header names the same class on both legs.
        let pcam = self.pcam.clone().or_else(|| self.fallback_pcam());
        let pcam_class = pcam
            .as_ref()
            .and_then(varcall_class)
            .unwrap_or_else(|| "?".to_string());
        let pcam_node = pcam
            .as_ref()
            .map_or_else(String::new, |node| node.get_path().to_string());

        format!(
            concat!(
                r#"{{"kind":"header","schema":"{}","observer":"rust","godot":"{}","#,
                r#""godot_hex":{},"max_fps":{},"dims":{},"camera_class":"{}","#,
                r#""camera_node":"{}","host_class":"{}","host_node":"{}","#,
                r#""pcam_class":"{}","pcam_node":"{}","require_class":"{}","#,
                r#""frames":{},"out":"{}"}}"#
            ),
            json_escape(SCHEMA),
            json_escape(&version_string),
            version_hex,
            engine.get_max_fps(),
            self.dims,
            json_escape(&camera_class),
            json_escape(&camera_node),
            json_escape(&host_class),
            json_escape(&host_node),
            json_escape(&pcam_class),
            json_escape(&pcam_node),
            json_escape(&self.require_class.to_string()),
            self.frames,
            json_escape(&self.out_display),
        )
    }

    /// One frame line, fixed key order: `f, d, pos, rot, scale_zoom, pcam, signals`.
    fn frame_line(&self, delta: f64) -> String {
        let (position, rotation, scale_zoom) = self.camera_telemetry();

        let mut pcam = String::from("null");
        if let Some(node) = self.pcam.as_ref() {
            pcam = self.pcam_line(node);
        }

        format!(
            concat!(
                r#"{{"f":{},"d":{},"pos":{},"rot":{},"scale_zoom":{},"#,
                r#""pcam":{},"signals":{}}}"#
            ),
            self.frame.get(),
            json_float(delta),
            json_floats(&position),
            json_floats(&rotation),
            json_floats(&scale_zoom),
            pcam,
            json_strings(&self.emitted.borrow()),
        )
    }

    /// Camera telemetry for the active dimension (see the module table).
    fn camera_telemetry(&self) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
        let Some(camera) = self.camera.as_ref() else {
            return (Vec::new(), Vec::new(), Vec::new());
        };

        if self.dims == 3 {
            let Some(camera_3d) = camera.clone().try_cast::<Camera3D>().ok() else {
                return (Vec::new(), Vec::new(), Vec::new());
            };
            let transform = camera_3d.get_global_transform();
            let rotation = transform.basis.get_quaternion();
            let position = vec![
                transform.origin.x as f64,
                transform.origin.y as f64,
                transform.origin.z as f64,
            ];
            let orientation = vec![
                rotation.x as f64,
                rotation.y as f64,
                rotation.z as f64,
                rotation.w as f64,
            ];
            let scale_zoom = vec![
                camera_3d.get_fov() as f64,
                camera_3d.get_size() as f64,
                camera_3d.get_near() as f64,
                camera_3d.get_far() as f64,
                camera_3d.get_h_offset() as f64,
                camera_3d.get_v_offset() as f64,
            ];
            return (position, orientation, scale_zoom);
        }

        let Some(camera_2d) = camera.clone().try_cast::<Camera2D>().ok() else {
            return (Vec::new(), Vec::new(), Vec::new());
        };
        let transform = camera_2d.get_global_transform();
        let rotation = transform.a.y.atan2(transform.a.x);
        let zoom = camera_2d.get_zoom();
        (
            vec![transform.origin.x as f64, transform.origin.y as f64],
            vec![rotation as f64],
            vec![zoom.x as f64, zoom.y as f64],
        )
    }

    /// The `pcam` object of a frame line, duck-typed through `try_call()`.
    fn pcam_line(&self, pcam: &Gd<Node>) -> String {
        let active = call_bool(pcam, "is_active");
        let priority = call_int(pcam, "get_priority");
        let transform = call_transform(pcam, "get_transform_output");
        let noise = call_transform(pcam, "get_noise_transform");
        let has_noise = call_bool(pcam, "has_noise_resource");

        format!(
            concat!(
                r#"{{"active":{},"priority":{},"transform":{},"#,
                r#""noise":{},"has_noise":{}}}"#
            ),
            json_optional_bool(active),
            json_optional_int(priority),
            json_optional_floats(transform.as_deref()),
            json_optional_floats(noise.as_deref()),
            json_optional_bool(has_noise),
        )
    }

    /// Append one line to the output and flush it, so a crash cannot lose frames.
    fn write_line(&mut self, line: &str) -> Result<(), String> {
        let path = self.out_display.clone();
        let Some(file) = self.out.as_mut() else {
            return Err(format!("output file {path} is closed"));
        };

        writeln!(file, "{line}").map_err(|error| format!("cannot write {path}: {error}"))?;
        file.flush()
            .map_err(|error| format!("cannot flush {path}: {error}"))?;
        Ok(())
    }

    /// Close the stream, report the frame count and quit with `0`.
    fn finish(&mut self) {
        self.done = true;
        self.unbind_pcam();
        self.out = None;
        godot_print!(
            "PCAMREC: wrote {} frames to {}",
            self.written,
            self.out_display
        );

        let mut tree = self.base().get_tree();
        tree.quit();
    }

    /// Report a fatal recorder error and quit with code `2` (harness exit contract).
    fn abort(&mut self, message: &str) {
        if self.done {
            return;
        }
        self.done = true;
        self.unbind_pcam();
        self.out = None;
        godot_error!("PCAMREC: {message}");

        let mut tree = self.base().get_tree();
        tree.quit_ex().exit_code(2).done();
    }
}

/// `2` for a `Camera2D`, `3` for a `Camera3D`, `None` for anything else.
fn camera_dims(node: &Gd<Node>) -> Option<u8> {
    if node.clone().try_cast::<Camera3D>().is_ok() {
        return Some(3);
    }
    if node.clone().try_cast::<Camera2D>().is_ok() {
        return Some(2);
    }
    None
}

/// Depth-first node list below (and excluding) `root`, in tree order.
fn descendants(root: &Gd<Node>) -> Vec<Gd<Node>> {
    let mut found = Vec::new();
    collect_descendants(root, &mut found);
    found
}

/// Depth-first pre-order walk; scene trees are shallow, so recursion is fine.
fn collect_descendants(node: &Gd<Node>, found: &mut Vec<Gd<Node>>) {
    for child in node.get_children().iter_shared() {
        found.push(child.clone());
        collect_descendants(&child, found);
    }
}

/// `get_class()` through a *varcall*, so a GDScript override is honoured (see the
/// module docs). `None` when the object is gone or does not answer with a string.
fn varcall_class(node: &Gd<Node>) -> Option<String> {
    let mut node = node.clone();
    let value = node.try_call("get_class", &[]).ok()?;
    value
        .try_to::<GString>()
        .ok()
        .map(|class| class.to_string())
}

/// `host.get_active_pcam()`; `None` when absent, null or already freed.
fn active_pcam_of(host: &Gd<Node>) -> Option<Gd<Node>> {
    let mut host = host.clone();
    let value = host.try_call("get_active_pcam", &[]).ok()?;
    let node = value.try_to::<Option<Gd<Node>>>().ok()??;

    if node.is_instance_valid() {
        Some(node)
    } else {
        None
    }
}

/// Call a no-argument predicate; `None` when the method is missing or throws.
fn call_bool(node: &Gd<Node>, method: &str) -> Option<bool> {
    if !node.has_method(method) {
        return None;
    }
    let mut node = node.clone();
    node.try_call(method, &[]).ok()?.try_to::<bool>().ok()
}

/// Call a no-argument integer getter; `None` when the method is missing.
fn call_int(node: &Gd<Node>, method: &str) -> Option<i64> {
    if !node.has_method(method) {
        return None;
    }
    let mut node = node.clone();
    node.try_call(method, &[]).ok()?.try_to::<i64>().ok()
}

/// Call a no-argument `Transform2D`/`Transform3D` getter and flatten it to floats
/// (6 for 2D, 12 for 3D); `None` when the method is missing or returns another type.
fn call_transform(node: &Gd<Node>, method: &str) -> Option<Vec<f64>> {
    if !node.has_method(method) {
        return None;
    }
    let mut node = node.clone();
    let value = node.try_call(method, &[]).ok()?;

    if let Ok(transform) = value.try_to::<Transform2D>() {
        return Some(vec![
            transform.a.x as f64,
            transform.a.y as f64,
            transform.b.x as f64,
            transform.b.y as f64,
            transform.origin.x as f64,
            transform.origin.y as f64,
        ]);
    }

    if let Ok(transform) = value.try_to::<Transform3D>() {
        let a = transform.basis.col_a();
        let b = transform.basis.col_b();
        let c = transform.basis.col_c();
        return Some(vec![
            a.x as f64,
            a.y as f64,
            a.z as f64,
            b.x as f64,
            b.y as f64,
            b.z as f64,
            c.x as f64,
            c.y as f64,
            c.z as f64,
            transform.origin.x as f64,
            transform.origin.y as f64,
            transform.origin.z as f64,
        ]);
    }

    None
}

/// A float with fixed precision; non-finite values become JSON `null` so the
/// comparer can report them instead of failing to parse the line.
fn json_float(value: f64) -> String {
    if value.is_finite() {
        format!("{:.*}", PRECISION, value)
    } else {
        "null".to_string()
    }
}

/// `[a,b,...]` with fixed precision.
fn json_floats(values: &[f64]) -> String {
    let mut out = String::with_capacity(values.len() * (PRECISION + 4) + 2);
    out.push('[');
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str(&json_float(*value));
    }
    out.push(']');
    out
}

/// `[...]`, or `null` when the getter was missing.
fn json_optional_floats(values: Option<&[f64]>) -> String {
    values.map_or_else(|| "null".to_string(), json_floats)
}

/// `true`/`false`, or `null` when the getter was missing.
fn json_optional_bool(value: Option<bool>) -> String {
    value.map_or_else(|| "null".to_string(), |flag| flag.to_string())
}

/// An integer, or `null` when the getter was missing.
fn json_optional_int(value: Option<i64>) -> String {
    value.map_or_else(|| "null".to_string(), |number| number.to_string())
}

/// `["a","b"]` with JSON escaping.
fn json_strings(values: &[String]) -> String {
    let mut out = String::from("[");
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push('"');
        out.push_str(&json_escape(value));
        out.push('"');
    }
    out.push(']');
    out
}

/// Minimal JSON string escaping (paths and node paths only carry `\`, `"` and
/// control characters; everything else is passed through).
fn json_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            control if control.is_control() => {
                out.push_str(&format!("\\u{:04x}", u32::from(control)));
            }
            other => out.push(other),
        }
    }
    out
}
