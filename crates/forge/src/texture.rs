//! TextureMap header fields beyond the pixels, and the two texture classes characters' shaders also use: the
//! lighting ramps (`TextureGradient`) and the eyes' reflection cube (a TextureMap of type 2).
//!
//! TextureMap body (offsets from the body, after the object header):
//! - +0x00 u32 width, +0x04 height, +0x08 depth, +0x0C pixel format (0 BGRA8, 2/3 DXT1, 4 DXT3, 5/7 DXT5)
//! - +0x10 u32 type: 1 a 2D map, 2 a cube (six faces)
//! - +0x14 u32 `GammaSettings`: 1 sRGB (diffuse maps), 0 linear (normal, specular and mask maps)
//! - +0x18 u32 mip count
//! - +0x1C / +0x20 u32 `AddressMode` U / V: 0 wrap, 1 mirror, 2 clamp, 3 border, 4 mirror once
//! - +0x24 `?`
//! - +0x28 / +0x2C / +0x30 u32 `FilterType` mag / min / mip: 0 point, 1 linear, 2 none
//! - +0x43 data tag 0x13237fe9, +0x47 u32 data size, +0x4B the pixels: each face's full mip chain, face after face
//!
//! The sampling fields' names and meanings are from Banned445's AC1-Movement-Rewritten (MIT); the histogram over every
//! texture in the install (`examples/texcheck`) bears them out: diffuse maps are sRGB, normal and specular maps
//! linear, and the ramps clamp along their lighting axis.
//!
//! TextureGradient body: the TextureMap layout with BGRA8 pixels, one mip, type 1.

use anyhow::{Result, ensure};

/// Class id of `TextureGradient` (CRC32 of the name).
pub const CLASS_GRADIENT: u32 = 0x245f_5ca5;
pub(crate) const DATA_TAG: u32 = 0x1323_7fe9;
pub(crate) const DATA_AT: usize = 0x4B;

/// Bounds-checked little-endian u32 (`None` past the end).
pub(crate) fn get_u32(d: &[u8], o: usize) -> Option<u32> {
    d.get(o..o.checked_add(4)?).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// How a texture is sampled: its colour space and sampler.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Sampling {
    /// `GammaSettings` is sRGB: the texels are decoded to linear when sampled.
    pub srgb: bool,
    /// `AddressMode` U, V (0 wrap, 1 mirror, 2 clamp, 3 border, 4 mirror once).
    pub address: [u32; 2],
    /// `FilterType` mag, min, mip (0 point, 1 linear, 2 none).
    pub filter: [u32; 3],
}

impl Sampling {
    /// The fields of a TextureMap (or TextureGradient) body; zeros where the body is short.
    pub fn read(body: &[u8]) -> Self {
        let w = |o| get_u32(body, o).unwrap_or(0);
        Self { srgb: w(0x14) == 1, address: [w(0x1C), w(0x20)], filter: [w(0x28), w(0x2C), w(0x30)] }
    }
}

/// The pixel data of a body: (offset, bytes), checked against the body's length.
pub(crate) fn pixel_data(body: &[u8]) -> Result<&[u8]> {
    ensure!(get_u32(body, 0x43) == Some(DATA_TAG), "texture data not inline (no tag at 0x43)");
    let size = get_u32(body, 0x47).unwrap_or(0) as usize;
    body.get(DATA_AT..DATA_AT + size).ok_or_else(|| anyhow::anyhow!("texture data at {DATA_AT:#x}: {size} bytes, the body has {}", body.len()))
}

/// A lighting ramp: `width` x `height` RGBA8 (one mip).
pub struct Gradient {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub sampling: Sampling,
}

/// Decode a TextureGradient body.
pub fn decode_gradient(body: &[u8]) -> Result<Gradient> {
    let (w, h) = (get_u32(body, 0).unwrap_or(0), get_u32(body, 4).unwrap_or(0));
    ensure!((1..=4096).contains(&w) && (1..=4096).contains(&h), "gradient size {w}x{h} at 0x0");
    ensure!(get_u32(body, 0x0C) == Some(0), "gradient format {:?} at 0xc (not BGRA8)", get_u32(body, 0x0C));
    let data = pixel_data(body)?;
    let n = (w * h * 4) as usize;
    ensure!(data.len() >= n, "gradient data at {DATA_AT:#x}: {} < {n}", data.len());
    let rgba = data[..n].chunks_exact(4).flat_map(|s| [s[2], s[1], s[0], s[3]]).collect();
    Ok(Gradient { width: w, height: h, rgba, sampling: Sampling::read(body) })
}

/// A cube map: `size` square faces, RGBA8, mip-major (each mip's six faces in D3D order +X -X +Y -Y +Z -Z).
pub struct Cube {
    pub size: u32,
    pub mips: Vec<Vec<u8>>,
}

/// Decode a cube TextureMap (type 2, DXT1/3/5 faces).
pub fn decode_cube(body: &[u8]) -> Result<Cube> {
    let w = |o| get_u32(body, o).unwrap_or(0);
    let (size, format, count) = (w(0), w(0x0C), w(0x18) as usize);
    ensure!(w(0x10) == 2, "texture type {} at 0x10 (not a cube)", w(0x10));
    ensure!((1..=4096).contains(&size) && w(4) == size, "cube faces {size}x{} at 0x0", w(4));
    ensure!((1..=13).contains(&count), "cube mip count {count} at 0x18");
    let f = match format {
        2 | 3 => texpresso::Format::Bc1,
        4 => texpresso::Format::Bc2,
        5 | 7 => texpresso::Format::Bc3,
        _ => anyhow::bail!("cube format {format} at 0xc"),
    };
    let side = |m: usize| (size as usize >> m).max(1);
    let lengths: Vec<usize> = (0..count).map(|m| f.compressed_size(side(m), side(m))).collect();
    let chain: usize = lengths.iter().sum();
    let data = pixel_data(body)?;
    ensure!(data.len() >= chain * 6, "cube data at {DATA_AT:#x}: {} < 6 x {chain}", data.len());
    let mut mips = Vec::with_capacity(count);
    let mut offset = 0;
    for (m, &bytes) in lengths.iter().enumerate() {
        let s = side(m);
        let mut mip = vec![0u8; s * s * 4 * 6];
        for face in 0..6 {
            let src = &data[face * chain + offset..face * chain + offset + bytes];
            f.decompress(src, s, s, &mut mip[face * s * s * 4..(face + 1) * s * s * 4]);
        }
        mips.push(mip);
        offset += bytes;
    }
    Ok(Cube { size, mips })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(w: u32, h: u32, format: u32, kind: u32, mips: u32, data: &[u8]) -> Vec<u8> {
        let mut b = vec![0u8; DATA_AT];
        for (at, v) in
            [(0, w), (4, h), (0x0C, format), (0x10, kind), (0x14, 1), (0x18, mips), (0x1C, 2), (0x28, 1), (0x43, DATA_TAG), (0x47, data.len() as u32)]
        {
            b[at..at + 4].copy_from_slice(&v.to_le_bytes());
        }
        b.extend_from_slice(data);
        b
    }

    #[test]
    fn crc_of_the_gradient_class() {
        assert_eq!(crc32fast::hash(b"TextureGradient"), CLASS_GRADIENT);
    }

    #[test]
    fn gradient_is_bgra_and_truncation_fails() {
        let b = body(1, 1, 0, 1, 1, &[10, 20, 30, 40]);
        let g = decode_gradient(&b).unwrap();
        assert_eq!(g.rgba, [30, 20, 10, 40]);
        assert_eq!(g.sampling, Sampling { srgb: true, address: [2, 0], filter: [1, 0, 0] });
        for n in 0..b.len() {
            assert!(decode_gradient(&b[..n]).is_err());
        }
    }

    #[test]
    fn cube_faces_keep_their_order_at_every_mip() {
        // 4x4 faces, two mips, DXT1 (8 bytes a block): a face's chain is 16 bytes.
        let mut data = vec![];
        for face in 0..6u8 {
            for mip in 0..2u8 {
                // A block of one colour: colour 0 = colour 1 = a red of face * 2 + mip.
                let c = (((face * 2 + mip) as u16) << 11).to_le_bytes();
                data.extend([c[0], c[1], c[0], c[1], 0, 0, 0, 0]);
            }
        }
        let b = body(4, 4, 2, 2, 2, &data);
        let cube = decode_cube(&b).unwrap();
        for (mip, pixels) in cube.mips.iter().enumerate() {
            let face_bytes = (4usize >> mip).pow(2) * 4;
            for face in 0..6 {
                let red = ((face * 2 + mip) as u32 * 255 / 31) as i32;
                assert!((pixels[face * face_bytes] as i32 - red).abs() <= 8, "face {face} mip {mip}");
            }
        }
        for n in 0..b.len() {
            assert!(decode_cube(&b[..n]).is_err());
        }
    }
}
