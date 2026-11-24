//! Keeps the author informed while a terrain is written to a model file: part of that work
//! cannot run alongside the editor, and some files the editor cannot safely import back.
//!
//! The dialogs are created in code because they exist only while an export is being
//! confirmed or is running, and their wording depends on the export in hand. They are the
//! editor's own message dialogs and lay out nothing of their own.
use super::plugin::MarchingSquaresTerrainPlugin;
use super::terrain_ui::ATLAS_POLYGON_RESOLUTION;
use crate::terrain::MarchingSquaresTerrain;
use godot::classes::{
    AcceptDialog, ConfirmationDialog, EditorInterface, ProjectSettings, RenderingServer,
};
use godot::prelude::*;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ImportCrashRisk {
    pub inside_imported_project: bool,
    pub compatibility_renderer: bool,
    pub threaded_import: bool,
    pub textures: usize,
}
impl ImportCrashRisk {
    pub(super) fn applies(&self) -> bool {
        self.inside_imported_project
            && self.compatibility_renderer
            && self.threaded_import
            && self.textures >= 2
    }
}
fn inside_imported_project(project_dir: &Path, target: &Path) -> bool {
    target.starts_with(project_dir)
        && !target
            .ancestors()
            .skip(1)
            .take_while(|d| d.starts_with(project_dir))
            .any(|d| d.join(".gdignore").exists())
}
fn atlas_export_crash_risk(path: &GString, textures: impl FnOnce() -> usize) -> ImportCrashRisk {
    let settings = ProjectSettings::singleton();
    let project_dir = settings.globalize_path("res://").to_string();
    let target = settings.globalize_path(path).to_string();
    let mut risk = ImportCrashRisk {
        inside_imported_project: inside_imported_project(
            Path::new(&project_dir),
            Path::new(&target),
        ),
        compatibility_renderer: RenderingServer::singleton().get_current_rendering_method()
            == "gl_compatibility",
        threaded_import: settings
            .get_setting("editor/import/use_multiple_threads")
            .try_to::<bool>()
            .unwrap_or(true),
        textures: 0,
    };
    if risk.inside_imported_project && risk.compatibility_renderer && risk.threaded_import {
        risk.textures = textures();
    }
    risk
}
fn crash_risk_text(path: &GString, textures: usize) -> String {
    format!(
        "This export writes {textures} textures into {path}, inside the project folder.\n\n\
         With the Compatibility renderer and multi-threaded import both on, the Godot editor \
         can crash while importing a GLB that holds two or more textures, and it then crashes \
         again on every start until the file is removed.\n\n\
         To avoid it, set the terrain's Export Path to a folder outside the project or to a \
         folder that contains a .gdignore file, turn off Project Settings > Editor > Import > \
         Use Multiple Threads, or switch the renderer to Forward+ or Mobile.\n\n\
         The export also stops the editor responding for several seconds while the file is \
         written."
    )
}
fn baking_text(path: &GString, done: i64, total: i64) -> String {
    format!(
        "Baking chunk {done} of {total} for {path}.\n\n\
         When baking finishes, the editor will stop responding for several seconds while the \
         file is written."
    )
}
fn writing_text(path: &GString) -> String {
    format!(
        "Writing {path}.\n\n\
         The editor is not responding until the file is saved. On a large terrain this takes \
         several seconds."
    )
}
const TEXT_WIDTH: f32 = 520.0;
fn fit_text(d: &mut Gd<AcceptDialog>) {
    d.set_autowrap(true);
    if let Some(mut label) = d.get_label() {
        label.set_custom_minimum_size(Vector2::new(TEXT_WIDTH, 0.0));
    }
}
impl MarchingSquaresTerrainPlugin {
    pub(super) fn request_atlas_export(&mut self) {
        let Some(terrain) = self.terrain.clone() else {
            return;
        };
        let path = terrain.get("export_path").to::<GString>();
        let risk = atlas_export_crash_risk(&path, || {
            terrain
                .bind()
                .get_export_atlas_plan(ATLAS_POLYGON_RESOLUTION)
                .get("textures")
                .and_then(|v| v.try_to::<i64>().ok())
                .unwrap_or(0) as usize
        });
        let textures = risk.textures;
        if !risk.applies() {
            self.start_atlas_export();
            return;
        }
        let Some(mut base) = EditorInterface::singleton().get_base_control() else {
            self.start_atlas_export();
            return;
        };
        let mut d = ConfirmationDialog::new_alloc();
        d.set_title("Export GLB");
        d.set_text(&crash_risk_text(&path, textures));
        fit_text(&mut d.clone().upcast());
        d.set_ok_button_text("Export Anyway");
        let start = self.base().callable("on_atlas_export_confirmed");
        d.connect("confirmed", &start);
        let free = Callable::from_object_method(&d, "queue_free");
        d.connect("confirmed", &free);
        d.connect("canceled", &free);
        base.add_child(&d);
        d.popup_centered();
        if let Some(mut cancel) = d.get_cancel_button() {
            cancel.grab_focus();
        }
    }
    pub(super) fn show_export_progress_notice(
        &mut self,
        terrain: &Gd<MarchingSquaresTerrain>,
        done: i64,
        total: i64,
        strip_shows_progress: bool,
    ) {
        let path = terrain.get("export_path").to::<GString>();
        if done >= total {
            self.show_export_notice(terrain, &writing_text(&path), false, true);
        } else if strip_shows_progress {
            self.drop_export_notice();
        } else if done > 0 {
            self.show_export_notice(terrain, &baking_text(&path, done, total), true, false);
        }
    }
    fn show_export_notice(
        &mut self,
        terrain: &Gd<MarchingSquaresTerrain>,
        text: &str,
        cancellable: bool,
        raise: bool,
    ) {
        let existing = self.export_notice.clone().filter(|d| d.is_instance_valid());
        let (mut d, fresh) = match existing {
            Some(d) => (d, false),
            None => {
                let Some(mut base) = EditorInterface::singleton().get_base_control() else {
                    return;
                };
                let mut d = AcceptDialog::new_alloc();
                d.set_title("Exporting GLB");
                fit_text(&mut d);
                d.set_exclusive(false);
                d.set_ok_button_text("Cancel Export");
                let cancel = self
                    .base()
                    .callable("on_export_notice_cancel")
                    .bind(&[terrain.to_variant()]);
                d.connect("confirmed", &cancel);
                base.add_child(&d);
                self.export_notice = Some(d.clone());
                (d, true)
            }
        };
        d.set_text(text);
        if let Some(mut ok) = d.get_ok_button() {
            ok.set_visible(cancellable);
        }
        if fresh || (raise && !d.is_visible()) {
            d.popup_centered();
        } else {
            d.reset_size();
        }
    }
    pub(super) fn drop_export_notice(&mut self) {
        if let Some(d) = self.export_notice.take()
            && d.is_instance_valid()
        {
            d.free();
        }
    }
}
#[godot_api(secondary)]
impl MarchingSquaresTerrainPlugin {
    #[func]
    fn on_atlas_export_confirmed(&mut self) {
        self.start_atlas_export();
    }
    #[func]
    fn on_export_notice_cancel(&mut self, mut terrain: Gd<MarchingSquaresTerrain>) {
        terrain.bind_mut().cancel_export();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn risk() -> ImportCrashRisk {
        ImportCrashRisk {
            inside_imported_project: true,
            compatibility_renderer: true,
            threaded_import: true,
            textures: 2,
        }
    }
    #[test]
    fn two_textures_imported_by_threaded_compatibility_editor_is_at_risk() {
        assert!(risk().applies());
    }
    #[test]
    fn any_one_workaround_removes_the_risk() {
        assert!(!ImportCrashRisk { inside_imported_project: false, ..risk() }.applies());
        assert!(!ImportCrashRisk { compatibility_renderer: false, ..risk() }.applies());
        assert!(!ImportCrashRisk { threaded_import: false, ..risk() }.applies());
        assert!(!ImportCrashRisk { textures: 1, ..risk() }.applies());
    }
    #[test]
    fn targets_outside_the_project_or_under_an_ignored_folder_are_not_imported() {
        let root = std::env::temp_dir().join(format!("mst-export-risk-{}", std::process::id()));
        let ignored = root.join("exports");
        std::fs::create_dir_all(ignored.join("deep")).unwrap();
        std::fs::write(ignored.join(".gdignore"), b"").unwrap();
        assert!(inside_imported_project(&root, &root.join("terrain.glb")));
        assert!(!inside_imported_project(&root, &ignored.join("terrain.glb")));
        assert!(!inside_imported_project(&root, &ignored.join("deep/terrain.glb")));
        assert!(!inside_imported_project(&root, Path::new("/elsewhere/terrain.glb")));
        std::fs::remove_dir_all(&root).unwrap();
    }
}
