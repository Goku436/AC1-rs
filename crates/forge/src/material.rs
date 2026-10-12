//! Material objects (class 0x85c817c3, `Material`): the shader template a material uses and its typed parameter
//! table (scalars, colours and texture slots), which the character shader in the `game` crate reads.
//!
//! Body layout (from Banned445's AC1-Movement-Rewritten, MIT; checked over every material, `examples/texcheck`):
//! - +0x00 u32 template id: the shader template (which equations the material uses, see `Style`)
//! - +0x04 u32 texture set id (the TextureSet a slot in mode 0 reads)
//! - +0x08..+0x29 `?`
//! - +0x29 u32 `?` (0 in every material read here; the parse requires it)
//! - +0x2D u32 the inside material's id (0 for none): what the back faces of two-sided cloth show
//! - +0x31 u32 `?` (0 or 1)
//! - +0x35 u32 parameter count, then the parameters back to back, ending the body:
//!   `{u32 name CRC32, u32 class, u32 type (the kind in bits 16..22), value}`
//!   - kind 10, class 0: f32 (a scalar)
//!   - kind 13, class 0: f32 x4 (a colour, RGBA)
//!   - kind 19, class `TextureSelector`: u32 `?`, u32 class again, u32 mode, u32 channel, u32 set id, u32 map id: a
//!     texture slot. Mode 0 takes the map spec at `channel` of the material's own texture set, mode 1 of set `set`,
//!     mode 2 is map `map` itself.
//!
//! A TextureSet body is the map spec ids per channel (u32 each); a TextureMapSpec body ends with its TextureMap's id.

use crate::Object;
use crate::texture::get_u32;
use anyhow::{Result, bail, ensure};

pub const CLASS_TEXTURE_SELECTOR: u32 = 0x7d08_460d;
const PARAMS_AT: usize = 0x39;

/// A material parameter's value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Param {
    Scalar(f32),
    Color([f32; 4]),
    /// A texture slot (see the module docs).
    Map {
        mode: u32,
        channel: u32,
        set: u32,
        map: u32,
    },
}

#[derive(Debug, Clone)]
pub struct Material {
    pub template: u32,
    pub texture_set: u32,
    /// The material the back faces show (cloth), if any.
    pub inside: Option<u32>,
    /// (name CRC32, value), in the body's order.
    pub params: Vec<(u32, Param)>,
}

/// CRC32 of a parameter name (what the table is keyed by).
pub fn param_id(name: &str) -> u32 {
    crc32fast::hash(name.as_bytes())
}

/// The character shader templates: which lighting equations a material's template uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    /// Cloth (and alpha-tested cloth): ramp lighting, specular from the specular map, a rim.
    Cloth,
    /// The inside of cloth: as cloth, no specular.
    InsideCloth,
    /// Skin: a tiled specular mask and a multiply map.
    Skin,
    /// Weapons, throwing daggers, the mouth: specular with a Fresnel term.
    Fresnel,
    /// Eyes: an iris tint from the diffuse alpha, a reflection cube.
    Eye,
}

impl Style {
    /// The style of a template id (the ids are the templates' object ids in the data; from Banned445's
    /// AC1-Movement-Rewritten). `None` for templates that are not the character ones.
    pub fn of_template(template: u32) -> Option<Self> {
        Some(match template {
            433_482_562 | 2_055_988_768 => Self::Cloth,
            439_116_860 => Self::InsideCloth,
            433_482_821 => Self::Skin,
            3_292_696_482 | 661_746_657 | 439_311_561 => Self::Fresnel,
            326_205_072 => Self::Eye,
            _ => return None,
        })
    }

    /// The lighting ramp (a TextureGradient id, in DataPC's "Game Fix") the template lights with; eyes have none.
    pub fn ramp(self) -> Option<u32> {
        match self {
            Self::Eye => None,
            Self::Skin => Some(329_254_216),
            Self::Fresnel => Some(439_311_685),
            Self::Cloth | Self::InsideCloth => Some(433_482_686),
        }
    }
}

impl Material {
    /// Parse a Material body (see the module docs). Errors name the offset.
    pub fn parse(body: &[u8]) -> Result<Self> {
        let w = |o: usize| get_u32(body, o).ok_or_else(|| anyhow::anyhow!("material truncated at {o:#x} ({} bytes)", body.len()));
        let (template, texture_set) = (w(0)?, w(4)?);
        ensure!(w(0x29)? == 0, "material: {:#x} at 0x29 (not 0)", w(0x29)?);
        ensure!(w(0x31)? <= 1, "material: {:#x} at 0x31 (not 0 or 1)", w(0x31)?);
        let inside = Some(w(0x2D)?).filter(|&id| id != 0);
        let count = w(0x35)? as usize;
        ensure!(count <= 64, "material: {count} parameters at 0x35");
        let mut at = PARAMS_AT;
        let mut params = Vec::with_capacity(count);
        for _ in 0..count {
            let (id, class, kind) = (w(at)?, w(at + 4)?, (w(at + 8)? >> 16) & 63);
            at += 12;
            let (value, size) = match (kind, class) {
                (10, 0) => (Param::Scalar(f32::from_bits(w(at)?)), 4),
                (13, 0) => (Param::Color([w(at)?, w(at + 4)?, w(at + 8)?, w(at + 12)?].map(f32::from_bits)), 16),
                (19, CLASS_TEXTURE_SELECTOR) if w(at + 4)? == class => {
                    let (mode, channel) = (w(at + 8)?, w(at + 12)?);
                    ensure!(mode <= 2 && channel <= 8, "material: texture slot mode {mode} channel {channel} at {:#x}", at + 8);
                    (Param::Map { mode, channel, set: w(at + 16)?, map: w(at + 20)? }, 24)
                }
                _ => bail!("material: parameter type {kind} class {class:08x} at {:#x}", at - 4),
            };
            let finite = match value {
                Param::Scalar(v) => v.is_finite(),
                Param::Color(c) => c.iter().all(|v| v.is_finite()),
                Param::Map { .. } => true,
            };
            ensure!(finite, "material: a non-finite value at {at:#x}");
            params.push((id, value));
            at += size;
        }
        ensure!(at == body.len(), "material: {} bytes after the parameters at {at:#x}", body.len().saturating_sub(at));
        Ok(Self { template, texture_set, inside, params })
    }

    pub fn param(&self, id: u32) -> Option<Param> {
        self.params.iter().find(|(k, _)| *k == id).map(|(_, v)| *v)
    }

    pub fn scalar(&self, name: &str) -> Option<f32> {
        match self.param(param_id(name)) {
            Some(Param::Scalar(v)) => Some(v),
            _ => None,
        }
    }

    pub fn color(&self, name: &str) -> Option<[f32; 4]> {
        match self.param(param_id(name)) {
            Some(Param::Color(v)) => Some(v),
            _ => None,
        }
    }

    /// The first texture slot named one of `names`.
    pub fn slot(&self, names: &[&str]) -> Option<Param> {
        names.iter().find_map(|n| self.param(param_id(n)).filter(|p| matches!(p, Param::Map { .. })))
    }

    pub fn style(&self) -> Option<Style> {
        Style::of_template(self.template)
    }

    /// The texture (TextureMap or TextureGradient object) a slot names, with `get` looking objects up by id (in
    /// whichever data files hold them).
    pub fn resolve<'o>(&self, slot: Param, get: impl Fn(u32) -> Option<&'o Object<'o>>) -> Option<&'o Object<'o>> {
        let Param::Map { mode, channel, set, map } = slot else { return None };
        let id = match mode {
            0 | 1 => get_u32(get(if mode == 0 { self.texture_set } else { set })?.body, channel as usize * 4)?,
            _ => map,
        };
        let o = get(id)?;
        match o.class {
            crate::CLASS_TEXTURE | crate::texture::CLASS_GRADIENT => Some(o),
            crate::datafile::CLASS_MAP_SPEC => {
                let tex = get_u32(o.body, o.body.len().checked_sub(4)?)?;
                get(tex).filter(|t| t.class == crate::CLASS_TEXTURE)
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn material() -> Vec<u8> {
        let mut p = vec![0u8; PARAMS_AT];
        p[0..4].copy_from_slice(&433_482_562u32.to_le_bytes());
        p[0x31..0x35].copy_from_slice(&1u32.to_le_bytes());
        p[0x35..0x39].copy_from_slice(&2u32.to_le_bytes());
        for x in [param_id("SpecularPower"), 0, 10 << 16, 3.5f32.to_bits()] {
            p.extend(x.to_le_bytes());
        }
        for x in [param_id("SpecularTexture"), CLASS_TEXTURE_SELECTOR, 19 << 16, 7, CLASS_TEXTURE_SELECTOR, 2, 0, 0, 1234] {
            p.extend(x.to_le_bytes());
        }
        p
    }

    #[test]
    fn crc_of_the_selector_class() {
        assert_eq!(crc32fast::hash(b"TextureSelector"), CLASS_TEXTURE_SELECTOR);
    }

    #[test]
    fn parameters_are_read_from_the_typed_table() {
        let m = Material::parse(&material()).unwrap();
        assert_eq!(m.scalar("SpecularPower"), Some(3.5));
        assert_eq!(m.style(), Some(Style::Cloth));
        assert_eq!(m.slot(&["SpecularMap", "SpecularTexture"]), Some(Param::Map { mode: 2, channel: 0, set: 0, map: 1234 }));
        assert_eq!(m.inside, None);
    }

    #[test]
    fn truncated_or_bad_values_fail() {
        let p = material();
        for n in 0..p.len() {
            assert!(Material::parse(&p[..n]).is_err(), "{n} bytes");
        }
        let mut p = p;
        p[PARAMS_AT + 12..PARAMS_AT + 16].copy_from_slice(&f32::NAN.to_bits().to_le_bytes());
        assert!(Material::parse(&p).is_err());
    }
}
