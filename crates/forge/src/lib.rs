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
pub mod hkx;
pub mod index;
pub mod mesh;
pub mod navmesh;
pub mod shape;
pub mod skeleton;

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
const TEXDATA_TAG: u32 = 0x13237fe9;

/// A decoded top-level mip as RGBA8.
pub struct Texture {
    pub width: u32,
    pub height: u32,
    pub format: u32,
    pub rgba: Vec<u8>,
    /// The smaller mip levels that follow in the chain, RGBA, each half the size of the one before
    /// (as many as the data holds).
    pub mips: Vec<Vec<u8>>,
}

/// TextureMap body: u32 w, u32 h, u32 depth, u32 format, ... at +0x43 the data tag,
/// +0x47 u32 data size, +0x4B pixel data (full mip chain, top mip first).
pub fn decode_texture(body: &[u8]) -> Result<Texture> {
    let (width, height, format) = (u32_at(body, 0), u32_at(body, 4), u32_at(body, 12));
    ensure!(u32_at(body, 0x43) == TEXDATA_TAG, "texture data not inline");
    let size = u32_at(body, 0x47) as usize;
    let data = &body[0x4B..0x4B + size];
    let bc = match format {
        2 | 3 => Some(texpresso::Format::Bc1),
        4 => Some(texpresso::Format::Bc2),
        5 | 7 => Some(texpresso::Format::Bc3),
        _ => None,
    };
    let level_size = |w: usize, h: usize| bc.map_or(w * h * 4, |f| f.compressed_size(w, h));
    let decode = |src: &[u8], w: usize, h: usize| {
        let mut rgba = vec![0u8; w * h * 4];
        match bc {
            Some(f) => f.decompress(src, w, h, &mut rgba),
            None => {
                for (dst, s) in rgba.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
                    dst.copy_from_slice(&[s[2], s[1], s[0], s[3]]); // BGRA -> RGBA
                }
            }
        }
        rgba
    };
    let (mut w, mut h) = (width as usize, height as usize);
    let need = level_size(w, h);
    ensure!(data.len() >= need, "texture data short at {:#x}: {} < {need}", 0x4B, data.len());
    let rgba = decode(&data[..need], w, h);
    // The rest of the chain, while the data lasts.
    let (mut off, mut mips) = (need, vec![]);
    while w > 1 || h > 1 {
        (w, h) = ((w / 2).max(1), (h / 2).max(1));
        let n = level_size(w, h);
        if off + n > data.len() {
            break;
        }
        mips.push(decode(&data[off..off + n], w, h));
        off += n;
    }
    Ok(Texture { width, height, format, rgba, mips })
}

pub fn write_png(path: impl AsRef<Path>, t: &Texture) -> Result<()> {
    let file = std::io::BufWriter::new(File::create(path)?);
    let mut enc = png::Encoder::new(file, t.width, t.height);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()?.write_image_data(&t.rgba)?;
    Ok(())
}
