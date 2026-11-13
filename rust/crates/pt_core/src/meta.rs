//! Metadata-pass packing, mirrored by `pixel_common.gdshaderinc` (encode) and
//! `pixel_resolve.gdshader` (decode).
//!
//! The Compatibility renderer's lit output is only bit-exact for channel values >= ~56, and
//! alpha is always written as 1, so the metadata pass stores a mixed-radix number
//! `N = (flags * NORMAL_LEVELS + normal) * DEPTH_LEVELS + depth` as three base-192 digits,
//! each offset by 64 (`digit + 64` in 0..255). A pixel whose channels are below 64 is
//! background.
//!
//! * `flags`  (64): bits 0-1 pixel-size multiplier - 1, bits 2-4 outline palette id & 7,
//!   bit 5 outline enabled.
//! * `normal` (81): view-space normal, hemi-octahedral on a 9x9 grid.
//! * `depth`  (1365): linear view depth / depth range.
//!
//! That pass is full (64 * 81 * 1365 = 7 076 160 of 192^3 = 7 077 888 codes), so the wider
//! per-object data goes through a second, optional **id pass**: a camera whose cull mask is
//! `pt_id_mask` (meta layer + bit [`ID_CAMERA_BIT`]) draws the same geometry and every pixel
//! shader writes [`IdMeta`] instead, with the same digit scheme:
//! `N = aux * OUTLINE_IDS + outline_id` (aux bit 0 = palette off, bit 1 = dither off).
//! Outline ids 0..=253 are user ids; [`HOVER_ID`] and [`SELECT_ID`] are the highlight rows of
//! the outline palette. Without the id pass the resolve falls back to the meta pass' 3-bit
//! id (`outline_id & 7`, palette and dither on).
//!
//! Per-object input is `pt_flags` (an int instance or material uniform), see [`object_flags`]:
//! bits 0-1 pixel size - 1, bits 2-4 outline id & 7, bit 5 outline, bit 6 texel snap,
//! bits 8-15 full outline id, bit 17 palette off, bit 18 dither off.

pub const DIGIT_BASE: u32 = 192;
pub const DIGIT_OFFSET: u32 = 64;
pub const FLAG_LEVELS: u32 = 64;
pub const NORMAL_GRID: u32 = 9;
pub const NORMAL_LEVELS: u32 = NORMAL_GRID * NORMAL_GRID;
pub const DEPTH_LEVELS: u32 = 1365;

/// Codes representable by three digits.
pub const DIGIT_CODES: u32 = DIGIT_BASE * DIGIT_BASE * DIGIT_BASE;
/// Lowest channel value Compatibility's lit output reproduces exactly (see AGENTS.md).
pub const MIN_EXACT_CHANNEL: u32 = 56;

const _: () = assert!(FLAG_LEVELS * NORMAL_LEVELS * DEPTH_LEVELS <= DIGIT_CODES);
const _: () = assert!(DIGIT_OFFSET + DIGIT_BASE - 1 <= 255);
const _: () = assert!(DIGIT_OFFSET >= MIN_EXACT_CHANNEL);

/// Outline palette rows (ids 0..=255).
pub const OUTLINE_IDS: u32 = 256;
/// Highest id objects may use; the last two rows are reserved for highlights.
pub const MAX_USER_ID: u32 = 253;
/// Outline id forced onto hovered geometry (diagnostics picker, `PixelObject.highlight`).
pub const HOVER_ID: u32 = 254;
/// Outline id forced onto selected / focused geometry.
pub const SELECT_ID: u32 = 255;
/// Aux flag combinations in the id pass (palette off, dither off).
pub const ID_AUX_LEVELS: u32 = 4;
const _: () = assert!(OUTLINE_IDS * ID_AUX_LEVELS <= DIGIT_CODES);

/// Bit added to the meta-layer mask to form the id camera's cull mask (`pt_id_mask`). It lies
/// above the 20 visual layers and *below* the 3D editor's own layers (24 misc tools, 25 grid /
/// origin, 26 edit, 27–30 per-viewport gizmos): bit 30 made the editor's origin axis lines
/// render into the editor preview's id pass, where they were outlined. No geometry uses it and
/// no game/editor camera mask equals `pt_id_mask`.
pub const ID_CAMERA_BIT: u32 = 23;

pub const FLAG_OUTLINE: i32 = 1 << 5;
pub const FLAG_SNAP: i32 = 1 << 6;
pub const FLAG_NO_PALETTE: i32 = 1 << 17;
pub const FLAG_NO_DITHER: i32 = 1 << 18;

/// Packs the per-object `pt_flags` uniform (decoded by `pixel_common.gdshaderinc`).
pub fn object_flags(pixel_size: i32, outline: bool, outline_id: i32, snap: bool, palette: bool, dither: bool) -> i32 {
    let id = outline_id.clamp(0, OUTLINE_IDS as i32 - 1);
    ((pixel_size.clamp(1, 4) - 1) & 3)
        | ((id & 7) << 2)
        | if outline { FLAG_OUTLINE } else { 0 }
        | if snap { FLAG_SNAP } else { 0 }
        | (id << 8)
        | if palette { 0 } else { FLAG_NO_PALETTE }
        | if dither { 0 } else { FLAG_NO_DITHER }
}

/// Replaces the outline id of packed flags (highlights) and forces the outline on.
pub fn with_outline_id(flags: i32, outline_id: u32) -> i32 {
    let id = outline_id.min(OUTLINE_IDS - 1) as i32;
    (flags & !(7 << 2) & !(255 << 8)) | ((id & 7) << 2) | (id << 8) | FLAG_OUTLINE
}

/// What the id pass writes for packed `pt_flags` (the shader's `pt_id_color`). Writers that
/// only fill bits 2-4 (id < 8) keep their id.
pub fn id_meta_from_flags(flags: i32) -> IdMeta {
    let hi = ((flags >> 8) & 255) as u32;
    let lo = ((flags >> 2) & 7) as u32;
    IdMeta { outline_id: if hi != 0 { hi } else { lo }, palette: flags & FLAG_NO_PALETTE == 0, dither: flags & FLAG_NO_DITHER == 0 }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Meta {
    pub pixel_size: u32,
    pub outline_id: u32,
    pub outline: bool,
    pub normal: u32,
    pub depth: u32,
}

/// Id-pass payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IdMeta {
    pub outline_id: u32,
    pub palette: bool,
    pub dither: bool,
}

fn digits(n: u32) -> [u8; 3] {
    debug_assert!(n < DIGIT_CODES);
    let d0 = n / (DIGIT_BASE * DIGIT_BASE);
    let d1 = (n / DIGIT_BASE) % DIGIT_BASE;
    let d2 = n % DIGIT_BASE;
    [(d0 + DIGIT_OFFSET) as u8, (d1 + DIGIT_OFFSET) as u8, (d2 + DIGIT_OFFSET) as u8]
}

fn number(b: [u8; 3]) -> Option<u32> {
    if b.iter().any(|&x| (x as u32) < DIGIT_OFFSET) {
        return None;
    }
    let d = b.map(|x| x as u32 - DIGIT_OFFSET);
    Some((d[0] * DIGIT_BASE + d[1]) * DIGIT_BASE + d[2])
}

pub fn pack(m: Meta) -> [u8; 3] {
    let flags = (m.pixel_size.clamp(1, 4) - 1) | (m.outline_id.min(7) << 2) | ((m.outline as u32) << 5);
    digits((flags * NORMAL_LEVELS + m.normal.min(NORMAL_LEVELS - 1)) * DEPTH_LEVELS + m.depth.min(DEPTH_LEVELS - 1))
}

pub fn unpack(b: [u8; 3]) -> Option<Meta> {
    let n = number(b)?;
    let depth = n % DEPTH_LEVELS;
    let rest = n / DEPTH_LEVELS;
    let normal = rest % NORMAL_LEVELS;
    let flags = rest / NORMAL_LEVELS;
    Some(Meta { pixel_size: (flags & 3) + 1, outline_id: (flags >> 2) & 7, outline: flags & 32 != 0, normal, depth })
}

pub fn pack_id(m: IdMeta) -> [u8; 3] {
    let aux = (!m.palette as u32) | ((!m.dither as u32) << 1);
    digits(aux * OUTLINE_IDS + m.outline_id.min(OUTLINE_IDS - 1))
}

pub fn unpack_id(b: [u8; 3]) -> Option<IdMeta> {
    let n = number(b)?;
    let aux = n / OUTLINE_IDS;
    if aux >= ID_AUX_LEVELS {
        return None;
    }
    Some(IdMeta { outline_id: n % OUTLINE_IDS, palette: aux & 1 == 0, dither: aux & 2 == 0 })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_extremes() {
        for ps in 1..=4 {
            for id in [0, 3, 7] {
                for o in [false, true] {
                    for (nrm, dep) in [(0, 0), (80, 1364), (40, 700)] {
                        let m = Meta { pixel_size: ps, outline_id: id, outline: o, normal: nrm, depth: dep };
                        assert_eq!(unpack(pack(m)), Some(m));
                    }
                }
            }
        }
        assert_eq!(unpack([0, 0, 0]), None);
    }

    #[test]
    fn digit_budget() {
        assert!(FLAG_LEVELS * NORMAL_LEVELS * DEPTH_LEVELS <= DIGIT_CODES);
        assert!(OUTLINE_IDS * ID_AUX_LEVELS <= DIGIT_CODES);
        let max_meta = pack(Meta { pixel_size: 4, outline_id: 7, outline: true, normal: NORMAL_LEVELS - 1, depth: DEPTH_LEVELS - 1 });
        let max_id = pack_id(IdMeta { outline_id: SELECT_ID, palette: false, dither: false });
        let min_id = pack_id(IdMeta { outline_id: 0, palette: true, dither: true });
        let min_meta = pack(Meta { pixel_size: 1, outline_id: 0, outline: false, normal: 0, depth: 0 });
        for b in max_meta.iter().chain(&max_id).chain(&min_id).chain(&min_meta) {
            assert!(*b as u32 >= DIGIT_OFFSET && *b as u32 >= MIN_EXACT_CHANNEL);
        }
        assert!(MAX_USER_ID + 1 >= 64, "at least 64 user outline ids");
        assert!(HOVER_ID > MAX_USER_ID && SELECT_ID > HOVER_ID && SELECT_ID < OUTLINE_IDS);
        assert!(ID_CAMERA_BIT >= 20 && ID_CAMERA_BIT < 24);
    }

    #[test]
    fn id_roundtrip_all() {
        for id in 0..OUTLINE_IDS {
            for (p, d) in [(true, true), (false, true), (true, false), (false, false)] {
                let m = IdMeta { outline_id: id, palette: p, dither: d };
                let b = pack_id(m);
                assert!(b.iter().all(|&x| x as u32 >= DIGIT_OFFSET));
                assert_eq!(unpack_id(b), Some(m));
            }
        }
        assert_eq!(unpack_id([10, 200, 200]), None);
        assert_eq!(unpack_id([255, 255, 255]), None);
    }

    #[test]
    fn flags_layout() {
        for id in 0..OUTLINE_IDS as i32 {
            for (p, d) in [(true, true), (false, false), (true, false)] {
                let f = object_flags(3, true, id, true, p, d);
                assert_eq!(f & 3, 2);
                assert_eq!((f >> 2) & 7, id & 7);
                assert_ne!(f & FLAG_OUTLINE, 0);
                assert_ne!(f & FLAG_SNAP, 0);
                assert_eq!(id_meta_from_flags(f), IdMeta { outline_id: id as u32, palette: p, dither: d });
                // The meta pass sees the same pixel size / outline / low id bits.
                let m = unpack(pack(Meta { pixel_size: 3, outline_id: (f as u32 >> 2) & 7, outline: true, normal: 0, depth: 0 })).unwrap();
                assert_eq!(m.outline_id, id as u32 & 7);
            }
        }
        // Default terrain/water flags (32) are id 0, palette + dither on.
        assert_eq!(id_meta_from_flags(32), IdMeta { outline_id: 0, palette: true, dither: true });
        assert_eq!(id_meta_from_flags(32 | (5 << 2)).outline_id, 5);
        let h = with_outline_id(object_flags(2, false, 9, false, false, true), HOVER_ID);
        assert_eq!(id_meta_from_flags(h), IdMeta { outline_id: HOVER_ID, palette: false, dither: true });
        assert_ne!(h & FLAG_OUTLINE, 0);
        assert_eq!(h & 3, 1);
    }
}
