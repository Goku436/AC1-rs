//! Reader for Assassin's Creed (2007, PC) `.forge` archives.
//!
//! Layout (derived from the user's own install, forge version 25):
//! - `scimitar\0`, u32 version, u64 offset of the file-data header
//! - file-data header: u32 count, ... u64 offset of the index block
//! - index block: u32 count, u32, u64 index-table offset, u64 next,
//!   u32 first, u32 last, u64 name-table offset, u64 raw end
//! - index table: count x { u64 offset, u32 unknown, u32 size }
//! - name table: count x 0xBC bytes, name (NUL-terminated) at +0x2C
//! - each record at `offset`: "FILEDATA" + name, a metadata copy, then the payload

pub mod action;
pub mod anim;
pub mod animset;
pub mod datafile;
pub mod graph;
pub mod guidance;
pub mod hkx;
pub mod index;
pub mod lod;
pub mod material;
pub mod mesh;
pub mod navmesh;
pub mod shape;
pub mod skeleton;
pub mod sound;
pub mod texture;
pub mod world;

use anyhow::{Context, Result, bail, ensure};
use memmap2::Mmap;
use std::{fs::File, path::Path};

pub const COMPRESSED_MAGIC: u64 = 0x1004_FA99_57FB_AA33;
const NAME_ENTRY_SIZE: usize = 0xBC;

#[derive(Debug, Clone)]
pub struct Entry {
    pub name: String,
    pub offset: u64,
    pub size: u32,
    pub file_id: u64,
    pub type_hash: u32,
}

pub struct Forge {
    data: Mmap,
    pub version: u32,
    pub entries: Vec<Entry>,
}

pub(crate) fn u32_at(d: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(d[o..o + 4].try_into().unwrap())
}
pub(crate) fn u64_at(d: &[u8], o: usize) -> u64 {
    u64::from_le_bytes(d[o..o + 8].try_into().unwrap())
}
pub(crate) fn u16_at(d: &[u8], o: usize) -> u16 {
    u16::from_le_bytes(d[o..o + 2].try_into().unwrap())
}

impl Forge {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let file = File::open(path.as_ref()).with_context(|| format!("open {:?}", path.as_ref()))?;
        // SAFETY: read-only mapping of a file we don't modify.
        let data = unsafe { Mmap::map(&file)? };
        let d = &data[..];
        ensure!(d.starts_with(b"scimitar\0"), "not a forge archive");
        let version = u32_at(d, 9);
        let fdh = u64_at(d, 13) as usize;
        let count = u32_at(d, fdh) as usize;
        let index_block = u64_at(d, fdh + 0x20) as usize;
        let index_table = u64_at(d, index_block + 8) as usize;
        let name_table = u64_at(d, index_block + 0x20) as usize;
        ensure!(u32_at(d, index_block) as usize == count, "index count mismatch");

        let mut entries = Vec::with_capacity(count);
        for i in 0..count {
            let ie = index_table + i * 16;
            let ne = name_table + i * NAME_ENTRY_SIZE;
            let raw_name = &d[ne + 0x2C..ne + 0x2C + 128];
            let len = raw_name.iter().position(|&b| b == 0).unwrap_or(128);
            entries.push(Entry {
                name: String::from_utf8_lossy(&raw_name[..len]).into_owned(),
                offset: u64_at(d, ie),
                size: u32_at(d, ie + 12),
                file_id: u64_at(d, ne + 4),
                type_hash: u32_at(d, ne + 0x28),
            });
        }
        Ok(Self { data, version, entries })
    }

    /// Raw payload bytes of a record (still compressed if the record is compressed).
    pub fn raw(&self, e: &Entry) -> Result<&[u8]> {
        let d = &self.data[..];
        let base = e.offset as usize;
        ensure!(&d[base..base + 8] == b"FILEDATA", "record {} has no FILEDATA tag", e.name);
        // Metadata copy follows the 0x180-byte name area; its size field matches the index.
        let meta = base + 0x187;
        ensure!(u32_at(d, meta + 4) == e.size, "record {} metadata size mismatch", e.name);
        let start = meta + 0x31;
        Ok(&d[start..start + e.size as usize])
    }

    /// Fully decoded payload.
    pub fn read(&self, e: &Entry) -> Result<Vec<u8>> {
        decode_payload(self.raw(e)?)
    }
}

/// A payload is either plain bytes or one or more compressed streams back to back
/// (the first usually decodes to a small header, the second to the data).
pub fn decode_payload(raw: &[u8]) -> Result<Vec<u8>> {
    if raw.len() < 8 || u64_at(raw, 0) != COMPRESSED_MAGIC {
        return Ok(raw.to_vec());
    }
    let mut out = Vec::new();
    let mut pos = 0;
    while pos + 8 <= raw.len() && u64_at(raw, pos) == COMPRESSED_MAGIC {
        pos = decode_stream(raw, pos, &mut out)?;
    }
    Ok(out)
}

/// Stream: magic u64, u16 version, u8 method, u16 max chunk, u16 max chunk,
/// u16 chunk count, count x (u16 uncompressed, u16 compressed),
/// then per chunk: u32 checksum + compressed bytes.
fn decode_stream(raw: &[u8], mut pos: usize, out: &mut Vec<u8>) -> Result<usize> {
    pos += 8;
    let _ver = u16_at(raw, pos);
    let method = raw[pos + 2];
    let chunks = u16_at(raw, pos + 7) as usize;
    pos += 9;
    let sizes: Vec<(usize, usize)> = (0..chunks).map(|i| (u16_at(raw, pos + i * 4) as usize, u16_at(raw, pos + i * 4 + 2) as usize)).collect();
    pos += chunks * 4;
    for (usize_, csize) in sizes {
        // A 0 in the 16-bit field means a full 64 KiB.
        let usize_ = if usize_ == 0 { 0x10000 } else { usize_ };
        let csize = if csize == 0 { 0x10000 } else { csize };
        pos += 4; // checksum
        let src = &raw[pos..pos + csize];
        if csize == usize_ {
            out.extend_from_slice(src);
        } else {
            match method {
                1 | 2 | 5 => {
                    let buf =
                        lzokay_native::decompress(&mut std::io::Cursor::new(src), Some(usize_)).map_err(|e| anyhow::anyhow!("lzo method {method}: {e:?}"))?;
                    ensure!(buf.len() == usize_, "lzo produced {}, expected {usize_}", buf.len());
                    out.extend_from_slice(&buf);
                }
                m => bail!("unknown compression method {m}"),
            }
        }
        pos += csize;
    }
    Ok(pos)
}

/// One object inside a decoded data file.
#[derive(Debug, Clone)]
pub struct Object<'a> {
    pub id: u32,
    pub class: u32,
    pub name: String,
    /// Everything after the object's header (name, id, repeated class hash).
    pub body: &'a [u8],
    /// The whole object including header.
    pub bytes: &'a [u8],
}

/// Split a decoded data file into its objects: u16 count, count x {u32 id, u32 size},
/// then objects back to back, each `u32 class, u32 size, u32 name_len, name\0, u32 id, u32 class, body`.
pub fn parse_objects(d: &[u8]) -> Result<Vec<Object<'_>>> {
    ensure!(d.len() >= 2, "too short");
    let n = u16_at(d, 0) as usize;
    let mut pos = 2 + n * 8;
    ensure!(pos <= d.len(), "object table overruns data");
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let id = u32_at(d, 2 + i * 8);
        let size = u32_at(d, 6 + i * 8) as usize;
        ensure!(pos + size <= d.len(), "object {i} overruns data");
        let bytes = &d[pos..pos + size];
        let class = u32_at(bytes, 0);
        let name_len = u32_at(bytes, 8) as usize;
        let name = String::from_utf8_lossy(&bytes[12..12 + name_len.min(size.saturating_sub(12))]).into_owned();
        // After the name: a byte (0, or 1 when a block of extra properties follows), then the object's id and
        // class again, then the body. (Navigation meshes carry that block.)
        let after = 12 + name_len + 1;
        let mut tag = id.to_le_bytes().to_vec();
        tag.extend(class.to_le_bytes());
        let hdr = match bytes.get(after - 1) {
            Some(1) => bytes.get(after..).and_then(|b| b.windows(8).position(|w| w == tag)).map_or(after + 8, |k| after + k + 8),
            _ => after + 8,
        };
        out.push(Object { id, class, name, body: &bytes[hdr.min(size)..], bytes });
        pos += size;
    }
    Ok(out)
}

pub const CLASS_TEXTURE: u32 = 0xa2b7e917;

/// A decoded top-level mip as RGBA8.
pub struct Texture {
    pub width: u32,
    pub height: u32,
    pub format: u32,
    pub rgba: Vec<u8>,
    /// The smaller mip levels that follow in the chain, RGBA, each half the size of the one before
    /// (as many as the data holds).
    pub mips: Vec<Vec<u8>>,
    /// Its colour space and sampler (`texture::Sampling`).
    pub sampling: texture::Sampling,
    /// A cube map (type 2 at +0x10): the levels above are its first face's.
    pub cube: bool,
}

/// How a TextureMap's pixels are stored (its format at +0x0C): 2 and 3 DXT1 (BC1), 4 DXT3 (BC2), 5 and 7 DXT5 (BC3),
/// anything else (0) BGRA8.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PixelFormat {
    Bgra8,
    Bc1,
    Bc2,
    Bc3,
}

impl PixelFormat {
    pub fn of(format: u32) -> Self {
        match format {
            2 | 3 => Self::Bc1,
            4 => Self::Bc2,
            5 | 7 => Self::Bc3,
            _ => Self::Bgra8,
        }
    }

    fn bc(self) -> Option<texpresso::Format> {
        match self {
            Self::Bc1 => Some(texpresso::Format::Bc1),
            Self::Bc2 => Some(texpresso::Format::Bc2),
            Self::Bc3 => Some(texpresso::Format::Bc3),
            Self::Bgra8 => None,
        }
    }

    /// Bytes of one `w` x `h` level (block formats in whole 4x4 blocks).
    pub fn level_size(self, w: usize, h: usize) -> usize {
        self.bc().map_or(w * h * 4, |f| f.compressed_size(w, h))
    }

    /// A block-compressed format (4x4 blocks).
    pub fn is_block(self) -> bool {
        self != Self::Bgra8
    }
}

/// A TextureMap's pixels as stored: the top level and the smaller levels after it, each half the size of the one before
/// (as many as the data holds; a cube's are its first face's).
pub struct StoredTexture<'a> {
    pub width: u32,
    pub height: u32,
    /// The format field (+0x0C) as read.
    pub format: u32,
    pub pixels: PixelFormat,
    pub levels: Vec<&'a [u8]>,
    pub sampling: texture::Sampling,
    pub cube: bool,
}

/// The stored levels of a TextureMap body (see `texture` for the header fields): u32 w, u32 h, u32 depth, u32 format,
/// ... at +0x43 the data tag, +0x47 u32 data size, +0x4B pixel data (full mip chain, top mip first).
pub fn texture_levels(body: &[u8]) -> Result<StoredTexture<'_>> {
    let w32 = |o| texture::get_u32(body, o).unwrap_or(0);
    let (width, height, format) = (w32(0), w32(4), w32(12));
    ensure!((1..=8192).contains(&width) && (1..=8192).contains(&height), "texture size {width}x{height} at 0x0");
    let data = texture::pixel_data(body)?;
    let pixels = PixelFormat::of(format);
    let (mut w, mut h) = (width as usize, height as usize);
    let need = pixels.level_size(w, h);
    ensure!(data.len() >= need, "texture data short at {:#x}: {} < {need}", 0x4B, data.len());
    let mut levels = vec![&data[..need]];
    // The rest of the chain, while the data lasts.
    let mut off = need;
    while w > 1 || h > 1 {
        (w, h) = ((w / 2).max(1), (h / 2).max(1));
        let n = pixels.level_size(w, h);
        if off + n > data.len() {
            break;
        }
        levels.push(&data[off..off + n]);
        off += n;
    }
    Ok(StoredTexture { width, height, format, pixels, levels, sampling: texture::Sampling::read(body), cube: w32(0x10) == 2 })
}

/// Decode a TextureMap body to RGBA8 (`texture_levels`, each level decoded).
pub fn decode_texture(body: &[u8]) -> Result<Texture> {
    let t = texture_levels(body)?;
    let decode = |src: &[u8], w: usize, h: usize| {
        let mut rgba = vec![0u8; w * h * 4];
        match t.pixels.bc() {
            Some(f) => f.decompress(src, w, h, &mut rgba),
            None => {
                for (dst, s) in rgba.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
                    dst.copy_from_slice(&[s[2], s[1], s[0], s[3]]); // BGRA -> RGBA
                }
            }
        }
        rgba
    };
    let (mut w, mut h) = (t.width as usize, t.height as usize);
    let mut decoded = Vec::with_capacity(t.levels.len());
    for l in &t.levels {
        decoded.push(decode(l, w, h));
        (w, h) = ((w / 2).max(1), (h / 2).max(1));
    }
    let mut levels = decoded.into_iter();
    let rgba = levels.next().unwrap_or_default();
    Ok(Texture { width: t.width, height: t.height, format: t.format, rgba, mips: levels.collect(), sampling: t.sampling, cube: t.cube })
}

pub fn write_png(path: impl AsRef<Path>, t: &Texture) -> Result<()> {
    let file = std::io::BufWriter::new(File::create(path)?);
    let mut enc = png::Encoder::new(file, t.width, t.height);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()?.write_image_data(&t.rgba)?;
    Ok(())
}
