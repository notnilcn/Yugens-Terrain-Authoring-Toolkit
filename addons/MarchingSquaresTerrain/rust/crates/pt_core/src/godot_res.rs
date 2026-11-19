//! Minimal reader for Godot 4's binary resource format (`.res`, magic `RSRC`).
//!
//! It reads the header, string table, external/internal resource tables and the properties
//! of any resource in the file, which is enough to import plain-data resources such as the
//! original MarchingSquaresTerrain's `MSTChunkData` without its GDScript class being
//! available. Compressed files (`RSCC`, written with `ResourceSaver.FLAG_COMPRESS`) are a
//! block container: [`split_compressed`] returns the blocks and the caller decompresses them
//! (the engine has the codecs; this crate stays dependency-free) and joins them with
//! [`join_blocks`].

use std::fmt;

/// A decoded Variant. Math types keep `f64` components regardless of `real_t`.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Nil,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    StringName(String),
    NodePath(String),
    /// Vector2/3/4, Rect2, Plane, Quaternion, AABB, Basis, Transform2D/3D, Projection.
    Reals(Vec<f64>),
    /// Vector2i/3i/4i, Rect2i.
    Ints(Vec<i32>),
    Color([f32; 4]),
    Rid,
    Object(ObjRef),
    Callable,
    Signal,
    Dict(Vec<(Value, Value)>),
    Array(Vec<Value>),
    Bytes(Vec<u8>),
    Int32s(Vec<i32>),
    Int64s(Vec<i64>),
    Float32s(Vec<f32>),
    Float64s(Vec<f64>),
    Strings(Vec<String>),
    /// Packed Vector2/3/4 arrays, flattened (`dim` components per element).
    Vectors { dim: usize, data: Vec<f64> },
    Colors(Vec<[f32; 4]>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum ObjRef {
    Empty,
    External { type_name: String, path: String },
    ExternalIndex(u32),
    Internal(u32),
}

impl Value {
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Int(i) => Some(*i),
            Value::Bool(b) => Some(*b as i64),
            Value::Float(f) => Some(*f as i64),
            _ => None,
        }
    }
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Int(i) => Some(*i as f64),
            Value::Float(f) => Some(*f),
            Value::Bool(b) => Some(*b as i64 as f64),
            _ => None,
        }
    }
    pub fn as_ivec2(&self) -> Option<[i32; 2]> {
        match self {
            Value::Ints(v) if v.len() == 2 => Some([v[0], v[1]]),
            Value::Reals(v) if v.len() == 2 => Some([v[0] as i32, v[1] as i32]),
            _ => None,
        }
    }
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Value::Bytes(b) => Some(b),
            _ => None,
        }
    }
    /// Numbers of an Array / packed numeric array, flattened one level (2D `Array[Array]`
    /// height maps become row-major).
    pub fn numbers(&self) -> Option<Vec<f64>> {
        let mut out = Vec::new();
        fn push(v: &Value, out: &mut Vec<f64>, depth: u32) -> bool {
            match v {
                Value::Array(a) if depth < 2 => a.iter().all(|e| push(e, out, depth + 1)),
                Value::Float32s(a) => {
                    out.extend(a.iter().map(|x| *x as f64));
                    true
                }
                Value::Float64s(a) => {
                    out.extend_from_slice(a);
                    true
                }
                Value::Int32s(a) => {
                    out.extend(a.iter().map(|x| *x as f64));
                    true
                }
                Value::Int64s(a) => {
                    out.extend(a.iter().map(|x| *x as f64));
                    true
                }
                other => match other.as_f64() {
                    Some(f) if depth > 0 => {
                        out.push(f);
                        true
                    }
                    _ => false,
                },
            }
        }
        push(self, &mut out, 0).then_some(out)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResError(pub String);

impl fmt::Display for ResError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

fn err<T>(msg: impl Into<String>) -> Result<T, ResError> {
    Err(ResError(msg.into()))
}

/// One resource stored in the file.
#[derive(Clone, Debug, PartialEq)]
pub struct ResourceEntry {
    /// `local://…` id of the resource in the file.
    pub path: String,
    pub type_name: String,
    pub props: Vec<(String, Value)>,
}

impl ResourceEntry {
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.props.iter().find(|(k, _)| k == name).map(|(_, v)| v)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResourceFile {
    pub version: (u32, u32, u32),
    /// Type of the main resource (e.g. `Resource`).
    pub main_type: String,
    /// Global script class of the main resource, when recorded (e.g. `MSTChunkData`).
    pub script_class: String,
    pub ext_resources: Vec<(String, String)>,
    /// The decoded main resource (sub-resources are skipped).
    pub resources: Vec<ResourceEntry>,
}

impl ResourceFile {
    pub fn main(&self) -> Option<&ResourceEntry> {
        self.resources.last()
    }
}

/// Splits an `RSCC` container into `(mode, uncompressed_total, block_size, blocks)`.
/// Every block but the last decompresses to `block_size` bytes. `mode` is Godot's
/// `FileAccess.CompressionMode` (0 FastLZ, 1 Deflate, 2 Zstd, 3 Gzip, 4 Brotli).
pub fn split_compressed(b: &[u8]) -> Result<(u32, usize, usize, Vec<&[u8]>), ResError> {
    if b.len() < 16 || &b[0..4] != b"RSCC" {
        return err("not an RSCC file");
    }
    let rd = |o: usize| u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
    let mode = rd(4);
    let block_size = rd(8) as usize;
    let total = rd(12) as usize;
    if block_size == 0 {
        return err("RSCC: zero block size");
    }
    let count = total / block_size + 1;
    let mut o = 16;
    if b.len() < o + count * 4 {
        return err("RSCC: truncated block table");
    }
    let sizes: Vec<usize> = (0..count).map(|i| rd(o + i * 4) as usize).collect();
    o += count * 4;
    let mut blocks = Vec::with_capacity(count);
    for s in sizes {
        if o + s > b.len() {
            return err("RSCC: truncated block");
        }
        blocks.push(&b[o..o + s]);
        o += s;
    }
    Ok((mode, total, block_size, blocks))
}

/// Uncompressed size of block `i` of an RSCC container.
pub fn block_len(total: usize, block_size: usize, i: usize) -> usize {
    let start = i * block_size;
    total.saturating_sub(start).min(block_size)
}

pub fn join_blocks(blocks: Vec<Vec<u8>>, total: usize) -> Result<Vec<u8>, ResError> {
    let out: Vec<u8> = blocks.into_iter().flatten().collect();
    if out.len() != total {
        return err(format!("RSCC: decompressed {} bytes, expected {total}", out.len()));
    }
    Ok(out)
}

/// Reads an uncompressed `RSRC` resource file.
pub fn parse(b: &[u8]) -> Result<ResourceFile, ResError> {
    if b.len() < 4 || &b[0..4] != b"RSRC" {
        return err("not an RSRC resource");
    }
    parse_from(b, 4)
}

/// Reads the decompressed payload of an `RSCC` file (the stream after the magic; resource
/// offsets are relative to it).
pub fn parse_decompressed(b: &[u8]) -> Result<ResourceFile, ResError> {
    parse_from(b, 0)
}

fn parse_from(b: &[u8], start: usize) -> Result<ResourceFile, ResError> {
    let mut r = Reader { b, o: start, real64: false, strings: Vec::new(), ver_format: 0 };
    let big_endian = r.u32()?;
    let _use_real64 = r.u32()?;
    if big_endian != 0 {
        return err("big-endian resources are not supported");
    }
    let major = r.u32()?;
    let minor = r.u32()?;
    let format = r.u32()?;
    r.ver_format = format;
    if major < 4 {
        return err(format!("resource format of Godot {major}.{minor} is not supported"));
    }
    let main_type = r.ustr()?;
    let _importmd = r.u64()?;
    let flags = r.u32()?;
    const NAMED_SCENE_IDS: u32 = 1;
    const UIDS: u32 = 2;
    const REAL_T_IS_DOUBLE: u32 = 4;
    const HAS_SCRIPT_CLASS: u32 = 8;
    let _ = NAMED_SCENE_IDS;
    let _uid = if flags & UIDS != 0 { r.u64()? } else { 0 };
    r.real64 = flags & REAL_T_IS_DOUBLE != 0;
    let script_class = if flags & HAS_SCRIPT_CLASS != 0 { r.ustr()? } else { String::new() };
    for _ in 0..11 {
        r.u32()?; // reserved fields
    }
    let n = r.u32()? as usize;
    if n > 1 << 20 {
        return err("string table too large");
    }
    for _ in 0..n {
        let s = r.ustr()?;
        r.strings.push(s);
    }
    let n = r.u32()? as usize;
    let mut ext_resources = Vec::with_capacity(n.min(4096));
    for _ in 0..n {
        let t = r.ustr()?;
        let p = r.ustr()?;
        if flags & UIDS != 0 {
            r.u64()?;
        }
        ext_resources.push((t, p));
    }
    let n = r.u32()? as usize;
    let mut table = Vec::with_capacity(n.min(4096));
    for _ in 0..n {
        let p = r.ustr()?;
        let ofs = r.u64()? as usize;
        table.push((p, ofs));
    }
    // Only the main resource (the last entry) is decoded: sub-resources (meshes, …) may use
    // variant types this reader does not need to understand.
    let mut resources = Vec::with_capacity(1);
    for (path, ofs) in table.into_iter().rev().take(1) {
        if ofs >= b.len() {
            return err(format!("resource {path}: offset out of range"));
        }
        r.o = ofs;
        let type_name = r.ustr()?;
        let pc = r.u32()? as usize;
        let mut props = Vec::with_capacity(pc.min(4096));
        for _ in 0..pc {
            let name = r.name()?;
            let v = r.variant().map_err(|e| ResError(format!("{type_name}.{name}: {}", e.0)))?;
            props.push((name, v));
        }
        resources.push(ResourceEntry { path, type_name, props });
    }
    Ok(ResourceFile { version: (major, minor, format), main_type, script_class, ext_resources, resources })
}

struct Reader<'a> {
    b: &'a [u8],
    o: usize,
    real64: bool,
    strings: Vec<String>,
    ver_format: u32,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], ResError> {
        if self.o + n > self.b.len() {
            return err("unexpected end of file");
        }
        let s = &self.b[self.o..self.o + n];
        self.o += n;
        Ok(s)
    }
    fn u16(&mut self) -> Result<u16, ResError> {
        let s = self.take(2)?;
        Ok(u16::from_le_bytes([s[0], s[1]]))
    }
    fn u32(&mut self) -> Result<u32, ResError> {
        let s = self.take(4)?;
        Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }
    fn u64(&mut self) -> Result<u64, ResError> {
        let s = self.take(8)?;
        let mut a = [0u8; 8];
        a.copy_from_slice(s);
        Ok(u64::from_le_bytes(a))
    }
    fn f32(&mut self) -> Result<f32, ResError> {
        Ok(f32::from_bits(self.u32()?))
    }
    fn f64(&mut self) -> Result<f64, ResError> {
        Ok(f64::from_bits(self.u64()?))
    }
    fn real(&mut self) -> Result<f64, ResError> {
        if self.real64 { self.f64() } else { Ok(self.f32()? as f64) }
    }
    fn reals(&mut self, n: usize) -> Result<Vec<f64>, ResError> {
        (0..n).map(|_| self.real()).collect()
    }
    fn ints(&mut self, n: usize) -> Result<Vec<i32>, ResError> {
        (0..n).map(|_| self.u32().map(|x| x as i32)).collect()
    }
    fn len(&mut self) -> Result<usize, ResError> {
        let n = (self.u32()? & 0x7fff_ffff) as usize;
        if n > self.b.len() {
            return err("length out of range");
        }
        Ok(n)
    }
    /// Length-prefixed UTF-8 string (the length counts the trailing NUL).
    fn ustr(&mut self) -> Result<String, ResError> {
        let n = self.u32()? as usize;
        let s = self.take(n)?;
        let end = s.iter().position(|c| *c == 0).unwrap_or(s.len());
        Ok(String::from_utf8_lossy(&s[..end]).into_owned())
    }
    /// String-table reference (or inline string when bit 31 is set).
    fn name(&mut self) -> Result<String, ResError> {
        let id = self.u32()?;
        if id & 0x8000_0000 != 0 {
            let n = (id & 0x7fff_ffff) as usize;
            let s = self.take(n)?;
            let end = s.iter().position(|c| *c == 0).unwrap_or(s.len());
            return Ok(String::from_utf8_lossy(&s[..end]).into_owned());
        }
        self.strings.get(id as usize).cloned().ok_or_else(|| ResError(format!("string id {id} out of range")))
    }
    fn pad(&mut self, n: usize) -> Result<(), ResError> {
        let extra = (4 - n % 4) % 4;
        self.take(extra).map(|_| ())
    }

    fn variant(&mut self) -> Result<Value, ResError> {
        let t = self.u32()?;
        Ok(match t {
            1 => Value::Nil,
            2 => Value::Bool(self.u32()? != 0),
            3 => Value::Int(self.u32()? as i32 as i64),
            4 => Value::Float(self.real()?),
            5 => Value::String(self.ustr()?),
            10 => Value::Reals(self.reals(2)?),
            11 => Value::Reals(self.reals(4)?),
            12 => Value::Reals(self.reals(3)?),
            13 | 14 => Value::Reals(self.reals(4)?),
            15 => Value::Reals(self.reals(6)?),
            16 => Value::Reals(self.reals(9)?),
            17 => Value::Reals(self.reals(12)?),
            18 => Value::Reals(self.reals(6)?),
            20 => Value::Color([self.f32()?, self.f32()?, self.f32()?, self.f32()?]),
            22 => {
                let names = self.u16()? as usize;
                let mut subs = self.u16()? as usize;
                let absolute = subs & 0x8000 != 0;
                subs &= 0x7fff;
                if self.ver_format < 3 {
                    subs += 1;
                }
                let mut s = String::new();
                if absolute {
                    s.push('/');
                }
                let mut parts = Vec::new();
                for _ in 0..names {
                    parts.push(self.name()?);
                }
                s.push_str(&parts.join("/"));
                for _ in 0..subs {
                    s.push(':');
                    s.push_str(&self.name()?);
                }
                Value::NodePath(s)
            }
            23 => {
                self.u32()?;
                Value::Rid
            }
            24 => Value::Object(match self.u32()? {
                0 => ObjRef::Empty,
                1 => {
                    let type_name = self.ustr()?;
                    let path = self.ustr()?;
                    ObjRef::External { type_name, path }
                }
                2 => ObjRef::Internal(self.u32()?),
                3 => ObjRef::ExternalIndex(self.u32()?),
                o => return err(format!("unknown object reference kind {o}")),
            }),
            26 => {
                let n = self.len()?;
                let mut d = Vec::with_capacity(n.min(4096));
                for _ in 0..n {
                    let k = self.variant()?;
                    let v = self.variant()?;
                    d.push((k, v));
                }
                Value::Dict(d)
            }
            30 => {
                let n = self.len()?;
                let mut a = Vec::with_capacity(n.min(1 << 16));
                for _ in 0..n {
                    a.push(self.variant()?);
                }
                Value::Array(a)
            }
            31 => {
                let n = self.len()?;
                let v = self.take(n)?.to_vec();
                self.pad(n)?;
                Value::Bytes(v)
            }
            32 => {
                let n = self.len()?;
                Value::Int32s(self.ints(n)?)
            }
            33 => {
                let n = self.len()?;
                Value::Float32s((0..n).map(|_| self.f32()).collect::<Result<_, _>>()?)
            }
            34 => {
                let n = self.len()?;
                Value::Strings((0..n).map(|_| self.ustr()).collect::<Result<_, _>>()?)
            }
            35 => {
                let n = self.len()?;
                Value::Vectors { dim: 3, data: self.reals(n * 3)? }
            }
            36 => {
                let n = self.len()?;
                let mut v = Vec::with_capacity(n);
                for _ in 0..n {
                    v.push([self.f32()?, self.f32()?, self.f32()?, self.f32()?]);
                }
                Value::Colors(v)
            }
            37 => {
                let n = self.len()?;
                Value::Vectors { dim: 2, data: self.reals(n * 2)? }
            }
            40 => Value::Int(self.u64()? as i64),
            41 => Value::Float(self.f64()?),
            42 => Value::Callable,
            43 => Value::Signal,
            44 => Value::StringName(self.ustr()?),
            45 => Value::Ints(self.ints(2)?),
            46 => Value::Ints(self.ints(4)?),
            47 => Value::Ints(self.ints(3)?),
            48 => {
                let n = self.len()?;
                Value::Int64s((0..n).map(|_| self.u64().map(|x| x as i64)).collect::<Result<_, _>>()?)
            }
            49 => {
                let n = self.len()?;
                Value::Float64s((0..n).map(|_| self.f64()).collect::<Result<_, _>>()?)
            }
            50 => Value::Reals(self.reals(4)?),
            51 => Value::Ints(self.ints(4)?),
            52 => Value::Reals(self.reals(16)?),
            53 => {
                let n = self.len()?;
                Value::Vectors { dim: 4, data: self.reals(n * 4)? }
            }
            other => return err(format!("unsupported variant type id {other}")),
        })
    }
}

/// Test helper / tooling: writes a minimal uncompressed `RSRC` file with one resource
/// (property values limited to the types the writer knows). Mirrors [`parse`].
pub fn write_simple(main_type: &str, script_class: &str, props: &[(&str, Value)]) -> Vec<u8> {
    let mut o = Vec::new();
    let u32_ = |o: &mut Vec<u8>, v: u32| o.extend_from_slice(&v.to_le_bytes());
    let ustr = |o: &mut Vec<u8>, s: &str| {
        o.extend_from_slice(&((s.len() + 1) as u32).to_le_bytes());
        o.extend_from_slice(s.as_bytes());
        o.push(0);
    };
    o.extend_from_slice(b"RSRC");
    for v in [0u32, 0, 4, 7, 6] {
        u32_(&mut o, v);
    }
    ustr(&mut o, main_type);
    o.extend_from_slice(&0u64.to_le_bytes());
    let flags = 2 | if script_class.is_empty() { 0 } else { 8 };
    u32_(&mut o, flags);
    o.extend_from_slice(&0x1234u64.to_le_bytes());
    if !script_class.is_empty() {
        ustr(&mut o, script_class);
    }
    for _ in 0..11 {
        u32_(&mut o, 0);
    }
    u32_(&mut o, props.len() as u32);
    for (k, _) in props {
        ustr(&mut o, k);
    }
    u32_(&mut o, 0); // ext
    u32_(&mut o, 1); // int
    ustr(&mut o, "local://1");
    let ofs_at = o.len();
    o.extend_from_slice(&0u64.to_le_bytes());
    let start = o.len() as u64;
    o[ofs_at..ofs_at + 8].copy_from_slice(&start.to_le_bytes());
    ustr(&mut o, main_type);
    u32_(&mut o, props.len() as u32);
    fn wv(o: &mut Vec<u8>, v: &Value) {
        let u = |o: &mut Vec<u8>, v: u32| o.extend_from_slice(&v.to_le_bytes());
        match v {
            Value::Nil => u(o, 1),
            Value::Bool(b) => {
                u(o, 2);
                u(o, *b as u32);
            }
            Value::Int(i) => {
                u(o, 40);
                o.extend_from_slice(&i.to_le_bytes());
            }
            Value::Float(f) => {
                u(o, 41);
                o.extend_from_slice(&f.to_le_bytes());
            }
            Value::Ints(v) if v.len() == 2 => {
                u(o, 45);
                for x in v {
                    u(o, *x as u32);
                }
            }
            Value::Array(a) => {
                u(o, 30);
                u(o, a.len() as u32);
                for e in a {
                    wv(o, e);
                }
            }
            Value::Bytes(b) => {
                u(o, 31);
                u(o, b.len() as u32);
                o.extend_from_slice(b);
                o.resize(o.len() + (4 - b.len() % 4) % 4, 0);
            }
            Value::Float32s(a) => {
                u(o, 33);
                u(o, a.len() as u32);
                for x in a {
                    o.extend_from_slice(&x.to_le_bytes());
                }
            }
            Value::Colors(a) => {
                u(o, 36);
                u(o, a.len() as u32);
                for c in a {
                    for x in c {
                        o.extend_from_slice(&x.to_le_bytes());
                    }
                }
            }
            Value::Object(ObjRef::Empty) => {
                u(o, 24);
                u(o, 0);
            }
            other => panic!("write_simple: unsupported value {other:?}"),
        }
    }
    for (i, (_, v)) in props.iter().enumerate() {
        u32_(&mut o, i as u32);
        wv(&mut o, v);
    }
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_simple_resource() {
        let props = vec![
            ("chunk_coords", Value::Ints(vec![-1, 2])),
            ("merge_mode", Value::Int(3)),
            ("height_map", Value::Array(vec![Value::Float(1.5), Value::Float(-2.0)])),
            ("ground_texture_idx", Value::Bytes(vec![1, 2, 3])),
            ("grass_mask_map", Value::Colors(vec![[1.0, 0.0, 0.0, 0.0]])),
            ("mesh", Value::Object(ObjRef::Empty)),
            ("flag", Value::Bool(true)),
        ];
        let b = write_simple("Resource", "MSTChunkData", &props);
        let f = parse(&b).unwrap();
        assert_eq!(f.script_class, "MSTChunkData");
        assert_eq!(f.version, (4, 7, 6));
        let m = f.main().unwrap();
        assert_eq!(m.type_name, "Resource");
        for (k, v) in &props {
            assert_eq!(m.get(k), Some(v), "{k}");
        }
        assert_eq!(m.get("height_map").unwrap().numbers(), Some(vec![1.5, -2.0]));
    }

    #[test]
    fn nested_arrays_flatten_row_major() {
        let v = Value::Array(vec![
            Value::Array(vec![Value::Float(1.0), Value::Int(2)]),
            Value::Array(vec![Value::Float(3.0), Value::Float(4.0)]),
        ]);
        assert_eq!(v.numbers(), Some(vec![1.0, 2.0, 3.0, 4.0]));
        assert_eq!(Value::Int(3).numbers(), None);
    }

    #[test]
    fn rejects_garbage_and_truncation() {
        assert!(parse(b"nope").is_err());
        let b = write_simple("Resource", "", &[("a", Value::Bytes(vec![9; 10]))]);
        assert!(parse(&b[..b.len() - 6]).is_err());
        assert!(parse(&b).is_ok());
    }

    #[test]
    fn rscc_blocks() {
        let mut b = b"RSCC".to_vec();
        for v in [2u32, 4, 6, 3, 2] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        b.extend_from_slice(b"abcde");
        let (mode, total, bs, blocks) = split_compressed(&b).unwrap();
        assert_eq!((mode, total, bs), (2, 6, 4));
        assert_eq!(blocks, vec![&b"abc"[..], &b"de"[..]]);
        assert_eq!((block_len(6, 4, 0), block_len(6, 4, 1)), (4, 2));
        assert!(join_blocks(vec![vec![0; 4], vec![0; 2]], 6).is_ok());
        assert!(join_blocks(vec![vec![0; 4]], 6).is_err());
    }

    /// Reads a real decompressed `MSTChunkData` when `PT_MST_RAW` points to one.
    #[test]
    fn real_file_if_available() {
        let Ok(p) = std::env::var("PT_MST_RAW") else { return };
        let b = std::fs::read(p).unwrap();
        let f = parse_decompressed(&b).unwrap();
        let m = f.main().unwrap();
        for (k, v) in &m.props {
            let s = format!("{v:?}");
            eprintln!("{k} = {}", &s[..s.len().min(120)]);
        }
    }
}
