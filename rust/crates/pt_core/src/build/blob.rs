//! Versioned little-endian binary format for `BMesh` (one `PackedByteArray` per mesh keeps
//! scenes small).
//!
//! ```text
//! "PTBM" u16 version u16 flags
//! u32 nverts, nverts × f32[3]
//! u32 nfaces, per face: u16 n, n × u32, u16 material, u16 smooth, u8[4] color,
//!                       u8 uvflags (bit0 world), f32[2] offset, f32 rotation, f32[2] scale
//!                       v2+: u8 manual, u16 ncoords, ncoords × f32[2]   (manual per-corner UVs)
//!                       v3+: f32[3] uv_axis, u16 ncolors, ncolors × u8[4] (UV lock axis,
//!                            per-corner vertex-paint colours)
//! ```
//!
//! Older blobs still load: version 1 faces use the automatic projection, version 1–2 faces have
//! no projection axis and no corner colours (the face colour is used).

use super::model::{BMesh, Face, UvParams};
use crate::math::*;

const MAGIC: &[u8; 4] = b"PTBM";
pub const VERSION: u16 = 3;

pub fn encode(m: &BMesh) -> Vec<u8> {
    let mut b = Vec::with_capacity(16 + m.positions.len() * 12 + m.faces.len() * 48);
    b.extend_from_slice(MAGIC);
    b.extend_from_slice(&VERSION.to_le_bytes());
    b.extend_from_slice(&0u16.to_le_bytes());
    b.extend_from_slice(&(m.positions.len() as u32).to_le_bytes());
    for p in &m.positions {
        for c in [p.x, p.y, p.z] {
            b.extend_from_slice(&c.to_le_bytes());
        }
    }
    b.extend_from_slice(&(m.faces.len() as u32).to_le_bytes());
    for f in &m.faces {
        b.extend_from_slice(&(f.verts.len() as u16).to_le_bytes());
        for v in &f.verts {
            b.extend_from_slice(&v.to_le_bytes());
        }
        b.extend_from_slice(&f.material.to_le_bytes());
        b.extend_from_slice(&f.smooth.to_le_bytes());
        b.extend_from_slice(&f.color);
        b.push(f.uv.world as u8);
        for c in [f.uv.offset.x, f.uv.offset.y, f.uv.rotation, f.uv.scale.x, f.uv.scale.y] {
            b.extend_from_slice(&c.to_le_bytes());
        }
        b.push(f.uv_manual as u8);
        b.extend_from_slice(&(f.uv_coords.len() as u16).to_le_bytes());
        for c in &f.uv_coords {
            b.extend_from_slice(&c.x.to_le_bytes());
            b.extend_from_slice(&c.y.to_le_bytes());
        }
        for c in [f.uv_axis.x, f.uv_axis.y, f.uv_axis.z] {
            b.extend_from_slice(&c.to_le_bytes());
        }
        b.extend_from_slice(&(f.color_corners.len() as u16).to_le_bytes());
        for c in &f.color_corners {
            b.extend_from_slice(c);
        }
    }
    b
}

struct Reader<'a> {
    b: &'a [u8],
    i: usize,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], String> {
        let s = self.b.get(self.i..self.i + n).ok_or_else(|| format!("truncated at byte {}", self.i))?;
        self.i += n;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, String> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn f32(&mut self) -> Result<f32, String> {
        Ok(f32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
}

pub fn decode(bytes: &[u8]) -> Result<BMesh, String> {
    if bytes.is_empty() {
        return Ok(BMesh::default());
    }
    let mut r = Reader { b: bytes, i: 0 };
    if r.take(4)? != MAGIC {
        return Err("not a builder mesh blob".into());
    }
    let ver = r.u16()?;
    if ver == 0 || ver > VERSION {
        return Err(format!("unsupported builder mesh version {ver}"));
    }
    let _flags = r.u16()?;
    let nv = r.u32()? as usize;
    if nv > bytes.len() / 12 {
        return Err("vertex count exceeds blob size".into());
    }
    let mut m = BMesh::default();
    for _ in 0..nv {
        m.positions.push(v3(r.f32()?, r.f32()?, r.f32()?));
    }
    let nf = r.u32()? as usize;
    if nf > bytes.len() / 8 {
        return Err("face count exceeds blob size".into());
    }
    for _ in 0..nf {
        let n = r.u16()? as usize;
        let mut verts = Vec::with_capacity(n);
        for _ in 0..n {
            let v = r.u32()?;
            if v as usize >= nv {
                return Err(format!("vertex index {v} out of range"));
            }
            verts.push(v);
        }
        let material = r.u16()?;
        let smooth = r.u16()?;
        let c = r.take(4)?;
        let color = [c[0], c[1], c[2], c[3]];
        let world = r.u8()? & 1 == 1;
        let offset = v2(r.f32()?, r.f32()?);
        let rotation = r.f32()?;
        let scale = v2(r.f32()?, r.f32()?);
        let (mut uv_manual, mut uv_coords) = (false, Vec::new());
        if ver >= 2 {
            uv_manual = r.u8()? & 1 == 1;
            let nc = r.u16()? as usize;
            uv_coords.reserve(nc);
            for _ in 0..nc {
                uv_coords.push(v2(r.f32()?, r.f32()?));
            }
        }
        let (mut uv_axis, mut color_corners) = (Vec3::ZERO, Vec::new());
        if ver >= 3 {
            uv_axis = v3(r.f32()?, r.f32()?, r.f32()?);
            let nc = r.u16()? as usize;
            color_corners.reserve(nc);
            for _ in 0..nc {
                let c = r.take(4)?;
                color_corners.push([c[0], c[1], c[2], c[3]]);
            }
        }
        m.faces.push(Face { verts, material, smooth, color, uv: UvParams { offset, rotation, scale, world }, uv_manual, uv_coords, uv_axis, color_corners });
    }
    Ok(m)
}

#[cfg(test)]
mod tests {
    use super::super::shapes::*;
    use super::*;

    #[test]
    fn roundtrip() {
        let mut m = generate(&ShapeSpec { kind: ShapeKind::Stairs, ..Default::default() }, None);
        m.faces[2].material = 3;
        m.faces[1].color = [10, 20, 30, 40];
        m.faces[0].uv.rotation = 90.0;
        m.faces[0].uv.world = false;
        m.faces[3].uv_manual = true;
        m.faces[3].uv_coords = (0..m.faces[3].verts.len()).map(|i| v2(i as f32 * 0.25, 0.5)).collect();
        m.faces[4].uv_axis = v3(0.0, 0.6, 0.8);
        m.faces[4].color_corners = (0..m.faces[4].verts.len()).map(|i| [i as u8, 200, 30, 255 - i as u8]).collect();
        let b = encode(&m);
        assert_eq!(decode(&b).unwrap(), m);
        assert_eq!(decode(&[]).unwrap(), BMesh::default());
    }

    /// Version 1 blobs (no manual UV block) still decode, with automatic UVs.
    #[test]
    fn migrates_version_1() {
        let m = cuboid(v3(1.0, 2.0, 3.0));
        let mut v1 = Vec::new();
        v1.extend_from_slice(MAGIC);
        v1.extend_from_slice(&1u16.to_le_bytes());
        v1.extend_from_slice(&0u16.to_le_bytes());
        v1.extend_from_slice(&(m.positions.len() as u32).to_le_bytes());
        for p in &m.positions {
            for c in [p.x, p.y, p.z] {
                v1.extend_from_slice(&c.to_le_bytes());
            }
        }
        v1.extend_from_slice(&(m.faces.len() as u32).to_le_bytes());
        for f in &m.faces {
            v1.extend_from_slice(&(f.verts.len() as u16).to_le_bytes());
            for v in &f.verts {
                v1.extend_from_slice(&v.to_le_bytes());
            }
            v1.extend_from_slice(&f.material.to_le_bytes());
            v1.extend_from_slice(&f.smooth.to_le_bytes());
            v1.extend_from_slice(&f.color);
            v1.push(f.uv.world as u8);
            for c in [f.uv.offset.x, f.uv.offset.y, f.uv.rotation, f.uv.scale.x, f.uv.scale.y] {
                v1.extend_from_slice(&c.to_le_bytes());
            }
        }
        let d = decode(&v1).unwrap();
        assert_eq!(d, m);
        assert!(d.faces.iter().all(|f| !f.uv_manual && f.uv_coords.is_empty()));
        // Re-encoding writes the current version.
        assert_eq!(u16::from_le_bytes([encode(&d)[4], encode(&d)[5]]), VERSION);
    }

    /// Version 2 blobs (manual UVs, no axis / corner colours) still decode.
    #[test]
    fn migrates_version_2() {
        let mut m = cuboid(v3(1.0, 2.0, 3.0));
        m.faces[1].uv_manual = true;
        m.faces[1].uv_coords = (0..m.faces[1].verts.len()).map(|i| v2(i as f32, 1.0)).collect();
        let mut v2b = Vec::new();
        v2b.extend_from_slice(MAGIC);
        v2b.extend_from_slice(&2u16.to_le_bytes());
        v2b.extend_from_slice(&0u16.to_le_bytes());
        v2b.extend_from_slice(&(m.positions.len() as u32).to_le_bytes());
        for p in &m.positions {
            for c in [p.x, p.y, p.z] {
                v2b.extend_from_slice(&c.to_le_bytes());
            }
        }
        v2b.extend_from_slice(&(m.faces.len() as u32).to_le_bytes());
        for f in &m.faces {
            v2b.extend_from_slice(&(f.verts.len() as u16).to_le_bytes());
            for v in &f.verts {
                v2b.extend_from_slice(&v.to_le_bytes());
            }
            v2b.extend_from_slice(&f.material.to_le_bytes());
            v2b.extend_from_slice(&f.smooth.to_le_bytes());
            v2b.extend_from_slice(&f.color);
            v2b.push(f.uv.world as u8);
            for c in [f.uv.offset.x, f.uv.offset.y, f.uv.rotation, f.uv.scale.x, f.uv.scale.y] {
                v2b.extend_from_slice(&c.to_le_bytes());
            }
            v2b.push(f.uv_manual as u8);
            v2b.extend_from_slice(&(f.uv_coords.len() as u16).to_le_bytes());
            for c in &f.uv_coords {
                v2b.extend_from_slice(&c.x.to_le_bytes());
                v2b.extend_from_slice(&c.y.to_le_bytes());
            }
        }
        let d = decode(&v2b).unwrap();
        assert_eq!(d, m);
        assert!(d.faces.iter().all(|f| f.uv_axis == Vec3::ZERO && f.color_corners.is_empty()));
        assert_eq!(d.faces[1].manual_uvs().map(|c| c.len()), Some(4));
        assert_eq!(decode(&encode(&d)).unwrap(), m);
    }

    #[test]
    fn rejects_truncated_and_garbage() {
        let b = encode(&cuboid(v3(1.0, 1.0, 1.0)));
        for cut in [3, 8, 20, b.len() - 1] {
            assert!(decode(&b[..cut]).is_err(), "cut {cut}");
        }
        assert!(decode(b"XXXX\x01\x00\x00\x00").is_err());
        let mut v = b.clone();
        v[4] = 9;
        assert!(decode(&v).is_err());
    }
}
