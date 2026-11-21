use std::cell::{Cell, RefCell};
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::rc::Rc;
use godot::classes::{Camera2D, Camera3D, Engine, ProjectSettings};
use godot::prelude::*;
pub const SCHEMA: &str = "pcam-record/v1";
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
const PRECISION: usize = 9;
#[derive(GodotClass)]
#[class(base=Node, tool)]
pub struct PcamRecorder {
    base: Base<Node>,
    #[export]
    out_path: GString,
    #[export]
    frames: i64,
    #[export]
    camera_path: NodePath,
    #[export]
    host_path: NodePath,
    #[export]
    require_class: GString,
    frame: Rc<Cell<i64>>,
    emitted: Rc<RefCell<Vec<String>>>,
    out: Option<File>,
    out_display: String,
    camera: Option<Gd<Node>>,
    dims: u8,
    host: Option<Gd<Node>>,
    pcam: Option<Gd<Node>>,
    linked: Option<Gd<Node>>,
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
        self.emitted.borrow_mut().clear();
        self.frame.set(self.frame.get() + 1);
        self.written += 1;
        if self.frames > 0 && self.written >= self.frames {
            self.finish();
        }
        Ok(())
    }
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
    fn resolve_host(&self) -> Option<Gd<Node>> {
        if !self.host_path.is_empty() {
            return self.base().get_node_or_null(&self.host_path);
        }
        let root = self.scene_root().ok()?;
        descendants(&root)
            .into_iter()
            .find(|node| node.has_method("get_active_pcam"))
    }
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
            return Ok(());
        }
        Err(format!(
            "no node of class '{wanted}' in the scene tree (classes not registered?)"
        ))
    }
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
    fn write_line(&mut self, line: &str) -> Result<(), String> {
        let path = self.out_display.clone();
        let Some(file) = self.out.as_mut() else {
            return Err(format!("output file {path} is closed"));
        };
        writeln!(file, "{line}").map_err(|error| format!("cannot persist {path}: {error}"))?;
        file.flush()
            .map_err(|error| format!("cannot flush {path}: {error}"))?;
        Ok(())
    }
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
fn camera_dims(node: &Gd<Node>) -> Option<u8> {
    if node.clone().try_cast::<Camera3D>().is_ok() {
        return Some(3);
    }
    if node.clone().try_cast::<Camera2D>().is_ok() {
        return Some(2);
    }
    None
}
fn descendants(root: &Gd<Node>) -> Vec<Gd<Node>> {
    let mut found = Vec::new();
    collect_descendants(root, &mut found);
    found
}
fn collect_descendants(node: &Gd<Node>, found: &mut Vec<Gd<Node>>) {
    for child in node.get_children().iter_shared() {
        found.push(child.clone());
        collect_descendants(&child, found);
    }
}
fn varcall_class(node: &Gd<Node>) -> Option<String> {
    let mut node = node.clone();
    let value = node.try_call("get_class", &[]).ok()?;
    value
        .try_to::<GString>()
        .ok()
        .map(|class| class.to_string())
}
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
fn call_bool(node: &Gd<Node>, method: &str) -> Option<bool> {
    if !node.has_method(method) {
        return None;
    }
    let mut node = node.clone();
    node.try_call(method, &[]).ok()?.try_to::<bool>().ok()
}
fn call_int(node: &Gd<Node>, method: &str) -> Option<i64> {
    if !node.has_method(method) {
        return None;
    }
    let mut node = node.clone();
    node.try_call(method, &[]).ok()?.try_to::<i64>().ok()
}
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
fn json_float(value: f64) -> String {
    if value.is_finite() {
        format!("{:.*}", PRECISION, value)
    } else {
        "null".to_string()
    }
}
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
fn json_optional_floats(values: Option<&[f64]>) -> String {
    values.map_or_else(|| "null".to_string(), json_floats)
}
fn json_optional_bool(value: Option<bool>) -> String {
    value.map_or_else(|| "null".to_string(), |flag| flag.to_string())
}
fn json_optional_int(value: Option<i64>) -> String {
    value.map_or_else(|| "null".to_string(), |number| number.to_string())
}
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
