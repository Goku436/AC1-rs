//! Effects: AC1's particle systems and the links that spawn them.
//!
//! - `FX` (824a23ba): an effect. Its operators spawn entities (`FXSpawnedEntity` / `FXSpawnedEntityOperator`, at a bone
//!   or the target), drive the camera (`FXCameraOperator`: the camera shakes) or a material. The spawned entity is an
//!   `Entity` (0984415e, named `GFX_FX_*`) whose `Visual` links a `ParticleSystem`. The FX graph is not walked here:
//!   `links` lists the u32s of a body at every offset, for the caller to keep those that are objects it knows.
//! - `ParticleSystem` (caaa7ee8): u32 ? (2), u32 emitter count, the `ParticleEmitter`s inline (u8 0, u32 id, u32 class
//!   a1361abc, 579 bytes each), then f32 `ParticleRateFactor`, `DurationFactor`, `VelocityFactor`, an inline `IColor`
//!   `ColorFactor` and f32 `AlphaFactor` (28 bytes). Every particle system of DataPC's "Rank 9" and "Game Bootstrap
//!   Settings" (12) parses to its end.
//! - `ParticleEmitter` (a1361abc): a fixed 579 bytes (`Emitter`); the field names are the exe's reflection list's, by
//!   CRC32 of guesses, where one was found.
//! - Where they are used: a `ContactTable` entry lists the FX a contact plays on its surface (`sound::ContactTable::fx`:
//!   the dust of steps, landings and rolls); `HumanFXPack_AlTair` (ba42b58e) the FX AC1's code plays (+88 the landing's
//!   camera shake, the haystack's straw `FX_Clip_Character_Hay_Bal(TopEntry)`, `(SideEntry)`, `(Side Exit)`).

use anyhow::{Result, bail, ensure};

/// CRC32("ParticleSystem").
pub const CLASS_PARTICLE_SYSTEM: u32 = 0xcaaa_7ee8;
/// CRC32("ParticleEmitter").
pub const CLASS_PARTICLE_EMITTER: u32 = 0xa136_1abc;
/// The class of `FX_*` objects (its name is not in the exe's strings).
pub const CLASS_FX: u32 = 0x824a_23ba;
/// CRC32("HumanFXPack").
pub const CLASS_HUMAN_FX_PACK: u32 = 0xba42_b58e;
/// CRC32("Visual"): an entity's shown mesh or particle system.
pub const CLASS_VISUAL: u32 = 0xec65_8d29;
const CLASS_ICOLOR: u32 = 0xf820_6af7;
const CLASS_WIND_FORCE: u32 = 0xe7f9_a244;
/// A `ParticleEmitter`'s serialized size after its id and class.
const EMITTER_SIZE: usize = 579;

/// `ParticleEmitter::BlendMode` (the exe's `BlendMode` enum: `Blend_Copy` 0 ... `Blend_Alpha` 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Blend {
    Copy,
    Additive,
    Subtractive,
    Modulate,
    Alpha,
    /// A value past the exe's list.
    Other(u32),
}

impl Blend {
    pub fn of(v: u32) -> Self {
        match v {
            0 => Self::Copy,
            1 => Self::Additive,
            2 => Self::Subtractive,
            3 => Self::Modulate,
            4 => Self::Alpha,
            v => Self::Other(v),
        }
    }
}

/// `ParticleEmitter::FlowMode` 0: `max_particles` sent, then done.
pub const FLOW_ONCE: u32 = 0;
/// `FLOW_LOOP`: sent for ever, at `rate`, `max_particles` alive at most.
pub const FLOW_LOOP: u32 = 1;

/// One emitter of a particle system. Tables of four keys run over a particle's life, the keys at the ends of its three
/// phases (`phase`: each phase's length, between the min and max given). Game space (Z up). Field names are the exe's
/// (`ParticleEmitter`'s reflection list, its 87 properties in stream order, names matched by CRC32 of guesses); `?`
/// where none matched.
#[derive(Clone, Debug, PartialEq)]
pub struct Emitter {
    /// +0x00 `ParticleType` (0 `PARTICLETYPE_2D`: camera-facing sprites, 1 `_3D`).
    pub particle_type: u32,
    /// +0x04 `EmitterShape` (0 point, 1 rectangle, 2 circle, 3 box, 4 sphere, 5 cylinder; its size `shape_params`).
    pub shape: u32,
    /// +0x08 `FlowMode` (`FLOW_ONCE`, `FLOW_LOOP`, 2 `FLOW_SPAWNONREQUEST`).
    pub flow: u32,
    /// +0x0c `LocalParticles`: the particles move with the emitter.
    pub local_particles: bool,
    /// u8 flags `?` at +0x0d, +0x10, +0x11, +0x1b, +0x1d, +0x1e, +0x28, +0x30 (+0x10 and +0x11 are 1 in nearly every
    /// emitter, +0x10 0 in the water splashes and falls; +0x28 1 in the blood sprays).
    pub flags: [u8; 8],
    /// +0x0e `DepthFade`, +0x0f `NearFade` (soft particles against the depth buffer, and near the camera), with
    /// +0x55 `DepthFadeScale` (200 or 1000) and +0x59 `NearFadeScale` (10 or 1).
    pub depth_fade: bool,
    pub near_fade: bool,
    pub depth_fade_scale: f32,
    pub near_fade_scale: f32,
    /// +0x12 `DirectionMode` (0 `DIRECTION_EMITTERZ`: about the emitter's Z, 1 `_FROMCENTER`, 2 `_TOCENTER`).
    pub direction_mode: u32,
    /// +0x16 `CameraRelativeSpawn`.
    pub camera_relative_spawn: bool,
    /// +0x17 `TextureAnimMode` (0 off, 1 once, 2 loop, 3 ping-pong) over the texture's `frames` at `frame_rate`.
    pub texture_anim: u32,
    /// +0x1c `DoubleSided`.
    pub double_sided: bool,
    /// +0x1f `BlendMode`.
    pub blend: Blend,
    /// +0x23 `LightingMode` (0 `LIGHTMODE_NONE`, 1 `_PERVERTEX`: lit), +0x3d `LightColor`, +0x49 `LightAmbient`
    /// (inline `IColor`s, as RGBA).
    pub lit: bool,
    pub light_color: [u8; 4],
    pub light_ambient: [u8; 4],
    /// +0x27 `GlowEnabled`, +0x1fc `GlowIntensity`.
    pub glow: bool,
    pub glow_intensity: f32,
    /// +0x29 `ParticleEffect` (0 none, 1 `PARTICLEFX_CYCLONE`), +0x21f its 8 f32 `EffectParams`.
    pub effect: u32,
    pub effect_params: [f32; 8],
    /// +0x2d `FogEnabled`.
    pub fog: bool,
    /// +0x2e `AlphaTestEnabled`, +0x2f `AlphaTestValue` (0-255).
    pub alpha_test: Option<u8>,
    /// +0x31: three f32 `?` ((0, 0, 1) in every emitter: an axis).
    pub axis: [f32; 3],
    /// +0x5d `LocalMatrix`: the emitter in its entity's frame (column-major 4x4; the fountains' jets are turned down or
    /// out by it).
    pub local_matrix: [f32; 16],
    /// +0x9d `EmitterShapeParams`: the shape's size (dust 0.1, 1, 1; straw 0.2, 0.05, -0.5).
    pub shape_params: [f32; 3],
    /// +0xa9 `ZMin`, +0xad `Delay` (s before the first particle).
    pub z_min: f32,
    pub delay: f32,
    /// +0xb1 `MaxParticleCount`.
    pub max_particles: u32,
    /// +0xb5 `ParticleRate` (particles a second).
    pub rate: f32,
    /// +0xb9 `Acceleration` (m/s², game space: gravity -9.8 or less down Z).
    pub acceleration: [f32; 3],
    /// +0xc5, +0xc9: u32 `?` (0, 5 or 20; 4 to 232, mostly 100), +0xcd u8 `?` (1 in the ambient and fire systems).
    pub unknown_c5: [u32; 2],
    pub unknown_cd: u8,
    /// +0xce `MinVelocity`, +0xd2 `MaxVelocity` (m/s).
    pub velocity: [f32; 2],
    /// +0xd6 `DirectionVariation` (radians: how far from the emitter's Z the particles leave).
    pub direction_variation: [f32; 2],
    /// +0xde `MinRotation`, +0xe2 `MaxRotation` (radians, at birth).
    pub rotation: [f32; 2],
    /// +0xe6 `MinPhaseDuration`, +0xf2 `MaxPhaseDuration` (s).
    pub phase_min: [f32; 3],
    pub phase_max: [f32; 3],
    /// +0xfe `MinSize`, +0x11e `MaxSize`: (width, height) m at each key.
    pub size_min: [[f32; 2]; 4],
    pub size_max: [[f32; 2]; 4],
    /// +0x13e `MinScale`, +0x16e `MaxScale`: a vec3 at each key (1 in the sprites).
    pub scale_min: [[f32; 3]; 4],
    pub scale_max: [[f32; 3]; 4],
    /// +0x19e `Color`: four inline `IColor`s (stored B G R A), as RGBA.
    pub color: [[u8; 4]; 4],
    /// +0x1ce `MinRotationSpeed`, +0x1de `MaxRotationSpeed` (radians a second, at each key).
    pub spin_min: [f32; 4],
    pub spin_max: [f32; 4],
    /// +0x1ee `Mesh` (0: a sprite), +0x1f2 `TextureMap` (object ids).
    pub mesh: u32,
    pub texture: u32,
    /// +0x1f6, +0x1f7: u8 `?` (the texture's frames across and down; 0 or 1: one frame).
    pub frames: [u8; 2],
    /// +0x1f8: f32 `?` (30 in most, 4-23 in the rest: the frames a second).
    pub frame_rate: f32,
    /// +0x200, +0x204, +0x208, +0x20c: f32 `?`.
    pub unknown_200: [f32; 4],
    /// +0x210 an inline `WindForce` (u32 id, class, u8 `?` +0x218, f32 `?` +0x219, two bytes `?`); after its effect
    /// params, 4 bytes (0: the `SpawnData` slot, empty).
    pub wind: (u8, f32),
}

impl Emitter {
    /// `local_matrix`'s Z axis: the way the particles leave (game space, in the entity's frame).
    pub fn up(&self) -> [f32; 3] {
        [self.local_matrix[8], self.local_matrix[9], self.local_matrix[10]]
    }
}

fn f32_at(b: &[u8], at: usize) -> Result<f32> {
    Ok(f32::from_le_bytes(u32_at(b, at)?.to_le_bytes()))
}

fn u32_at(b: &[u8], at: usize) -> Result<u32> {
    let Some(s) = b.get(at..at + 4) else { bail!("particle data ends at {at:#x}") };
    Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn floats<const N: usize>(b: &[u8], at: usize) -> Result<[f32; N]> {
    let mut out = [0.0; N];
    for (i, v) in out.iter_mut().enumerate() {
        *v = f32_at(b, at + 4 * i)?;
    }
    Ok(out)
}

/// An inline `IColor` at `o` (u32 id, class, B G R A) as RGBA.
fn icolor(b: &[u8], o: usize) -> Result<[u8; 4]> {
    ensure!(u32_at(b, o + 4)? == CLASS_ICOLOR, "no IColor at {:#x}", o + 4);
    let Some(v) = b.get(o + 8..o + 12) else { bail!("IColor at {o:#x} cut short") };
    Ok([v[2], v[1], v[0], v[3]])
}

/// A `ParticleEmitter`'s 579 bytes (after its id and class), at `at` in `b` (offsets in errors are `b`'s).
fn parse_emitter(b: &[u8], at: usize) -> Result<Emitter> {
    ensure!(b.len() >= at + EMITTER_SIZE, "emitter at {at:#x} cut short");
    let mut color = [[0u8; 4]; 4];
    for (k, c) in color.iter_mut().enumerate() {
        *c = icolor(b, at + 0x19e + 12 * k)?;
    }
    ensure!(u32_at(b, at + 0x214)? == CLASS_WIND_FORCE, "no WindForce at {:#x}", at + 0x214);
    let pairs = |o: usize| -> Result<[[f32; 2]; 4]> {
        let f: [f32; 8] = floats(b, o)?;
        Ok([[f[0], f[1]], [f[2], f[3]], [f[4], f[5]], [f[6], f[7]]])
    };
    let vec3s = |o: usize| -> Result<[[f32; 3]; 4]> {
        let f: [f32; 12] = floats(b, o)?;
        Ok(std::array::from_fn(|k| [f[3 * k], f[3 * k + 1], f[3 * k + 2]]))
    };
    let byte = |o: usize| b[at + o];
    let [r0, r1]: [f32; 2] = floats(b, at + 0xde)?;
    Ok(Emitter {
        particle_type: u32_at(b, at)?,
        shape: u32_at(b, at + 0x04)?,
        flow: u32_at(b, at + 0x08)?,
        local_particles: byte(0x0c) != 0,
        flags: [0x0d, 0x10, 0x11, 0x1b, 0x1d, 0x1e, 0x28, 0x30].map(byte),
        depth_fade: byte(0x0e) != 0,
        near_fade: byte(0x0f) != 0,
        depth_fade_scale: f32_at(b, at + 0x55)?,
        near_fade_scale: f32_at(b, at + 0x59)?,
        direction_mode: u32_at(b, at + 0x12)?,
        camera_relative_spawn: byte(0x16) != 0,
        texture_anim: u32_at(b, at + 0x17)?,
        double_sided: byte(0x1c) != 0,
        blend: Blend::of(u32_at(b, at + 0x1f)?),
        lit: u32_at(b, at + 0x23)? != 0,
        light_color: icolor(b, at + 0x3d)?,
        light_ambient: icolor(b, at + 0x49)?,
        glow: byte(0x27) != 0,
        glow_intensity: f32_at(b, at + 0x1fc)?,
        effect: u32_at(b, at + 0x29)?,
        effect_params: floats(b, at + 0x21f)?,
        fog: byte(0x2d) != 0,
        alpha_test: (byte(0x2e) != 0).then_some(byte(0x2f)),
        axis: floats(b, at + 0x31)?,
        local_matrix: floats(b, at + 0x5d)?,
        shape_params: floats(b, at + 0x9d)?,
        z_min: f32_at(b, at + 0xa9)?,
        delay: f32_at(b, at + 0xad)?,
        max_particles: u32_at(b, at + 0xb1)?,
        rate: f32_at(b, at + 0xb5)?,
        acceleration: floats(b, at + 0xb9)?,
        unknown_c5: [u32_at(b, at + 0xc5)?, u32_at(b, at + 0xc9)?],
        unknown_cd: byte(0xcd),
        velocity: floats(b, at + 0xce)?,
        direction_variation: floats(b, at + 0xd6)?,
        rotation: [r0, r1],
        phase_min: floats(b, at + 0xe6)?,
        phase_max: floats(b, at + 0xf2)?,
        size_min: pairs(at + 0xfe)?,
        size_max: pairs(at + 0x11e)?,
        scale_min: vec3s(at + 0x13e)?,
        scale_max: vec3s(at + 0x16e)?,
        color,
        spin_min: floats(b, at + 0x1ce)?,
        spin_max: floats(b, at + 0x1de)?,
        mesh: u32_at(b, at + 0x1ee)?,
        texture: u32_at(b, at + 0x1f2)?,
        frames: [byte(0x1f6), byte(0x1f7)],
        frame_rate: f32_at(b, at + 0x1f8)?,
        unknown_200: floats(b, at + 0x200)?,
        wind: (byte(0x218), f32_at(b, at + 0x219)?),
    })
}

/// A `ParticleSystem` body's emitters (see the module header).
pub fn parse_particle_system(body: &[u8]) -> Result<Vec<Emitter>> {
    let n = u32_at(body, 4)? as usize;
    ensure!(n < 64, "{n} emitters at 0x4");
    let mut at = 8;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        ensure!(body.get(at) == Some(&0), "emitter at {at:#x} not inline");
        ensure!(u32_at(body, at + 5)? == CLASS_PARTICLE_EMITTER, "no ParticleEmitter at {:#x}", at + 5);
        out.push(parse_emitter(body, at + 9)?);
        at += 9 + EMITTER_SIZE;
    }
    ensure!(body.len() == at + 28, "{} bytes after the emitters at {at:#x}, not 28", body.len().saturating_sub(at));
    Ok(out)
}

/// Every u32 of `body` at every byte offset (the links an FX graph or an entity may hold, for the caller to match
/// against the objects it knows), in order, zero and 0xffffffff left out.
pub fn links(body: &[u8]) -> Vec<u32> {
    body.windows(4).map(|w| u32::from_le_bytes([w[0], w[1], w[2], w[3]])).filter(|&v| v != 0 && v != u32::MAX).collect()
}

/// The particle systems an entity's `Visual`s (ec658d29: its class, u8 shown, u32 the mesh or particle system) show,
/// in order, each once: `is_system` says which links are particle systems. (A placed effect is an entity whose
/// `Visual` shows a `ParticleSystem` instead of a mesh: a fountain's jets, a lantern's fire, a chimney's smoke.)
pub fn shown_systems(body: &[u8], is_system: impl Fn(u32) -> bool) -> Vec<u32> {
    let mut out = vec![];
    let tag = CLASS_VISUAL.to_le_bytes();
    for (k, _) in body.windows(4).enumerate().filter(|(_, w)| *w == tag) {
        let Some(&shown) = body.get(k + 4) else { continue };
        let Ok(id) = u32_at(body, k + 5) else { continue };
        if shown != 0 && is_system(id) && !out.contains(&id) {
            out.push(id);
        }
    }
    out
}

/// `HumanFXPack` slots (byte offsets into its body, a u32 link each): the FX AC1's code plays.
pub mod pack {
    /// `FX_Camera_Shake_Assassins_Fall` (`HumanInAir__SetupToGround_Landing` 0xE05940, over 3 m).
    pub const FALL_SHAKE: usize = 88;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn class_ids_are_crc32_of_their_names() {
        assert_eq!(crc32fast::hash(b"ParticleSystem"), CLASS_PARTICLE_SYSTEM);
        assert_eq!(crc32fast::hash(b"ParticleEmitter"), CLASS_PARTICLE_EMITTER);
        assert_eq!(crc32fast::hash(b"HumanFXPack"), CLASS_HUMAN_FX_PACK);
        assert_eq!(crc32fast::hash(b"IColor"), CLASS_ICOLOR);
        assert_eq!(crc32fast::hash(b"WindForce"), CLASS_WIND_FORCE);
        assert_eq!(crc32fast::hash(b"Visual"), CLASS_VISUAL);
    }

    /// A made-up system of one emitter: its fields where the layout puts them.
    #[test]
    fn a_particle_system_reads_its_emitter_and_ends_exactly() {
        let mut e = vec![0u8; EMITTER_SIZE];
        let put = |e: &mut Vec<u8>, at: usize, v: u32| e[at..at + 4].copy_from_slice(&v.to_le_bytes());
        put(&mut e, 0x04, 2);
        put(&mut e, 0x08, FLOW_LOOP);
        put(&mut e, 0x1f, 1);
        put(&mut e, 0x23, 1);
        put(&mut e, 0xb1, 30);
        put(&mut e, 0xb5, 300f32.to_bits());
        put(&mut e, 0xc1, (-2f32).to_bits());
        put(&mut e, 0xe6, 0.5f32.to_bits());
        // (The local matrix: identity, 1.6 m down.)
        for k in 0..4 {
            put(&mut e, 0x5d + 20 * k, 1f32.to_bits());
        }
        put(&mut e, 0x5d + 56, (-1.6f32).to_bits());
        for o in [0x3d, 0x49] {
            put(&mut e, o + 4, CLASS_ICOLOR);
        }
        e[0x49 + 8..0x49 + 12].copy_from_slice(&[1, 2, 3, 255]);
        for k in 0..4 {
            put(&mut e, 0x19e + 12 * k + 4, CLASS_ICOLOR);
            e[0x19e + 12 * k + 8..0x19e + 12 * k + 12].copy_from_slice(&[10, 20, 30, 40]);
        }
        put(&mut e, 0x1f2, 0x0ff5_9d15);
        put(&mut e, 0x1f8, 30f32.to_bits());
        put(&mut e, 0x214, CLASS_WIND_FORCE);
        let mut body = vec![2, 0, 0, 0, 1, 0, 0, 0, 0];
        body.extend(0x150d_9fa1u32.to_le_bytes());
        body.extend(CLASS_PARTICLE_EMITTER.to_le_bytes());
        body.extend(&e);
        body.extend([0u8; 28]);
        let em = parse_particle_system(&body).unwrap();
        assert_eq!(em.len(), 1);
        let em = &em[0];
        assert_eq!((em.shape, em.flow, em.max_particles, em.rate), (2, FLOW_LOOP, 30, 300.0));
        assert_eq!((em.blend, em.lit), (Blend::Additive, true));
        assert_eq!(em.acceleration, [0.0, 0.0, -2.0]);
        assert_eq!(em.phase_min[0], 0.5);
        assert_eq!(em.color[0], [30, 20, 10, 40], "stored B G R A");
        assert_eq!(em.light_ambient, [3, 2, 1, 255]);
        assert_eq!(em.up(), [0.0, 0.0, 1.0]);
        assert_eq!(em.local_matrix[14], -1.6);
        assert_eq!((em.texture, em.frame_rate), (0x0ff5_9d15, 30.0));
        body.push(0);
        assert!(parse_particle_system(&body).is_err(), "a byte too many");
        assert!(parse_particle_system(&body[..100]).is_err());
    }

    #[test]
    fn links_skip_zero_and_none() {
        assert_eq!(links(&[1, 0, 0, 0, 0xff, 0xff, 0xff, 0xff]), vec![1, 0xff00_0000, 0xffff_0000, 0xffff_ff00]);
    }

    /// An entity's `Visual`: shown, its link a system; hidden ones and meshes left out.
    #[test]
    fn a_visual_shows_its_particle_system() {
        let mut b = vec![0u8; 8];
        for (shown, id) in [(1u8, 7u32), (0, 8), (1, 9), (1, 7)] {
            b.extend(5u32.to_le_bytes());
            b.extend(CLASS_VISUAL.to_le_bytes());
            b.push(shown);
            b.extend(id.to_le_bytes());
        }
        assert_eq!(shown_systems(&b, |id| id != 9), vec![7]);
        assert!(shown_systems(&b[..b.len() - 2], |_| false).is_empty(), "cut short");
    }

    /// The install's particle systems parse to their end: DataPC's and Damascus' (the pass rate over every forge is the
    /// `fxcheck` example's); and the Damascus cells place their effects (skips without the install).
    #[test]
    fn install_particle_systems_parse() {
        let dir = std::env::var("AC1_GAME_DIR").unwrap_or_else(|_| r"P:\SteamLibrary\steamapps\common\Assassins Creed".into());
        let dir = std::path::Path::new(&dir);
        if !dir.join("DataPC.forge").exists() {
            eprintln!("no install, skipped");
            return;
        }
        let (mut n, mut ok, mut placed) = (0, 0, 0);
        for file in ["DataPC.forge", "DataPC_Damascus.forge"] {
            let forge = crate::Forge::open(dir.join(file)).unwrap();
            let datas: Vec<Vec<u8>> = forge.entries.iter().filter_map(|e| forge.read(e).ok()).collect();
            let objects: Vec<Vec<crate::Object>> = datas.iter().map(|d| crate::parse_objects(d).unwrap_or_default()).collect();
            let systems: std::collections::HashSet<u32> = objects.iter().flatten().filter(|o| o.class == CLASS_PARTICLE_SYSTEM).map(|o| o.id).collect();
            for o in objects.iter().flatten() {
                if o.class == CLASS_PARTICLE_SYSTEM {
                    n += 1;
                    match parse_particle_system(o.body) {
                        Ok(_) => ok += 1,
                        Err(e) => eprintln!("{file}: {}: {e:#}", o.name),
                    }
                } else if file.contains("Damascus") && !shown_systems(o.body, |id| systems.contains(&id)).is_empty() {
                    placed += 1;
                }
            }
        }
        eprintln!("particle systems {ok}/{n}; Damascus objects showing one: {placed}");
        assert_eq!(ok, n);
        assert!(n >= 405, "{n}");
        assert!(placed >= 300, "{placed}");
    }
}
