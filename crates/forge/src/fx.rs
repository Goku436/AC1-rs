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
const CLASS_ICOLOR: u32 = 0xf820_6af7;
const CLASS_WIND_FORCE: u32 = 0xe7f9_a244;
/// A `ParticleEmitter`'s serialized size after its id and class.
const EMITTER_SIZE: usize = 579;

/// One emitter of a particle system. Tables of four keys run over a particle's life, the keys at the ends of its three
/// phases (`phase`: each phase's length, between the min and max given). Game space (Z up).
#[derive(Clone, Debug, PartialEq)]
pub struct Emitter {
    /// +0x04 `EmitterShape` (0 a point; 1-4 an area, `shape_params[0]` across: our reading).
    pub shape: u32,
    /// +0x9d: three f32 ? (the shape's size: dust 0.1, 1, 1; straw 0.2, 0.05, -0.5).
    pub shape_params: [f32; 3],
    /// +0xb1 `MaxParticleCount`.
    pub max_particles: u32,
    /// +0xb5 `ParticleRate` (particles a second).
    pub rate: f32,
    /// +0xb9 `Acceleration` (m/s², game space: gravity -9.8 or less down Z).
    pub acceleration: [f32; 3],
    /// +0xce `MinVelocity`, +0xd2 `MaxVelocity` (m/s).
    pub velocity: [f32; 2],
    /// +0xd6 `DirectionVariation` (radians: how far from straight up the particles leave).
    pub direction_variation: [f32; 2],
    /// +0xde `MinRotation`, +0xe2 `MaxRotation` (radians, at birth).
    pub rotation: [f32; 2],
    /// +0xe6 `MinPhaseDuration`, +0xf2 `MaxPhaseDuration` (s).
    pub phase_min: [f32; 3],
    pub phase_max: [f32; 3],
    /// +0xfe `MinSize`, +0x11e `MaxSize`: (width, height) m at each key.
    pub size_min: [[f32; 2]; 4],
    pub size_max: [[f32; 2]; 4],
    /// +0x19e `Color`: four inline `IColor`s (stored B G R A), as RGBA.
    pub color: [[u8; 4]; 4],
    /// +0x1ce `MinRotationSpeed`, +0x1de `MaxRotationSpeed` (radians a second, at each key).
    pub spin_min: [f32; 4],
    pub spin_max: [f32; 4],
    /// +0x1ee `Mesh` (0: a sprite), +0x1f2 `TextureMap` (object ids).
    pub mesh: u32,
    pub texture: u32,
    /// +0x1f6 `TextureFrameCountX`, +0x1f7 `TextureFrameCountY` (0 or 1: one frame).
    pub frames: [u8; 2],
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

/// A `ParticleEmitter`'s 579 bytes (after its id and class), at `at` in `b` (offsets in errors are `b`'s).
fn parse_emitter(b: &[u8], at: usize) -> Result<Emitter> {
    ensure!(b.len() >= at + EMITTER_SIZE, "emitter at {at:#x} cut short");
    let mut color = [[0u8; 4]; 4];
    for (k, c) in color.iter_mut().enumerate() {
        let o = at + 0x19e + 12 * k;
        ensure!(u32_at(b, o + 4)? == CLASS_ICOLOR, "no IColor at {:#x}", o + 4);
        let v = &b[o + 8..o + 12];
        *c = [v[2], v[1], v[0], v[3]];
    }
    ensure!(u32_at(b, at + 0x214)? == CLASS_WIND_FORCE, "no WindForce at {:#x}", at + 0x214);
    let pairs = |o: usize| -> Result<[[f32; 2]; 4]> {
        let f: [f32; 8] = floats(b, o)?;
        Ok([[f[0], f[1]], [f[2], f[3]], [f[4], f[5]], [f[6], f[7]]])
    };
    let [r0, r1]: [f32; 2] = floats(b, at + 0xde)?;
    Ok(Emitter {
        shape: u32_at(b, at + 0x04)?,
        shape_params: floats(b, at + 0x9d)?,
        max_particles: u32_at(b, at + 0xb1)?,
        rate: f32_at(b, at + 0xb5)?,
        acceleration: floats(b, at + 0xb9)?,
        velocity: floats(b, at + 0xce)?,
        direction_variation: floats(b, at + 0xd6)?,
        rotation: [r0, r1],
        phase_min: floats(b, at + 0xe6)?,
        phase_max: floats(b, at + 0xf2)?,
        size_min: pairs(at + 0xfe)?,
        size_max: pairs(at + 0x11e)?,
        color,
        spin_min: floats(b, at + 0x1ce)?,
        spin_max: floats(b, at + 0x1de)?,
        mesh: u32_at(b, at + 0x1ee)?,
        texture: u32_at(b, at + 0x1f2)?,
        frames: [b[at + 0x1f6], b[at + 0x1f7]],
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
    }

    /// A made-up system of one emitter: its fields where the layout puts them.
    #[test]
    fn a_particle_system_reads_its_emitter_and_ends_exactly() {
        let mut e = vec![0u8; EMITTER_SIZE];
        let put = |e: &mut Vec<u8>, at: usize, v: u32| e[at..at + 4].copy_from_slice(&v.to_le_bytes());
        put(&mut e, 0x04, 2);
        put(&mut e, 0xb1, 30);
        put(&mut e, 0xb5, 300f32.to_bits());
        put(&mut e, 0xc1, (-2f32).to_bits());
        put(&mut e, 0xe6, 0.5f32.to_bits());
        for k in 0..4 {
            put(&mut e, 0x19e + 12 * k + 4, CLASS_ICOLOR);
            e[0x19e + 12 * k + 8..0x19e + 12 * k + 12].copy_from_slice(&[10, 20, 30, 40]);
        }
        put(&mut e, 0x1f2, 0x0ff5_9d15);
        put(&mut e, 0x214, CLASS_WIND_FORCE);
        let mut body = vec![2, 0, 0, 0, 1, 0, 0, 0, 0];
        body.extend(0x150d_9fa1u32.to_le_bytes());
        body.extend(CLASS_PARTICLE_EMITTER.to_le_bytes());
        body.extend(&e);
        body.extend([0u8; 28]);
        let em = parse_particle_system(&body).unwrap();
        assert_eq!(em.len(), 1);
        let em = &em[0];
        assert_eq!((em.shape, em.max_particles, em.rate), (2, 30, 300.0));
        assert_eq!(em.acceleration, [0.0, 0.0, -2.0]);
        assert_eq!(em.phase_min[0], 0.5);
        assert_eq!(em.color[0], [30, 20, 10, 40], "stored B G R A");
        assert_eq!(em.texture, 0x0ff5_9d15);
        body.push(0);
        assert!(parse_particle_system(&body).is_err(), "a byte too many");
        assert!(parse_particle_system(&body[..100]).is_err());
    }

    #[test]
    fn links_skip_zero_and_none() {
        assert_eq!(links(&[1, 0, 0, 0, 0xff, 0xff, 0xff, 0xff]), vec![1, 0xff00_0000, 0xffff_0000, 0xffff_ff00]);
    }

    /// The install's particle systems parse to their end (skips without the install).
    #[test]
    fn install_particle_systems_parse() {
        let dir = std::env::var("AC1_GAME_DIR").unwrap_or_else(|_| r"P:\SteamLibrary\steamapps\common\Assassins Creed".into());
        let Ok(forge) = crate::Forge::open(std::path::Path::new(&dir).join("DataPC.forge")) else {
            eprintln!("no install, skipped");
            return;
        };
        let mut n = 0;
        for name in ["Rank 9", "Game Bootstrap Settings"] {
            let e = forge.entries.iter().find(|e| e.name == name).unwrap();
            let data = forge.read(e).unwrap();
            for o in crate::parse_objects(&data).unwrap().iter().filter(|o| o.class == CLASS_PARTICLE_SYSTEM) {
                parse_particle_system(o.body).unwrap_or_else(|e| panic!("{}: {e:#}", o.name));
                n += 1;
            }
        }
        assert!(n >= 12, "{n}");
    }
}
