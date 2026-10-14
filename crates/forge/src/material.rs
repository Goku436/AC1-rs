//! Material objects (class 0x85c817c3, `Material`): the shader template a material uses and its typed parameter
//! table (scalars, colours and texture slots), which the character shader in the `game` crate reads.
//!
//! Body layout (from Banned445's AC1-Movement-Rewritten, MIT; checked over every material, `examples/texcheck`):
//! - +0x00 u32 template id: the shader template (which equations the material uses, see `Style`)
//! - +0x04 u32 texture set id (the TextureSet a slot in mode 0 reads)
//! - +0x08 the `Mask` field (an inline object: u32 id 0, u32 class `Mask` df5d6c0e, then its body to +0x17 `?`)
//! - +0x17..+0x29 the render flags, a byte each in the order of Material's reflected property list (the exe's class
//!   descriptor; names by CRC32 of the exe's strings, checked against the materials named for them over every material,
//!   `examples/matflags`): +0x17 `BlendMode` (0 everywhere), +0x18 `AlphaDisplayMode`, +0x19 `InvertAlpha`,
//!   +0x1A `AlphaTestValue` (the alpha test's reference, 0..255), +0x1E 17aace0e (name not found; taken as "blended":
//!   set on exactly the materials drawn over what is behind them, ground blends and dirt trims `*_VA`, stains, blood,
//!   moss fading out, water, the hay's and beards' cards; 7 of 1145 distinct flag sets in Masyaf, 7 of 2405 in
//!   Damascus, `examples/matflags`), +0x20 `InvertCulling`, +0x25 `AlphaTestEnabled`,
//!   +0x26 `TwoSided`, +0x27 af654d1a (name not found; taken as "hidden": the one shipped material with it set is the
//!   unused `EyeIris`, and the NPC builders' tables set it to hide beards, hair and head parts: `No_Hair`,
//!   `Disabled_HeadOBJ`, `*_Beard_Dis_Mat`, `forge::build`); the rest `?` (bools). Leaves, hay, trellises and tarps
//!   test alpha; the value alone is not enough (windows carry one with the test off).
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
//!   - kind 19, class `UVTransform` (c52e2125): u32 `?`, u32 class again, f32 `ScaleU`, `ScaleV`, `TranslationU`,
//!     `TranslationV`, `TranslationSpeedU`, `TranslationSpeedV`, `Rotation`, `RotationSpeed`, u8 `AnimateTranslation`,
//!     u8 `AnimateRotation` (names by CRC32 from the exe's property list of `UVTransform`; the water's panners). The
//!     exe makes it a 2x3 matrix each frame, `u = cos r * su * x - sin r * sv * y + tu`, `v = sin r * su * x + cos r * sv
//!     * y + tv`, the translation (when animated) advancing by its speed times the frame time and wrapped to -1..1, the
//!     rotation by its speed (wrapped to a turn); `UvTransform::at`.
//!
//! A TextureSet body is the map spec ids per channel (u32 each); a TextureMapSpec body ends with its TextureMap's id.

use crate::Object;
use crate::texture::get_u32;
use anyhow::{Result, bail, ensure};

pub const CLASS_TEXTURE_SELECTOR: u32 = 0x7d08_460d;
pub const CLASS_UV_TRANSFORM: u32 = 0xc52e_2125;
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
    /// A texture coordinate transform (see the module docs).
    Uv(UvTransform),
}

/// A `UVTransform` parameter: how a shader's texture coordinates are scaled, turned and scrolled.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UvTransform {
    pub scale: [f32; 2],
    pub translation: [f32; 2],
    /// Per second, when `animate_translation`.
    pub translation_speed: [f32; 2],
    /// Radians.
    pub rotation: f32,
    /// Radians per second, when `animate_rotation`.
    pub rotation_speed: f32,
    pub animate_translation: bool,
    pub animate_rotation: bool,
}

impl UvTransform {
    /// The transform's matrix rows at `t` seconds: `(u, v) = (row0 . (x, y, 1), row1 . (x, y, 1))`. Animated, the
    /// translation starts from zero and wraps into -1..1 as the game's does each frame (whole tiles, the same picture).
    pub fn at(&self, t: f32) -> [[f32; 3]; 2] {
        let wrap = |x: f32| x - x.trunc();
        let (tu, tv) = if self.animate_translation {
            (wrap(self.translation_speed[0] * t), wrap(self.translation_speed[1] * t))
        } else {
            (self.translation[0], self.translation[1])
        };
        let r = if self.animate_rotation { (self.rotation_speed * t).rem_euclid(std::f32::consts::TAU) } else { self.rotation };
        let (s, c) = r.sin_cos();
        let [su, sv] = self.scale;
        [[c * su, -s * sv, tu], [s * su, c * sv, tv]]
    }
}

#[derive(Debug, Clone)]
pub struct Material {
    pub template: u32,
    pub texture_set: u32,
    /// The material the back faces show (cloth), if any.
    pub inside: Option<u32>,
    /// (name CRC32, value), in the body's order.
    pub params: Vec<(u32, Param)>,
    /// The alpha test's reference (0..1) when the material tests alpha (`AlphaTestEnabled`, `AlphaTestValue`).
    pub alpha_test: Option<f32>,
    /// Drawn from both sides (`TwoSided`).
    pub two_sided: bool,
    /// Not drawn (+0x27, see the module docs).
    pub hidden: bool,
    /// Blended over what is behind it by its alpha (+0x1E, see the module docs).
    pub blend: bool,
}

/// The offsets of the render flags read (see the module docs).
const ALPHA_TEST_VALUE: usize = 0x1A;
const ALPHA_TEST_ENABLED: usize = 0x25;
const TWO_SIDED: usize = 0x26;
const HIDDEN: usize = 0x27;
const BLEND: usize = 0x1E;
/// The CRC32 an NPC build table names the hidden flag by (`forge::build`).
pub const HIDDEN_PROPERTY: u32 = 0xaf65_4d1a;

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
    /// The townsfolk's clothes (`Character-Mask2`/`3`/`4` and their `-NoSpecular`, `Alpha` kinds): cloth whose
    /// diffuse is tinted by up to three colours (`Color_R`, `Color_G`, `Color_B`) through the channels of a mask map
    /// (`DiffuseMask`), then dirtied toward `Dirt`; the specular tinted the same way from white.
    Mask { channels: u8, specular: bool },
    /// NPC arms and legs (`Character-Arms`): the diffuse dirtied toward `Dirt_Color` by `Dirt_Mask`, times
    /// `saturate(MultiplyMask + SkinColor)`; the specular map's red the highlight's strength, its green a fifth of its power.
    Arms,
    /// NPC heads (`Character-GenericHead`): the diffuse times `HeadMultiply`; the specular map's red (x 1.5) the highlight's
    /// strength, its green a tenth of its power, the highlight tinted by the diffuse's red.
    Head,
    /// NPC hair and beards (`Character-Hair`, `Character-Hair_Alpha`): the diffuse times `HairColor`, the highlight the
    /// diffuse times `HairSpecular`; plain N.L, no ramp.
    Hair,
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
            0x3201_395a | 0x53f6_d750 | 0xb4dd_70cb => Self::Mask { channels: 3, specular: true },
            0x4542_17b0 | 0x4dc4_fe7a => Self::Mask { channels: 2, specular: true },
            0x4542_189d => Self::Mask { channels: 1, specular: true },
            0x3201_2c2b => Self::Mask { channels: 3, specular: false },
            0x4542_1983 | 0x4dc4_fb05 => Self::Mask { channels: 2, specular: false },
            0x4542_1a31 => Self::Mask { channels: 1, specular: false },
            0x1b43_0ea3 => Self::Arms,
            0x018a_1172 => Self::Head,
            0x2d29_5631 | 0xdfa4_3f8c => Self::Hair,
            _ => return None,
        })
    }

    /// Whether template `template` takes its alpha from a map (`AlphaTexture` dotted with `AlphaVector`): the masks'
    /// and hair's `Alpha` kinds (their shaders' output alpha).
    pub fn alpha_from_map(template: u32) -> bool {
        matches!(template, 0x4dc4_fe7a | 0x53f6_d750 | 0xb4dd_70cb | 0x4dc4_fb05 | 0xdfa4_3f8c)
    }

    /// The lighting ramp (a TextureGradient id, in DataPC's "Game Fix") the template lights with; eyes have none.
    pub fn ramp(self) -> Option<u32> {
        match self {
            Self::Eye => None,
            Self::Skin => Some(329_254_216),
            Self::Fresnel => Some(439_311_685),
            Self::Cloth | Self::InsideCloth | Self::Mask { .. } => Some(433_482_686),
            Self::Arms | Self::Head => Some(329_254_216),
            Self::Hair => None,
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
        let alpha_test = (body[ALPHA_TEST_ENABLED] != 0).then(|| body[ALPHA_TEST_VALUE] as f32 / 255.0);
        let two_sided = body[TWO_SIDED] != 0;
        let hidden = body[HIDDEN] != 0;
        let blend = body[BLEND] != 0;
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
                (19, CLASS_UV_TRANSFORM) if w(at + 4)? == class => {
                    let f = |k: usize| w(at + 8 + 4 * k).map(f32::from_bits);
                    ensure!(body.len() >= at + 42, "material: UVTransform truncated at {at:#x}");
                    let uv = UvTransform {
                        scale: [f(0)?, f(1)?],
                        translation: [f(2)?, f(3)?],
                        translation_speed: [f(4)?, f(5)?],
                        rotation: f(6)?,
                        rotation_speed: f(7)?,
                        animate_translation: body[at + 40] != 0,
                        animate_rotation: body[at + 41] != 0,
                    };
                    (Param::Uv(uv), 42)
                }
                _ => bail!("material: parameter type {kind} class {class:08x} at {:#x}", at - 4),
            };
            let finite = match value {
                Param::Scalar(v) => v.is_finite(),
                Param::Color(c) => c.iter().all(|v| v.is_finite()),
                Param::Map { .. } => true,
                Param::Uv(u) => [u.scale, u.translation, u.translation_speed, [u.rotation, u.rotation_speed]].iter().flatten().all(|v| v.is_finite()),
            };
            ensure!(finite, "material: a non-finite value at {at:#x}");
            params.push((id, value));
            at += size;
        }
        ensure!(at == body.len(), "material: {} bytes after the parameters at {at:#x}", body.len().saturating_sub(at));
        Ok(Self { template, texture_set, inside, params, alpha_test, two_sided, hidden, blend })
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

    pub fn uv_transform(&self, name: &str) -> Option<UvTransform> {
        match self.param(param_id(name)) {
            Some(Param::Uv(u)) => Some(u),
            _ => None,
        }
    }

    pub fn style(&self) -> Option<Style> {
        Style::of_template(self.template)
    }

    /// The lighting ramp the material's template lights with (the style's; `Character-Mask4Alpha-Vertex color` takes
    /// the skin's, its template names that one).
    pub fn ramp(&self) -> Option<u32> {
        if self.template == 0xb4dd_70cb {
            return Some(329_254_216);
        }
        self.style()?.ramp()
    }

    /// Set parameter `id` (an NPC builder's choice, `forge::build`): replaced where the material has it, added where not.
    pub fn set(&mut self, id: u32, value: Param) {
        match self.params.iter_mut().find(|(k, _)| *k == id) {
            Some(p) => p.1 = value,
            None => self.params.push((id, value)),
        }
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
    fn render_flags_are_read() {
        let mut p = material();
        assert_eq!(Material::parse(&p).unwrap().alpha_test, None);
        // (A value with the test off, as the windows have: no test.)
        p[ALPHA_TEST_VALUE] = 0x80;
        assert_eq!(Material::parse(&p).unwrap().alpha_test, None);
        p[ALPHA_TEST_ENABLED] = 1;
        p[TWO_SIDED] = 1;
        let m = Material::parse(&p).unwrap();
        assert_eq!(m.alpha_test, Some(128.0 / 255.0));
        assert!(m.two_sided);
        // (The names, by the CRC32s in the exe's property list of Material.)
        for (name, crc) in [("AlphaTestValue", 0x892e_2ecb), ("AlphaTestEnabled", 0x7abb_06a6), ("TwoSided", 0x4035_25b2)] {
            assert_eq!(crc32fast::hash(name.as_bytes()), crc, "{name}");
        }
    }

    #[test]
    fn uv_transforms_are_read() {
        let mut p = material();
        p[0x35..0x39].copy_from_slice(&3u32.to_le_bytes());
        for x in [param_id("Wave01"), CLASS_UV_TRANSFORM, 19 << 16, 0, CLASS_UV_TRANSFORM] {
            p.extend(x.to_le_bytes());
        }
        for x in [0.1f32, 0.2, 0.0, 0.0, -0.02, 0.03, 0.0, 0.0] {
            p.extend(x.to_le_bytes());
        }
        p.extend([1, 0]);
        let m = Material::parse(&p).unwrap();
        let u = m.uv_transform("Wave01").unwrap();
        assert_eq!((u.scale, u.translation_speed, u.animate_translation, u.animate_rotation), ([0.1, 0.2], [-0.02, 0.03], true, false));
        let r = u.at(10.0);
        assert!((r[0][0] - 0.1).abs() < 1e-6 && (r[1][1] - 0.2).abs() < 1e-6 && (r[0][2] + 0.2).abs() < 1e-5 && (r[1][2] - 0.3).abs() < 1e-5);
        // (Wrapped into -1..1 as the game does.)
        assert!((u.at(60.0)[0][2] + 0.2).abs() < 1e-4);
        assert!(Material::parse(&p[..p.len() - 1]).is_err());
        assert_eq!(crc32fast::hash(b"UVTransform"), CLASS_UV_TRANSFORM);
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
