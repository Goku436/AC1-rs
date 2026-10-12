//! Bounds-checked little-endian reads for parsers that must not panic on bad data: every error names the offset.

use anyhow::{Context, Result, ensure};

pub(crate) fn u8_at(d: &[u8], o: usize) -> Result<u8> {
    d.get(o).copied().with_context(|| format!("data ends at {o:#x} (wanted a byte)"))
}

pub(crate) fn u32_at(d: &[u8], o: usize) -> Result<u32> {
    let b = d.get(o..o.checked_add(4).context("offset overflow")?).with_context(|| format!("data ends at {o:#x} (wanted a u32)"))?;
    Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// A finite f32.
pub(crate) fn f32_at(d: &[u8], o: usize) -> Result<f32> {
    let v = f32::from_bits(u32_at(d, o)?);
    ensure!(v.is_finite(), "not a finite float at {o:#x}");
    Ok(v)
}

pub(crate) fn vec3_at(d: &[u8], o: usize) -> Result<[f32; 3]> {
    Ok([f32_at(d, o)?, f32_at(d, o + 4)?, f32_at(d, o + 8)?])
}

pub(crate) fn vec4_at(d: &[u8], o: usize) -> Result<[f32; 4]> {
    Ok([f32_at(d, o)?, f32_at(d, o + 4)?, f32_at(d, o + 8)?, f32_at(d, o + 12)?])
}

/// Every offset where `class` is written as a u32.
pub(crate) fn find_u32(d: &[u8], class: u32) -> Vec<usize> {
    let tag = class.to_le_bytes();
    d.windows(4).enumerate().filter(|(_, w)| *w == tag).map(|(p, _)| p).collect()
}

/// The only offset where `class` is written, or an error naming what was looked for.
pub(crate) fn find_one(d: &[u8], class: u32, what: &str) -> Result<usize> {
    let at = find_u32(d, class);
    ensure!(at.len() == 1, "{what} (class {class:08x}) found {} times", at.len());
    Ok(at[0])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_past_the_end_are_errors() {
        let d = [1u8, 0, 0, 0, 0, 0, 0x80, 0x7f];
        assert_eq!(u32_at(&d, 0).unwrap(), 1);
        assert!(u32_at(&d, 5).is_err());
        assert!(u32_at(&d, usize::MAX - 1).is_err());
        assert!(f32_at(&d, 4).is_err(), "infinity is refused");
        assert!(u8_at(&d, 8).is_err());
        assert_eq!(find_u32(&d, 1), vec![0]);
        assert!(find_one(&d, 2, "x").is_err());
    }
}
