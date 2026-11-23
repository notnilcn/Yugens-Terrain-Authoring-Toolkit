use godot::classes::image::Format;
use godot::classes::{Curve, IResource, Image, ImageTexture, ProjectSettings, Texture2D};
use godot::global::Error;
use godot::prelude::*;
use pt_core::palette::{
    ColorQuantize, DEFAULT_PALETTE, QuantizeOptions, bake_lut_with, hex_to_rgb, import_color_cube,
};
#[derive(GodotClass)]
#[class(tool, base = Resource)]
pub struct PixelPalette {
    #[export]
    #[var(set = assign_tints)]
    tints: PackedColorArray,
    #[export]
    #[var(set = set_source_image)]
    source_image: Option<Gd<Texture2D>>,
    #[export(enum = (Perceptual = 0, Channel = 1, Hue = 2, HueWeighted = 3, Lab = 4, Value = 5))]
    #[var(set = set_quantize_mode)]
    quantize_mode: i32,
    #[export]
    #[var(set = assign_hefts)]
    weights: Vector3,
    #[export]
    #[var(set = set_brightness_curve)]
    brightness_curve: Option<Gd<Curve>>,
    #[export]
    #[var(set = set_lut_image)]
    lut_image: Option<Gd<Texture2D>>,
    #[export(file = "*.png")]
    output_path: GString,
    #[export_tool_button(fn = Self::bake_button, name = "Bake LUT to PNG", icon = "ImageTexture")]
    bake_lut_png: PhantomVar<Callable>,
    lut: Option<Gd<ImageTexture>>,
    base: Base<Resource>,
}
#[godot_api]
impl IResource for PixelPalette {
    fn init(base: Base<Resource>) -> Self {
        let colors = DEFAULT_PALETTE
            .iter()
            .map(|c| {
                let [r, g, b] = hex_to_rgb(*c);
                Color::from_rgb(r, g, b)
            })
            .collect();
        PixelPalette {
            tints: colors,
            source_image: None,
            quantize_mode: 0,
            weights: Vector3::ONE,
            brightness_curve: None,
            lut_image: None,
            output_path: "res://palette_lut.png".into(),
            bake_lut_png: PhantomVar::default(),
            lut: None,
            base,
        }
    }
}
impl PixelPalette {
    fn palette_rgb(&self) -> Vec<[f32; 3]> {
        if let Some(tex) = &self.source_image
            && let Some(mut img) = tex.get_image()
        {
            if img.is_compressed() {
                img.decompress();
            }
            img.convert(Format::RGBA8);
            let mut out: Vec<[f32; 3]> = Vec::new();
            let (w, h) = (img.get_width(), img.get_height());
            for y in 0..h.min(256) {
                for x in 0..w.min(256) {
                    let c = img.get_pixel(x, y);
                    if c.a < 0.5 {
                        continue;
                    }
                    let rgb = [c.r, c.g, c.b];
                    if !out.iter().any(|o| {
                        (o[0] - rgb[0]).abs() + (o[1] - rgb[1]).abs() + (o[2] - rgb[2]).abs()
                            < 1.0 / 255.0
                    }) {
                        out.push(rgb);
                    }
                    if out.len() >= 256 {
                        return out;
                    }
                }
            }
            if !out.is_empty() {
                return out;
            }
        }
        self.tints
            .as_slice()
            .iter()
            .map(|c| [c.r, c.g, c.b])
            .collect()
    }
    fn choices(&self) -> QuantizeOptions {
        let curve = self
            .brightness_curve
            .as_ref()
            .map(|c| (0..256).map(|i| c.sample(i as f32 / 255.0)).collect());
        QuantizeOptions {
            quantize: ColorQuantize::from_index(self.quantize_mode),
            weights: [self.weights.x, self.weights.y, self.weights.z],
            value_curve: curve,
        }
    }
    fn imported(&self) -> Option<Gd<Image>> {
        let tex = self.lut_image.as_ref()?;
        let mut img = tex.get_image()?;
        if img.is_compressed() {
            img.decompress();
        }
        img.convert(Format::RGBA8);
        let (w, h) = (img.get_width() as usize, img.get_height() as usize);
        let data = img.get_data();
        match import_color_cube(w, h, data.as_slice()) {
            Some(bytes) => Image::create_from_data(
                pt_core::palette::LUT_WIDTH as i32,
                pt_core::palette::LUT_SIZE as i32,
                false,
                Format::RGBA8,
                &PackedByteArray::from(bytes.as_slice()),
            ),
            None => {
                godot_warn!(
                    "PixelPalette: lut_image must be 1024x32 (this addon) or 1024x1024 (a colour-cube chart); got {w}x{h}. Baking instead."
                );
                None
            }
        }
    }
    pub fn lut_image(&self) -> Option<Gd<Image>> {
        if let Some(img) = self.imported() {
            return Some(img);
        }
        let (w, h, bytes) = bake_lut_with(&self.palette_rgb(), &self.choices());
        Image::create_from_data(
            w as i32,
            h as i32,
            false,
            Format::RGBA8,
            &PackedByteArray::from(bytes.as_slice()),
        )
    }
    pub fn lut_texture(&mut self) -> Option<Gd<ImageTexture>> {
        if self.lut.is_none() {
            let img = self.lut_image()?;
            self.lut = ImageTexture::create_from_image(&img);
        }
        self.lut.clone()
    }
    fn invalidate(&mut self) {
        self.lut = None;
        self.base_mut().emit_changed();
    }
    fn bake_button(&mut self) {
        let path = self.output_path.clone();
        self.save_lut_png(path);
    }
}
#[godot_api]
impl PixelPalette {
    #[func]
    fn assign_tints(&mut self, v: PackedColorArray) {
        self.tints = v;
        self.invalidate();
    }
    #[func]
    fn set_source_image(&mut self, v: Option<Gd<Texture2D>>) {
        self.source_image = v;
        self.invalidate();
    }
    #[func]
    fn set_quantize_mode(&mut self, v: i32) {
        self.quantize_mode = v.clamp(0, 5);
        self.invalidate();
    }
    #[func]
    fn assign_hefts(&mut self, v: Vector3) {
        self.weights = v;
        self.invalidate();
    }
    #[func]
    fn set_brightness_curve(&mut self, v: Option<Gd<Curve>>) {
        self.brightness_curve = v;
        self.invalidate();
    }
    #[func]
    fn set_lut_image(&mut self, v: Option<Gd<Texture2D>>) {
        self.lut_image = v;
        self.invalidate();
    }
    #[func]
    fn bake(&mut self) -> Option<Gd<ImageTexture>> {
        self.lut = None;
        self.lut_texture()
    }
    #[func]
    pub fn save_lut_png(&mut self, path: GString) -> bool {
        let path = if path.is_empty() {
            GString::from("res://palette_lut.png")
        } else {
            path
        };
        let (w, h, bytes) = bake_lut_with(&self.palette_rgb(), &self.choices());
        let Some(img) = Image::create_from_data(
            w as i32,
            h as i32,
            false,
            Format::RGBA8,
            &PackedByteArray::from(bytes.as_slice()),
        ) else {
            return false;
        };
        let err = img.save_png(&path);
        if err != Error::OK {
            godot_error!("PixelPalette: could not write {path}: {err:?}");
            return false;
        }
        godot_print!(
            "PixelPalette: baked {w}x{h} LUT -> {path} ({})",
            ProjectSettings::singleton().globalize_path(&path)
        );
        if crate::editor_detection::is_editor() && path.to_string().starts_with("res://") {
            godot::classes::EditorInterface::singleton()
                .get_resource_filesystem()
                .map(|mut fs| fs.scan());
        }
        true
    }
}
