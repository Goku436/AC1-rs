//! A level's lighting and colour grade: `WorldAmbiance` (21c2d472) and the `LayeredSky` (24d32cf8) it links.
//!
//! One `WorldAmbiance` per level, in the level's own data file (`Damascus` in `DataPC_Damascus.forge`, ...). The class
//! layouts and field names come from the executable's reflection tables (class descriptors, property lists whose names
//! are CRC32s, matched against candidate words); the byte layouts below are checked on all 14 levels (`examples/ambcheck`).
//! Offsets are from the object's body (after its header's id and class).
//!
//! `WorldAmbiance` body:
//! - +0x00 u32 the `LayeredSky`'s id
//! - an inline `WorldFog` (u32 id, u32 class acf73d89, at +0x04): from its class: +4 u32 `?`, +8 u8 `Enabled`, +9 u32 `?`,
//!   +13 `FogColor` (u32 class a79767ed `Color`, f32 RGBA), +33 f32 `FogIntensity`, +37 `FogDensity`, +41 `FogNear`, +45
//!   `FogFar`
//! - an inline `PostEffects` (c83b8907): one inline settings object per effect (each u32 id, u32 class, u8 enabled, ...),
//!   among them `ColourControlSettings` (583f4578: f32 `Hue`, `Saturation`, `Brightness`, `Contrast`),
//!   `ColorBalanceSettings` (a8820575: 9 f32 colour shifts, then `BlackLevel`, `WhiteLevel`, `Gamma`, `Opacity`) and
//!   `GlowEffectSettings` (93ddeac0, no fields); others (blur, glitches, depth of field, ...) are not read.
//!
//! `LayeredSky` body:
//! - +0x00 u32 `?`, +0x04 f32 `SunLightAzimuth`, +0x08 f32 `SunLightElevation` (degrees), +0x0C u32 `?`
//! - +0x10 `SunLightColor` (u32 class f8206af7 `IColor`, u8 B, G, R, A), +0x18 f32 `SunLightIntensity`
//! - +0x1C u8 `UseCharacterSunLight`, +0x1D u32 `?`, +0x21 `CharacterSunLightColor` (`IColor`), +0x29 f32
//!   `CharacterSunLightIntensity`, +0x2D `CharacterSunLightAzimuth`, +0x31 `CharacterSunLightElevation`, then three f32
//!   `?` and the sun flare, the cloud shadows (not read)
//! - u8 `UseCharacterAmbient`, then two inline `AmbientCube`s (u32 id, u32 class b3c3604c): `EnvironmentAmbient` and
//!   `CharacterAmbient`, each `Right`, `Left`, `Front`, `Back`, `Top`, `Down` (f32 RGBA: +X, -X, +Y, -Y, +Z, -Z of the
//!   game's Z-up world), `Tint` (f32 RGBA), f32 `Saturation`, `Scale`, `Bias`, `NormalTopBendFactor`; then the sky's
//!   layers.
//!
//! How the game uses them (its DX9 shaders, in the `MaterialTemplate`s and the executable): the world's pixel shaders
//! light a surface by the ambient cube (each face weighted by the squared normal component on its side, the normal's Z
//! first scaled by `NormalTopBendFactor`), times the baked occlusion (`forge::instance`), plus the sun (N.L, shadows)
//! and blend to `FogColor` by `saturate((distance - near) * scale) * intensity`. The post effects are baked into a
//! 16x16x16 colour table each frame: per channel `BlackLevel`/`WhiteLevel`/`Gamma` levels and the colour shifts (in
//! shadows, midtones and highlights), mixed with the original by `Opacity`, then a matrix: brightness, contrast about
//! 0.5, saturation (luminance weights 0.3086, 0.6094, 0.082) and hue.

use anyhow::{Context, Result, ensure};

pub const CLASS_WORLD_AMBIANCE: u32 = 0x21c2_d472;
pub const CLASS_LAYERED_SKY: u32 = 0x24d3_2cf8;
const CLASS_WORLD_FOG: u32 = 0xacf7_3d89;
const CLASS_COLOR: u32 = 0xa797_67ed;
const CLASS_ICOLOR: u32 = 0xf820_6af7;
const CLASS_AMBIENT_CUBE: u32 = 0xb3c3_604c;
const CLASS_COLOUR_CONTROL: u32 = 0x583f_4578;
const CLASS_COLOR_BALANCE: u32 = 0xa882_0575;
const CLASS_GLOW: u32 = 0x93dd_eac0;

fn u32_at(d: &[u8], p: usize) -> Result<u32> {
    let s = d.get(p..p + 4).with_context(|| format!("short at {p:#x}"))?;
    Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn f32_at(d: &[u8], p: usize) -> Result<f32> {
    let v = f32::from_bits(u32_at(d, p)?);
    ensure!(v.is_finite(), "not a number at {p:#x}");
    Ok(v)
}

fn u8_at(d: &[u8], p: usize) -> Result<u8> {
    d.get(p).copied().with_context(|| format!("short at {p:#x}"))
}

/// The first place `class` appears (as a u32) at or after `from`.
fn find(d: &[u8], class: u32, from: usize) -> Option<usize> {
    let tag = class.to_le_bytes();
    d.get(from..)?.windows(4).position(|w| w == tag).map(|p| p + from)
}

/// An `IColor` (class, then B, G, R, A bytes) as RGB 0..1.
fn icolor(d: &[u8], p: usize) -> Result<[f32; 3]> {
    ensure!(u32_at(d, p)? == CLASS_ICOLOR, "no IColor at {p:#x}");
    let c = d.get(p + 4..p + 8).with_context(|| format!("short IColor at {p:#x}"))?;
    Ok([c[2] as f32 / 255.0, c[1] as f32 / 255.0, c[0] as f32 / 255.0])
}

/// A `Color` (class, then f32 RGBA).
fn color(d: &[u8], p: usize) -> Result<[f32; 4]> {
    ensure!(u32_at(d, p)? == CLASS_COLOR, "no Color at {p:#x}");
    Ok([f32_at(d, p + 4)?, f32_at(d, p + 8)?, f32_at(d, p + 12)?, f32_at(d, p + 16)?])
}

/// `AmbientCube`: the light from each side of the game's Z-up world.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AmbientCube {
    /// +X, -X, +Y, -Y, +Z (top), -Z (down), RGB.
    pub faces: [[f32; 3]; 6],
    pub tint: [f32; 3],
    pub saturation: f32,
    pub scale: f32,
    pub bias: f32,
    /// The normal's Z is scaled by this before the lookup (more: the top colour reaches further down).
    pub top_bend: f32,
}

impl AmbientCube {
    /// The body after the cube's class at `p`.
    fn parse(d: &[u8], p: usize) -> Result<Self> {
        let f = |k: usize| f32_at(d, p + 4 + 4 * k);
        let mut faces = [[0.0; 3]; 6];
        for (i, face) in faces.iter_mut().enumerate() {
            *face = [f(4 * i)?, f(4 * i + 1)?, f(4 * i + 2)?];
        }
        Ok(Self { faces, tint: [f(24)?, f(25)?, f(26)?], saturation: f(28)?, scale: f(29)?, bias: f(30)?, top_bend: f(31)? })
    }

    /// The faces as the shaders get them. The game builds them from the stored faces with `Tint`, `Saturation`,
    /// `Scale` and `Bias` (how exactly is not found yet: `?`); here each face is desaturated toward its luminance by
    /// `Saturation`, tinted, scaled and biased, the identity for the environment cubes (tint 1, saturation 1, scale 1,
    /// bias 0).
    pub fn shader_faces(&self) -> [[f32; 3]; 6] {
        self.faces.map(|c| {
            let l = c[0] * 0.3086 + c[1] * 0.6094 + c[2] * 0.082;
            std::array::from_fn(|k| ((l + (c[k] - l) * self.saturation) * self.tint[k] * self.scale + self.bias).max(0.0))
        })
    }
}

/// A sun: where it is (degrees; azimuth from the game's +X toward +Y, elevation above the horizon), its colour and
/// intensity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sun {
    pub azimuth: f32,
    pub elevation: f32,
    pub color: [f32; 3],
    pub intensity: f32,
}

impl Sun {
    /// The direction toward the sun in the game's Z-up world: the azimuth turns clockwise seen from above, from +X toward
    /// -Y (matched against the running game's shadows in Damascus: Damascus' 115 degrees lights the south faces).
    pub fn toward(&self) -> [f32; 3] {
        let (a, e) = (self.azimuth.to_radians(), self.elevation.to_radians());
        [a.sin() * e.cos(), a.cos() * e.cos(), e.sin()]
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fog {
    pub enabled: bool,
    pub color: [f32; 3],
    pub intensity: f32,
    pub density: f32,
    pub near: f32,
    pub far: f32,
}

/// `ColourControlSettings`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColourControl {
    pub enabled: bool,
    /// Degrees.
    pub hue: f32,
    pub saturation: f32,
    pub brightness: f32,
    pub contrast: f32,
}

/// `ColorBalanceSettings`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColorBalance {
    pub enabled: bool,
    /// Per channel (R, G, B): the shift in the highlights, the midtones and the shadows.
    pub shifts: [[f32; 3]; 3],
    pub black: f32,
    pub white: f32,
    pub gamma: f32,
    pub opacity: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Ambiance {
    pub sky: u32,
    pub fog: Fog,
    pub colour_control: ColourControl,
    pub color_balance: ColorBalance,
    pub glow: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Sky {
    pub sun: Sun,
    /// The characters' own sun, when `use_character_sun`.
    pub use_character_sun: bool,
    pub character_sun: Sun,
    pub use_character_ambient: bool,
    pub environment_ambient: AmbientCube,
    pub character_ambient: AmbientCube,
}

impl Ambiance {
    /// A `WorldAmbiance` object's body.
    pub fn parse(b: &[u8]) -> Result<Self> {
        let sky = u32_at(b, 0)?;
        let f = find(b, CLASS_WORLD_FOG, 4).context("no WorldFog")?;
        let fog = Fog {
            enabled: u8_at(b, f + 8)? != 0,
            color: color(b, f + 13).map(|c| [c[0], c[1], c[2]])?,
            intensity: f32_at(b, f + 33)?,
            density: f32_at(b, f + 37)?,
            near: f32_at(b, f + 41)?,
            far: f32_at(b, f + 45)?,
        };
        let c = find(b, CLASS_COLOUR_CONTROL, f).context("no ColourControlSettings")?;
        let colour_control = ColourControl {
            enabled: u8_at(b, c + 4)? != 0,
            hue: f32_at(b, c + 5)?,
            saturation: f32_at(b, c + 9)?,
            brightness: f32_at(b, c + 13)?,
            contrast: f32_at(b, c + 17)?,
        };
        let k = find(b, CLASS_COLOR_BALANCE, c).context("no ColorBalanceSettings")?;
        let v = |i: usize| f32_at(b, k + 5 + 4 * i);
        let color_balance = ColorBalance {
            enabled: u8_at(b, k + 4)? != 0,
            shifts: [[v(0)?, v(1)?, v(2)?], [v(3)?, v(4)?, v(5)?], [v(6)?, v(7)?, v(8)?]],
            black: v(9)?,
            white: v(10)?,
            gamma: v(11)?,
            opacity: v(12)?,
        };
        let g = find(b, CLASS_GLOW, k).context("no GlowEffectSettings")?;
        Ok(Self { sky, fog, colour_control, color_balance, glow: u8_at(b, g + 4)? != 0 })
    }
}

impl Sky {
    /// A `LayeredSky` object's body.
    pub fn parse(b: &[u8]) -> Result<Self> {
        let sun = Sun { azimuth: f32_at(b, 0x04)?, elevation: f32_at(b, 0x08)?, color: icolor(b, 0x10)?, intensity: f32_at(b, 0x18)? };
        let character_sun = Sun { color: icolor(b, 0x21)?, intensity: f32_at(b, 0x29)?, azimuth: f32_at(b, 0x2D)?, elevation: f32_at(b, 0x31)? };
        let e = find(b, CLASS_AMBIENT_CUBE, 0x35).context("no EnvironmentAmbient")?;
        let c = find(b, CLASS_AMBIENT_CUBE, e + 4).context("no CharacterAmbient")?;
        // (`UseCharacterAmbient` sits before the first cube's id.)
        let flag = e.checked_sub(5).context("cube too early")?;
        Ok(Self {
            sun,
            use_character_sun: u8_at(b, 0x1C)? != 0,
            character_sun,
            use_character_ambient: u8_at(b, flag)? != 0,
            environment_ambient: AmbientCube::parse(b, e)?,
            character_ambient: AmbientCube::parse(b, c)?,
        })
    }
}

/// A level's ambiance and the sky it links.
#[derive(Debug, Clone, PartialEq)]
pub struct AmbiancePair {
    pub ambiance: Ambiance,
    pub sky: Sky,
}

/// A level's ambiance and sky, from the objects of its data files: the first `WorldAmbiance` and the sky it links.
pub fn find_in<'a>(objects: impl IntoIterator<Item = &'a crate::Object<'a>> + Clone) -> Result<AmbiancePair> {
    let amb = objects.clone().into_iter().find(|o| o.class == CLASS_WORLD_AMBIANCE).context("no WorldAmbiance")?;
    let a = Ambiance::parse(amb.body).with_context(|| amb.name.clone())?;
    let sky = objects.into_iter().find(|o| o.class == CLASS_LAYERED_SKY && o.id == a.sky).context("its LayeredSky is not there")?;
    let s = Sky::parse(sky.body).with_context(|| sky.name.clone())?;
    Ok(AmbiancePair { ambiance: a, sky: s })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cube_identity_keeps_faces() {
        let c = AmbientCube { faces: [[0.2, 0.4, 0.6]; 6], tint: [1.0; 3], saturation: 1.0, scale: 1.0, bias: 0.0, top_bend: 1.0 };
        for f in c.shader_faces() {
            for k in 0..3 {
                assert!((f[k] - [0.2, 0.4, 0.6][k]).abs() < 1e-6);
            }
        }
    }

    #[test]
    fn short_or_wrong_bodies_are_errors() {
        assert!(Ambiance::parse(&[0; 8]).is_err());
        assert!(Sky::parse(&[0; 0x40]).is_err());
    }

    #[test]
    fn sun_up_at_ninety_degrees() {
        let s = Sun { azimuth: 0.0, elevation: 90.0, color: [1.0; 3], intensity: 1.0 };
        assert!((s.toward()[2] - 1.0).abs() < 1e-6);
    }
}
