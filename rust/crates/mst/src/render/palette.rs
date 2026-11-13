//! `PixelPalette` — a color palette baked into the LUT the resolve pass grades through.
//!
//! Matching metrics: OKLab (default) plus the original Pixelizer3D `PaletteLUT` metrics (RGB,
//! HSV, weighted HSV, value + brightness curve, CIE Lab). The LUT can be baked to a PNG
//! ("Bake LUT to PNG" inspector button or *Project > Tools > Bake PixelPalette LUT to PNG*)
//! and a baked PNG can be loaded back through `lut_image` (this addon's 1024 x 32 layout or the
//! original addon's 256 x 256 layout), which skips baking at runtime.

use godot::classes::image::Format;
use godot::classes::{Curve, IResource, Image, ImageTexture, ProjectSettings, Texture2D};
use godot::global::Error;
use godot::prelude::*;
use pt_core::palette::{BakeOptions, DEFAULT_PALETTE, Metric, bake_lut_with, hex_to_rgb, import_lut};

#[derive(GodotClass)]
#[class(tool, base = Resource)]
pub struct PixelPalette {
    /// Palette colors (sRGB). Ignored when `source_image` is set.
    #[export]
    #[var(set = set_colors)]
    colors: PackedColorArray,
    /// Optional palette image (e.g. a 1-pixel-tall swatch strip); its unique colors are used.
    #[export]
    #[var(set = set_source_image)]
    source_image: Option<Gd<Texture2D>>,
    /// Color distance used to pick the nearest palette color.
    #[export(enum = (OkLab = 0, Rgb = 1, Hsv = 2, WeightedHsv = 3, Value = 4, CieLab = 5))]
    #[var(set = set_match_metric)]
    match_metric: i32,
    /// Hue / saturation / value weights (WeightedHsv).
    #[export]
    #[var(set = set_weights)]
    weights: Vector3,
    /// Maps input value to comparison value (Value metric).
    #[export]
    #[var(set = set_brightness_curve)]
    brightness_curve: Option<Gd<Curve>>,
    /// A baked LUT PNG to use instead of baking (1024 x 32, or the original 256 x 256 layout).
    #[export]
    #[var(set = set_lut_image)]
    lut_image: Option<Gd<Texture2D>>,
    /// Where "Bake LUT to PNG" writes.
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
            colors,
            source_image: None,
            match_metric: 0,
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
                    if !out.iter().any(|o| (o[0] - rgb[0]).abs() + (o[1] - rgb[1]).abs() + (o[2] - rgb[2]).abs() < 1.0 / 255.0) {
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
        self.colors.as_slice().iter().map(|c| [c.r, c.g, c.b]).collect()
    }

    fn options(&self) -> BakeOptions {
        let curve = self.brightness_curve.as_ref().map(|c| (0..256).map(|i| c.sample(i as f32 / 255.0)).collect());
        BakeOptions { metric: Metric::from_i32(self.match_metric), weights: [self.weights.x, self.weights.y, self.weights.z], value_curve: curve }
    }

    /// The loaded `lut_image` converted to this addon's layout.
    fn imported(&self) -> Option<Gd<Image>> {
        let tex = self.lut_image.as_ref()?;
        let mut img = tex.get_image()?;
        if img.is_compressed() {
            img.decompress();
        }
        img.convert(Format::RGBA8);
        let (w, h) = (img.get_width() as usize, img.get_height() as usize);
        let data = img.get_data();
        match import_lut(w, h, data.as_slice()) {
            Some(bytes) => Image::create_from_data(pt_core::palette::LUT_WIDTH as i32, pt_core::palette::LUT_SIZE as i32, false, Format::RGBA8, &PackedByteArray::from(bytes.as_slice())),
            None => {
                godot_warn!("PixelPalette: lut_image must be 1024x32 (this addon) or 256x256 (Pixelizer3D PaletteLUT); got {w}x{h}. Baking instead.");
                None
            }
        }
    }

    pub fn lut_image(&self) -> Option<Gd<Image>> {
        if let Some(img) = self.imported() {
            return Some(img);
        }
        let (w, h, bytes) = bake_lut_with(&self.palette_rgb(), &self.options());
        Image::create_from_data(w as i32, h as i32, false, Format::RGBA8, &PackedByteArray::from(bytes.as_slice()))
    }

    /// Returns the (cached) LUT texture, baking it on first use.
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
    fn set_colors(&mut self, v: PackedColorArray) {
        self.colors = v;
        self.invalidate();
    }

    #[func]
    fn set_source_image(&mut self, v: Option<Gd<Texture2D>>) {
        self.source_image = v;
        self.invalidate();
    }

    #[func]
    fn set_match_metric(&mut self, v: i32) {
        self.match_metric = v.clamp(0, 5);
        self.invalidate();
    }

    #[func]
    fn set_weights(&mut self, v: Vector3) {
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

    /// Bakes the LUT now and returns it (also useful to save it as a PNG for inspection).
    #[func]
    fn bake(&mut self) -> Option<Gd<ImageTexture>> {
        self.lut = None;
        self.lut_texture()
    }

    /// Bakes the LUT (ignoring `lut_image`) and writes it as a PNG; refreshes the editor's
    /// filesystem when the path is inside the project. Returns true on success.
    #[func]
    pub fn save_lut_png(&mut self, path: GString) -> bool {
        let path = if path.is_empty() { GString::from("res://palette_lut.png") } else { path };
        let (w, h, bytes) = bake_lut_with(&self.palette_rgb(), &self.options());
        let Some(img) = Image::create_from_data(w as i32, h as i32, false, Format::RGBA8, &PackedByteArray::from(bytes.as_slice())) else {
            return false;
        };
        let err = img.save_png(&path);
        if err != Error::OK {
            godot_error!("PixelPalette: could not write {path}: {err:?}");
            return false;
        }
        godot_print!("PixelPalette: baked {w}x{h} LUT -> {path} ({})", ProjectSettings::singleton().globalize_path(&path));
        if crate::util::is_editor() && path.to_string().starts_with("res://") {
            godot::classes::EditorInterface::singleton().get_resource_filesystem().map(|mut fs| fs.scan());
        }
        true
    }
}
