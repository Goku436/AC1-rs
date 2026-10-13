//! How AC1 lights its world, for our shaders: the level's `WorldAmbiance` and `LayeredSky` (`forge::ambiance`) as the
//! uniform the character and city shaders share (`look.wgsl`), and the lights, fog and sky colour Bevy keeps for what is
//! drawn with its own materials.
//!
//! AC1's DX9 shaders (inside the data's `MaterialTemplate`s) light a surface by the level's ambient cube (the
//! environment's for the world, the characters' own for them) times the baked occlusion (`forge::instance`), plus the
//! sun (N.L, shadows), then blend to the fog colour; the post effects (levels, colour balance, colour control) are a
//! colour table applied to the frame. Here each shader does the same, and the grade runs at the end of each shader
//! rather than as a pass over the frame (no extra pass: the grade is per pixel, the same either way for what our shaders
//! draw; the sky colour is graded on the CPU).
//!
//! Switches: `AC1_LINEAR_LOOK=1` lights in linear space (AC1's shaders work on the colours as stored, the
//! default here); `AC1_NO_GRADE=1` leaves the post effects out.

use bevy::prelude::*;
use bevy::render::render_resource::ShaderType;
use forge::ambiance::{AmbiancePair, AmbientCube};

/// The shaders' `WorldLook` (look.wgsl).
#[derive(Clone, Copy, Debug, ShaderType, Reflect)]
pub struct WorldLook {
    pub env_cube: [Vec4; 6],
    pub char_cube: [Vec4; 6],
    pub sun: Vec4,
    pub char_sun: Vec4,
    pub char_sun_dir: Vec4,
    pub fog_color: Vec4,
    pub fog: Vec4,
    pub bend: Vec4,
    pub ao: Vec4,
    pub balance_r: Vec4,
    pub balance_g: Vec4,
    pub balance_b: Vec4,
    pub levels: Vec4,
    pub control: Vec4,
    pub options: Vec4,
}

/// The level's look (a resource), and the sun's direction (toward it, Bevy space).
#[derive(Resource, Clone, Copy, Debug)]
pub struct Look {
    pub world: WorldLook,
    pub sun_dir: Vec3,
    /// The fog's distances, where it starts and where it is full (for Bevy's own materials' fog).
    pub fog_near: f32,
    pub fog_far: f32,
}

/// Game (Z up) to Bevy (Y up).
fn to_bevy(v: [f32; 3]) -> Vec3 {
    Vec3::new(v[0], v[2], -v[1])
}

fn cube(c: &AmbientCube) -> [Vec4; 6] {
    c.shader_faces().map(|f| Vec4::new(f[0], f[1], f[2], 0.0))
}

/// The baked occlusion's mapping onto the ambient (`saturate(max(v * scale + bias, floor))`, `g_AOParams`) when the
/// level has no `AOSettings`: the bytes as they are. A level's own comes from its `World` (`forge::ambiance::AoSettings`;
/// Damascus: 1.5 v + 0.1, at least 0.55). The w: the factor for meshes with none (the stand-ins), the level's mapping of
/// an open wall's byte (about 110).
const AO: Vec4 = Vec4::new(1.0, 0.0, 0.0, 0.43);

/// An open wall's occlusion byte (0..1): what meshes with none (the stand-ins) are lit as.
const AO_OPEN: f32 = 110.0 / 255.0;

/// The occlusion's mapping: the level's, or `AC1_AO="scale,bias,floor,none"` (to try others against shots of the running
/// game).
fn ao_params(level: Option<&forge::ambiance::AoSettings>) -> Vec4 {
    let v: Vec<f32> = std::env::var("AC1_AO").map(|s| s.split(',').filter_map(|x| x.trim().parse().ok()).collect()).unwrap_or_default();
    if v.len() == 4 {
        return Vec4::new(v[0], v[1], v[2], v[3]);
    }
    match level {
        Some(a) => {
            let [x, y, z] = a.shader_params();
            Vec4::new(x, y, z, a.factor(AO_OPEN))
        }
        None => AO,
    }
}

/// The fog's ramp as the shaders take it (`g_FogParams`: start, 1 / length, most): `AC1_FOGP="start,scale,max"` to
/// try others.
fn fog_ramp(fog: &forge::ambiance::Fog) -> Vec3 {
    let v: Vec<f32> = std::env::var("AC1_FOGP").map(|s| s.split(',').filter_map(|x| x.trim().parse().ok()).collect()).unwrap_or_default();
    if v.len() == 3 {
        return Vec3::new(v[0], v[1], v[2]);
    }
    // From `FogNear` to `FogFar`, at most `FogIntensity`. (Where the executable fills `g_FogParams` is not found; this
    // reading matches the running game's haze on the roofs, reports/look3: the green dome 35 m off at the game's
    // brightness. `FogDensity` is left out: slowing the ramp by it, as before, left the near city crisp.)
    Vec3::new(fog.near, 1.0 / (fog.far - fog.near).max(1e-3), fog.intensity)
}

impl Look {
    pub fn from_data(pair: &AmbiancePair) -> Self {
        let (a, s) = (&pair.ambiance, &pair.sky);
        let char_sun = if s.use_character_sun { s.character_sun } else { s.sun };
        let char_cube = if s.use_character_ambient { &s.character_ambient } else { &s.environment_ambient };
        let col = |c: [f32; 3], k: f32| Vec4::new(c[0] * k, c[1] * k, c[2] * k, 0.0);
        let fog = &a.fog;
        let cb = &a.color_balance;
        let cc = &a.colour_control;
        let world = WorldLook {
            env_cube: cube(&s.environment_ambient),
            char_cube: cube(char_cube),
            sun: col(s.sun.color, s.sun.intensity),
            char_sun: col(char_sun.color, char_sun.intensity),
            char_sun_dir: to_bevy(char_sun.toward()).extend(0.0),
            fog_color: col(fog.color, 1.0),
            fog: fog_ramp(fog).extend(if fog.enabled { 1.0 } else { 0.0 }),
            bend: Vec4::new(s.environment_ambient.top_bend, char_cube.top_bend, 0.0, 0.0),
            ao: ao_params(pair.ao.as_ref()),
            balance_r: Vec4::new(cb.shifts[0][0], cb.shifts[0][1], cb.shifts[0][2], 0.0),
            balance_g: Vec4::new(cb.shifts[1][0], cb.shifts[1][1], cb.shifts[1][2], 0.0),
            balance_b: Vec4::new(cb.shifts[2][0], cb.shifts[2][1], cb.shifts[2][2], 0.0),
            levels: Vec4::new(cb.black, cb.white, cb.gamma, cb.opacity),
            control: Vec4::new(cc.saturation, cc.brightness, cc.contrast, if cc.enabled { 1.0 } else { 0.0 }),
            options: Vec4::new(
                if std::env::var("AC1_LINEAR_LOOK").is_ok() { 0.0 } else { 1.0 },
                if std::env::var("AC1_NO_GRADE").is_ok() { 0.0 } else { 1.0 },
                if cb.enabled { 1.0 } else { 0.0 },
                // (`AC1_SHOW_LAYERS=1`: the city shows its layered materials' blend weights, `city_material.wgsl`.)
                if std::env::var("AC1_SHOW_LAYERS").is_ok() { 1.0 } else { 0.0 },
            ),
        };
        let r = world.fog;
        Self { world, sun_dir: to_bevy(s.sun.toward()), fog_near: r.x, fog_far: r.x + 1.0 / r.y.max(1e-6) }
    }

    /// The level's look from the game data: the city's own (`AC1_LEVEL`), Masyaf's for the test level.
    pub fn load(game_dir: &std::path::Path, level: Option<&str>) -> Option<Self> {
        let name = level.unwrap_or("masyaf");
        let file = crate::city::forge_of(name);
        let f = forge::Forge::open(game_dir.join(&file)).ok()?;
        // (The level's own small data file carries it, named as the level.)
        let entries: Vec<&forge::Entry> = f.entries.iter().filter(|e| e.name.eq_ignore_ascii_case(name)).collect();
        let found = entries.into_iter().chain(f.entries.iter().filter(|e| !e.name.starts_with("Cell"))).find_map(|e| {
            let data = f.read(e).ok()?;
            let objects = forge::parse_objects(&data).ok()?;
            forge::ambiance::find_in(objects.iter()).ok()
        });
        match found {
            Some(pair) => {
                info!("look: {file}'s ambiance: sun {:?}, fog {:?}", pair.sky.sun, pair.ambiance.fog);
                Some(Self::from_data(&pair))
            }
            None => {
                warn!("look: no WorldAmbiance in {file}");
                None
            }
        }
    }

    /// A display colour graded as the shaders grade theirs (the sky's clear colour).
    pub fn graded(&self, c: [f32; 3]) -> Color {
        let w = &self.world;
        let mut c = Vec3::from(c).clamp(Vec3::ZERO, Vec3::ONE);
        if w.options.y > 0.5 {
            if w.options.z > 0.5 {
                let lv = ((c - Vec3::splat(w.levels.x)).max(Vec3::ZERO) / (w.levels.y - w.levels.x).max(1e-4)).powf(1.0 / w.levels.z.max(1e-4));
                let l = lv.clamp(Vec3::ZERO, Vec3::ONE);
                let b = Vec3::new(balance(l.x, w.balance_r), balance(l.y, w.balance_g), balance(l.z, w.balance_b));
                c += (b - c) * w.levels.w;
            }
            if w.control.w > 0.5 {
                c = (c * w.control.y - Vec3::splat(0.5)) * w.control.z + Vec3::splat(0.5);
                let lum = c.dot(Vec3::new(0.3086, 0.6094, 0.082));
                c = Vec3::splat(lum) + (c - Vec3::splat(lum)) * w.control.x;
            }
        }
        let c = c.clamp(Vec3::ZERO, Vec3::ONE);
        Color::srgb(c.x, c.y, c.z)
    }
}

fn mid_weight(x: f32) -> f32 {
    let t = x * 2.0 - 1.0;
    (1.0 - t * t) * 0.667
}

/// One channel of the colour balance (look.wgsl's `balance_channel`).
fn balance(x: f32, s: Vec4) -> f32 {
    let w = if s.z > 0.0 { mid_weight(x) } else { 1.075 - 1.0 / ((1.0 - x) * 15.93 + 1.0) };
    let x = (x + w * s.z).clamp(0.0, 1.0);
    let x = (x + mid_weight(x) * s.y).clamp(0.0, 1.0);
    let w = if s.x > 0.0 { 1.075 - 1.0 / (x * 15.93 + 1.0) } else { mid_weight(x) };
    (x + w * s.x).clamp(0.0, 1.0)
}

impl Default for Look {
    /// No data: a plain warm day (Damascus' numbers rounded).
    fn default() -> Self {
        let face = |r: f32, g: f32, b: f32| Vec4::new(r, g, b, 0.0);
        let env =
            [face(0.45, 0.38, 0.37), face(0.4, 0.34, 0.31), face(0.45, 0.46, 0.47), face(0.48, 0.42, 0.38), face(0.5, 0.53, 0.53), face(0.54, 0.39, 0.35)];
        let sun_dir = Vec3::new(-0.3, 0.71, -0.64).normalize();
        Self {
            world: WorldLook {
                env_cube: env,
                char_cube: env,
                sun: Vec4::new(1.08, 0.94, 0.75, 0.0),
                char_sun: Vec4::new(1.0, 0.92, 0.86, 0.0),
                char_sun_dir: sun_dir.extend(0.0),
                fog_color: Vec4::new(0.86, 0.78, 0.66, 0.0),
                fog: Vec4::new(15.0, 1.0 / 55.0, 0.8, 1.0),
                bend: Vec4::new(1.0, 2.0, 0.0, 0.0),
                ao: AO,
                balance_r: Vec4::ZERO,
                balance_g: Vec4::ZERO,
                balance_b: Vec4::ZERO,
                levels: Vec4::new(0.0, 1.0, 1.0, 0.0),
                control: Vec4::new(1.0, 1.0, 1.0, 0.0),
                options: Vec4::new(1.0, 0.0, 0.0, 0.0),
            },
            sun_dir,
            fog_near: 15.0,
            fog_far: 70.0,
        }
    }
}

pub struct LookPlugin;

impl Plugin for LookPlugin {
    fn build(&self, app: &mut App) {
        bevy::shader::load_shader_library!(app, "look.wgsl");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn balance_without_shifts_is_the_identity() {
        for k in 0..=10 {
            let x = k as f32 / 10.0;
            assert!((balance(x, Vec4::ZERO) - x).abs() < 1e-6);
        }
    }

    #[test]
    fn grade_off_keeps_the_colour() {
        let mut l = Look::default();
        l.world.options.y = 0.0;
        let c = l.graded([0.2, 0.5, 0.8]).to_srgba();
        assert!((c.red - 0.2).abs() < 1e-5 && (c.green - 0.5).abs() < 1e-5 && (c.blue - 0.8).abs() < 1e-5);
    }
}
