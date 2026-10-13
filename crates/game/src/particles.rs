//! AC1's particle effects (`forge::fx`), drawn as camera-facing sprites with AC1's blend and lighting modes
//! (`particle_material`).
//!
//! What spawns them, as AC1's data has it:
//! - Every contact key of Altaïr's clips (`ContactEvent`: steps, landings, rolls, slides) plays the `FX` its
//!   `ContactTable_AlTair` entry lists for the surface under him (`ContactTable::fx`): `FX_CollisionMaterial_<surface>`,
//!   which spawns `GFX_FX_CollisionMaterial_Default` (a puff of `GFX_Dust_Sand_01`) on dirt, sand, stone, wood and
//!   ceramic, with `_Default_Smoke` on dirt, carpet and unknown surfaces, `_Straw` on straw, `_Grass` on grass and
//!   `_Water_Puddle` in puddles; marble, pebbles, leaves, snow and glass spawn nothing. Steps at the lower foot, the
//!   rest (landings, rolls, hands) at the root.
//! - The haystack: `HumanFXPack_AlTair`'s `FX_Clip_Character_Hay_Bal(TopEntry)` going in (at the straw's sound key of
//!   the dive) and `(Side Exit)` hopping out, each a straw system at six bones (`RightArm`, `LeftArm`, `Spine1`, `Head`,
//!   `LeftForeArm`, `RightForeArm`).
//! - The city's placed effects (`WorldFx`, found by `city`): the entities of the cells whose `Visual` shows a
//!   `ParticleSystem` rather than a mesh: the fountains' and wall fountains' jets, rings and foam
//!   (`GFX_Object_Water_*`), the lanterns', cages' and torches' fire and smoke (`GFX_Object_Fire_*`), chimneys', pots'
//!   and incense smoke (`GFX_Object_Smoke_*`), the waterfalls, the flags' glints. Their emitters loop
//!   (`FLOW_LOOP`) while the camera is within `WORLD_RANGE` of them (as AC1 has them only in the cells loaded round the
//!   camera); one coming into range starts as if it had been running (its particles of every age at once).
//!
//! An emitter (`forge::fx::Emitter`) sends its particles from its shape (in its `LocalMatrix`, in the entity's frame),
//! along the matrix's Z tipped by `DirectionVariation` (or out from its centre), at `MinVelocity`-`MaxVelocity`, with
//! its `Acceleration` (world space) and the city's wind times its `WindForce` factor; a burst (`FLOW_ONCE`) sends
//! `MaxParticleCount` at `ParticleRate`, a loop keeps at most that many alive at that rate. Each particle lives its three
//! phases, its size, colour and spin going key to key at their ends; the texture's frames play at its frame rate. A city
//! emitter runs between its two camera distances (`view_range`), fading in and out over `VIEW_FADE` s as AC1's
//! (0xA80B40). As AC1's DX9 particle shaders: unlit (no emitter sets the light flag), unfogged (they have no fog).
//! Ours: the FX graph's operators (their property overrides, `FXLinkedValue`s) are not run, the scale keys,
//! `ParticleEffect`s (the cyclone), depth and near fades and 3D (mesh) particles are not. Not in headless runs (no
//! GPU).

use crate::animation::Playing;
use crate::character::{Character, Player};
use crate::level::Level;
use crate::particle_material::ParticleSurface;
use crate::sound::{Cursor, from_time, passed};
use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use forge::anim::{CONTACT_HUMAN, EventKind};
use forge::fx::{Blend, CLASS_FX, CLASS_PARTICLE_SYSTEM, Emitter, FLOW_LOOP};
use forge::sound::ContactTable;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

/// The data files of `DataPC.forge` that hold Altaïr's effects.
const FX_FILES: [&str; 2] = ["Rank 9", "Game Bootstrap Settings"];
/// The bones the haystack's straw comes off (`FX_Clip_Character_Hay_Bal(*)`).
const HAY_BONES: [&str; 6] = ["RightArm", "LeftArm", "Spine1", "Head", "LeftForeArm", "RightForeArm"];
/// Clips going into the hay (its `TopEntry` straw at their first sound key) and out of it (`Side Exit`, at the start).
const HAY_IN: [&str; 3] = ["xx_h_faith_jump_landing", "xx_h_air_to_haystack", "_to_haystack_0"];
const HAY_OUT: &str = "xx_l_haystack_hop_out";
/// At most this many particles at once (new ones wait while it is full).
const MAX_PARTICLES: usize = 12000;
/// The city's effects run within this distance of the camera at most (m): about the cells AC1 keeps loaded round it.
const WORLD_RANGE: f32 = 120.0;
/// How long a city emitter takes to fade in or out as the camera crosses its `view_range` (s; AC1's 5.0 at 0x1698BA8).
const VIEW_FADE: f32 = 5.0;
/// And only those ahead of the camera (the cosine of the angle off its view, wide enough for a wide screen and the
/// particles' size), or within `CLOSE` of it.
const AHEAD: f32 = 0.35;
const CLOSE: f32 = 12.0;

/// The camera distances a city emitter runs between (m): its `view_range`, at most `WORLD_RANGE`.
fn range_of(em: &Emitter) -> (f32, f32) {
    let [near, far] = em.view_range.map(|d| (d as f32).min(WORLD_RANGE));
    (near, far)
}
/// The forges a city's effects' textures are looked up in, after the city's own.
const FX_TEXTURE_FORGES: [&str; 2] = ["DataPC.forge", "DataPC_Common.forge"];

/// Game (Z up) to Bevy (Y up), as a matrix.
fn game_to_bevy() -> Mat4 {
    Mat4::from_cols(Vec4::X, Vec4::NEG_Z, Vec4::Y, Vec4::W)
}

/// The effects a city places (`city` finds them as it places the entities): each a `ParticleSystem` (its id, name and
/// body) shown by an entity at a world transform (Bevy space; the entity's frame, Z up inside).
#[derive(Resource, Default, Clone)]
pub struct WorldFx {
    pub forge_file: String,
    /// (By id, in order: the render cache's bytes are the same every build.)
    pub systems: std::collections::BTreeMap<u32, (String, Vec<u8>)>,
    pub placed: Vec<(Mat4, u32)>,
}

/// An emitter as it is drawn: its fields, its batch and its glow's (`HALO`).
struct Def {
    em: Emitter,
    batch: usize,
    halo: Option<usize>,
}

/// AC1's glow pass (`GlowEnabled`: the sprite's alpha x `GlowIntensity` drawn into a mask, the frame x the mask
/// downscaled 4 times and blurred over five halving levels, added back, 0xADC7D0) is drawn as the sprite's own colour x
/// alpha x `GlowIntensity` (at most 1, the mask's 8 bits) added in it (the frame under the mask, unblurred), and a
/// halo: each glowing particle once more, this many times its size, adding that / `HALO`² (the same light spread over
/// the larger quad, as a blur spreads it).
const HALO: f32 = 2.5;

/// An effect: its emitters (`Particles::defs`) and the bones it plays at (none: the contact point).
struct FxDef {
    emitters: Vec<usize>,
    bones: Vec<&'static str>,
}

/// Textures loaded for the effects (by object id), before they are registered.
type Loaded = Vec<(u32, Image)>;

/// What Altaïr's effects are made of: the contact table's FX by object id, the hay ones (their emitters by index into
/// `emitters`), and the textures.
struct FxData {
    contacts: ContactTable,
    fx: HashMap<u32, Vec<usize>>,
    hay_in: Option<Vec<usize>>,
    hay_out: Option<Vec<usize>>,
    emitters: Vec<Emitter>,
    textures: Loaded,
}

/// A city's effects as loaded: per system its emitters, the placements (transform, system index), the textures.
struct WorldData {
    systems: Vec<(String, Vec<Emitter>)>,
    placed: Vec<(Mat4, usize)>,
    textures: Loaded,
}

/// The texture an emitter names, decoded: a `Texture`, or the first texture another object (a map spec) links.
fn texture_of(id: u32, objs: &HashMap<u32, forge::Object>) -> Option<Image> {
    let o = objs.get(&id)?;
    let tex = if o.class == forge::CLASS_TEXTURE {
        o
    } else {
        forge::fx::links(o.body).into_iter().find_map(|l| objs.get(&l).filter(|t| t.class == forge::CLASS_TEXTURE))?
    };
    forge::decode_texture(tex.body).ok().map(crate::assets::to_image)
}

fn load(game_dir: &Path) -> anyhow::Result<FxData> {
    let forge = forge::Forge::open(game_dir.join("DataPC.forge"))?;
    let mut datas = vec![];
    for name in FX_FILES {
        let Some(e) = forge.entries.iter().find(|e| e.name == name) else { anyhow::bail!("no data file {name}") };
        datas.push(forge.read(e)?);
    }
    let mut objs: HashMap<u32, forge::Object> = HashMap::new();
    for d in &datas {
        for o in forge::parse_objects(d)? {
            objs.entry(o.id).or_insert(o);
        }
    }
    let contacts = objs
        .values()
        .find(|o| o.class == forge::sound::CLASS_CONTACT_TABLE && o.name == "ContactTable_AlTair")
        .map(|o| forge::sound::parse_contact_table(o.body))
        .ok_or_else(|| anyhow::anyhow!("no ContactTable_AlTair"))??;
    let mut emitters: Vec<Emitter> = vec![];
    // An FX's systems: the entities it links (in order, each once), the particle systems they link.
    let mut def = |fx: &forge::Object| -> Vec<usize> {
        let mut out = vec![];
        let mut seen = vec![];
        for l in forge::fx::links(fx.body) {
            let Some(ent) = objs.get(&l).filter(|o| o.class == forge::datafile::CLASS_ENTITY && !seen.contains(&o.id)) else { continue };
            seen.push(ent.id);
            for p in forge::fx::links(ent.body) {
                let Some(ps) = objs.get(&p).filter(|o| o.class == CLASS_PARTICLE_SYSTEM) else { continue };
                match forge::fx::parse_particle_system(ps.body) {
                    Ok(em) => {
                        for e in em {
                            out.push(emitters.len());
                            emitters.push(e);
                        }
                    }
                    Err(e) => warn!("fx: particle system {}: {e:#}", ps.name),
                }
                break;
            }
        }
        out
    };
    let mut fx = HashMap::new();
    for id in contacts.fx.iter().flatten().flatten() {
        if fx.contains_key(id) {
            continue;
        }
        if let Some(o) = objs.get(id).filter(|o| o.class == CLASS_FX) {
            fx.insert(*id, def(o));
        }
    }
    let named = |n: &str| objs.values().find(|o| o.class == CLASS_FX && o.name == n);
    let hay_in = named("FX_Clip_Character_Hay_Bal(TopEntry)").map(&mut def);
    let hay_out = named("FX_Clip_Character_Hay_Bal(Side Exit)").map(&mut def);
    let mut ids: Vec<u32> = emitters.iter().map(|e| e.texture).collect();
    ids.sort_unstable();
    ids.dedup();
    let textures = ids.into_iter().filter_map(|id| Some((id, texture_of(id, &objs)?))).collect();
    Ok(FxData { contacts, fx, hay_in, hay_out, emitters, textures })
}

/// A city's effects: their emitters, and their textures from the city's forge, DataPC's and the shared one (each found
/// by the forge's object index, only those data files read).
fn load_world(game_dir: &Path, fx: &WorldFx) -> anyhow::Result<WorldData> {
    let mut at: HashMap<u32, usize> = HashMap::new();
    let mut systems = vec![];
    let mut bad = 0;
    for (id, (name, body)) in &fx.systems {
        match forge::fx::parse_particle_system(body) {
            Ok(ems) => {
                at.insert(*id, systems.len());
                systems.push((name.clone(), ems));
            }
            Err(e) => {
                bad += 1;
                warn!("fx: particle system {name}: {e:#}");
            }
        }
    }
    let placed: Vec<(Mat4, usize)> = fx.placed.iter().filter_map(|(m, id)| Some((*m, *at.get(id)?))).collect();
    let mut want: Vec<u32> = systems.iter().flat_map(|s| s.1.iter().map(|e| e.texture)).filter(|&t| t != 0).collect();
    want.sort_unstable();
    want.dedup();
    let mut textures: Loaded = vec![];
    for name in std::iter::once(fx.forge_file.as_str()).chain(FX_TEXTURE_FORGES) {
        if want.is_empty() {
            break;
        }
        let path = game_dir.join(name);
        let forge = forge::Forge::open(&path)?;
        let idx = forge::index::ForgeIndex::load(&path)?;
        let mut files: Vec<&str> = want.iter().filter_map(|id| idx.objects.get(id)).map(|p| p.datafile.as_str()).collect();
        files.sort_unstable();
        files.dedup();
        let datas: Vec<Vec<u8>> = files.iter().filter_map(|f| forge.entries.iter().find(|e| e.name == *f)).filter_map(|e| forge.read(e).ok()).collect();
        let objs: HashMap<u32, forge::Object> = datas.iter().flat_map(|d| forge::parse_objects(d).unwrap_or_default()).map(|o| (o.id, o)).collect();
        want.retain(|id| match texture_of(*id, &objs) {
            Some(image) => {
                textures.push((*id, image));
                false
            }
            None => true,
        });
    }
    info!(
        "fx: the city's effects: {} placed, {} particle systems ({bad} not read), {} textures ({} not found)",
        placed.len(),
        systems.len(),
        textures.len(),
        want.len()
    );
    Ok(WorldData { systems, placed, textures })
}

type Slot<T> = Arc<Mutex<Option<anyhow::Result<T>>>>;

/// Run `f` on a thread of its own; its result lands in the slot.
fn on_thread<T: Send + 'static>(f: impl FnOnce() -> anyhow::Result<T> + Send + 'static) -> Slot<T> {
    let slot: Slot<T> = Arc::new(Mutex::new(None));
    let out = slot.clone();
    std::thread::spawn(move || {
        let r = f();
        if let Ok(mut o) = out.lock() {
            *o = Some(r);
        }
    });
    slot
}

fn take<T>(slot: &Slot<T>) -> Option<anyhow::Result<T>> {
    slot.lock().ok().and_then(|mut o| o.take())
}

/// One emitter sending out a burst (Altaïr's effects).
struct Burst {
    def: usize,
    /// Its frame: the emitter's local space (game axes) to the world (Bevy).
    frame: Mat4,
    /// The bone it follows while it emits, if any.
    bone: Option<Entity>,
    sent: u32,
    t: f32,
}

/// A city effect's emitter, running while the camera is near and looking its way.
struct Source {
    def: usize,
    frame: Mat4,
    at: Vec3,
    /// The camera distances it runs between (m, `range_of`).
    near: f32,
    range: f32,
    /// Its fade (0-1, `VIEW_FADE`): it multiplies its particles' alpha (NaN until its first frame).
    fade: f32,
    /// The wind on its particles this frame (Bevy space, m/s²: the city's wind times its factor).
    wind: Vec3,
    active: bool,
    t: f32,
    carry: f32,
    alive: u32,
    sent: u32,
}

/// No source (a burst's particle).
const NO_SOURCE: u32 = u32::MAX;

#[derive(Clone, Copy)]
struct Particle {
    batch: usize,
    /// Its glow's halo batch (`HALO`), if any.
    halo: Option<usize>,
    src: u32,
    pos: Vec3,
    vel: Vec3,
    acc: Vec3,
    /// Its emitter's wind factor (a burst's: the wind where the camera is).
    wind: f32,
    /// It dies at or below this height (Bevy y: the emitter's `ZMin` where it kills below it).
    floor: f32,
    /// It lies along its velocity (no spin).
    along: bool,
    age: f32,
    /// The ends of its phases (s from birth): keys 1, 2, 3.
    keys: [f32; 3],
    size: [[f32; 2]; 4],
    /// Its colour keys, RGBA 0-1 as stored.
    color: [[f32; 4]; 4],
    rot: f32,
    spin: [f32; 4],
    /// Its texture's frames (across, down), animation mode, frames a second, and its frame when not animated.
    frames: [u8; 2],
    anim: u32,
    frame_rate: f32,
    frame0: u32,
}

impl Particle {
    /// Where it is between its keys: (key, fraction to the next).
    fn key(&self) -> (usize, f32) {
        let mut from = 0.0;
        for (k, &end) in self.keys.iter().enumerate() {
            if self.age < end {
                return (k, ((self.age - from) / (end - from).max(1e-4)).clamp(0.0, 1.0));
            }
            from = end;
        }
        (2, 1.0)
    }

    /// The texture frame it shows (`TextureAnimMode`: 0 its own, 1 once, 2 loop, 3 ping-pong).
    fn frame(&self) -> u32 {
        let n = self.frames[0].max(1) as u32 * self.frames[1].max(1) as u32;
        if n <= 1 {
            return 0;
        }
        let f = (self.age * self.frame_rate.max(0.0)) as u32;
        match self.anim {
            1 => f.min(n - 1),
            2 => (f + self.frame0) % n,
            3 => {
                let k = f % (2 * n - 2);
                if k < n { k } else { 2 * n - 2 - k }
            }
            _ => self.frame0 % n,
        }
    }
}

/// A batch: the particles of one texture, blend and lighting, drawn as one mesh.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct BatchKey {
    tex: usize,
    blend: u32,
    /// Lit: AC1's sprite light flag (`Emitter::sprite_light`, not `LightingMode`, which the DX9 renderer never reads).
    lit: bool,
    /// `GlowIntensity` in thousandths where `GlowEnabled`, else 0.
    glow: u32,
    /// A glow's halo (`HALO`).
    halo: bool,
}

#[derive(Resource, Default)]
pub struct Particles {
    data: Option<Arc<FxData>>,
    loading: Option<Slot<FxData>>,
    failed: bool,
    /// Altaïr's effects by FX id, and the hay's.
    fx: HashMap<u32, FxDef>,
    hay_in: Option<FxDef>,
    hay_out: Option<FxDef>,
    world_loading: Option<Slot<WorldData>>,
    sources: Vec<Source>,
    defs: Vec<Def>,
    textures: Vec<Image>,
    tex_at: HashMap<u32, usize>,
    batch_keys: Vec<BatchKey>,
    batch_at: HashMap<BatchKey, usize>,
    cursors: HashMap<u8, Cursor>,
    bursts: Vec<Burst>,
    list: Vec<Particle>,
    /// Per batch: its mesh, material and the entity drawing it.
    batches: Vec<(Handle<Mesh>, Handle<ParticleSurface>, Entity)>,
    rng: u32,
    /// (`AC1_FPS=1`: the CPU time of `simulate` and `draw` summed, the frames, since the last log.)
    cost: (f32, u32),
}

impl Particles {
    fn frand(&mut self) -> f32 {
        let mut x = self.rng | 1;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        (x >> 8) as f32 / (1u32 << 24) as f32
    }

    fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.frand()
    }

    /// Textures loaded for effects, kept (each id once).
    fn add_textures(&mut self, loaded: Loaded) {
        for (id, image) in loaded {
            if !self.tex_at.contains_key(&id) {
                self.tex_at.insert(id, self.textures.len());
                self.textures.push(image);
            }
        }
    }

    /// An emitter to draw: its batch by its texture and modes (`None`: its texture not loaded).
    fn add_def(&mut self, em: &Emitter) -> Option<usize> {
        let tex = *self.tex_at.get(&em.texture)?;
        let blend = match em.blend {
            Blend::Additive => 1,
            Blend::Modulate => 3,
            _ => 4,
        };
        let glow = if em.glow { em.glow_intensity.clamp(0.0, 10.0) } else { 0.0 };
        let milli = |x: f32| (x * 1000.0).round() as u32;
        let batch = self.batch_of(BatchKey { tex, blend, lit: em.sprite_light(), glow: milli(glow), halo: false });
        let halo = (milli(glow) > 0).then(|| self.batch_of(BatchKey { tex, blend: 1, lit: false, glow: milli(glow), halo: true }));
        self.defs.push(Def { em: em.clone(), batch, halo });
        Some(self.defs.len() - 1)
    }

    /// A batch's index (made when new).
    fn batch_of(&mut self, key: BatchKey) -> usize {
        if let Some(b) = self.batch_at.get(&key) {
            return *b;
        }
        self.batch_keys.push(key);
        self.batch_at.insert(key, self.batch_keys.len() - 1);
        self.batch_keys.len() - 1
    }

    /// Start an effect's emitters at `at` (or each of its bones, from `bones`), facing up.
    fn play(&mut self, def: &FxDef, at: Vec3, bones: &[(Option<Entity>, Vec3)]) {
        for &d in &def.emitters {
            let local = Mat4::from_cols_array(&self.defs[d].em.local_matrix);
            if def.bones.is_empty() {
                self.bursts.push(Burst { def: d, frame: Mat4::from_translation(at) * game_to_bevy() * local, bone: None, sent: 0, t: 0.0 });
            } else {
                for &(bone, p) in bones {
                    self.bursts.push(Burst { def: d, frame: Mat4::from_translation(p) * game_to_bevy() * local, bone, sent: 0, t: 0.0 });
                }
            }
        }
    }

    /// A new particle of emitter `d` in `frame` (its local space, game axes, to the world), `age` s old already.
    fn spawn(&mut self, d: usize, frame: Mat4, src: u32, age: f32) -> bool {
        if self.list.len() >= MAX_PARTICLES {
            return false;
        }
        let em = &self.defs[d].em;
        let (shape, sp, dv, vel, batch, halo) = (em.shape, em.shape_params, em.direction_variation, em.velocity, self.defs[d].batch, self.defs[d].halo);
        let (phase_min, phase_max, mode) = (em.phase_min, em.phase_max, em.direction_mode);
        let [ax, ay, az] = em.acceleration;
        let (u, v, w) = (self.frand(), self.frand(), self.frand());
        let tau = std::f32::consts::TAU;
        // Where in its shape (game axes, the emitter's frame): 1 rectangle and 3 box across their sizes, 2 circle and 4
        // sphere within their radius, 5 cylinder within its radius and up its height (our reading of the sizes).
        let r = |x: f32| x.abs().min(20.0);
        let local = match shape {
            1 => Vec3::new((u - 0.5) * r(sp[0]), (v - 0.5) * r(sp[1]), 0.0),
            2 => {
                let (a, d) = (u * tau, v.sqrt() * r(sp[0]));
                Vec3::new(a.cos() * d, a.sin() * d, 0.0)
            }
            3 => Vec3::new((u - 0.5) * r(sp[0]), (v - 0.5) * r(sp[1]), (w - 0.5) * r(sp[2])),
            4 => {
                let (a, z) = (u * tau, v * 2.0 - 1.0);
                let s = (1.0 - z * z).max(0.0).sqrt();
                Vec3::new(s * a.cos(), s * a.sin(), z) * w.cbrt() * r(sp[0])
            }
            5 => {
                let (a, d) = (u * tau, v.sqrt() * r(sp[0]));
                Vec3::new(a.cos() * d, a.sin() * d, w * r(sp[1]))
            }
            _ => Vec3::ZERO,
        };
        // Which way: about the emitter's Z, tipped by the variation (between its two values), or from / to its centre.
        let tilt = self.range(dv[0], dv[1]).abs().min(std::f32::consts::PI);
        let turn = self.frand() * tau;
        let about_z = Vec3::new(tilt.sin() * turn.cos(), tilt.sin() * turn.sin(), tilt.cos());
        let dir = match mode {
            1 => local.try_normalize().unwrap_or(about_z),
            2 => (-local).try_normalize().unwrap_or(-about_z),
            _ => about_z,
        };
        let speed = self.range(vel[0], vel[1]);
        // (Its phases: one random for all three, as AC1's.)
        let mut keys = [0.0; 3];
        let mut end = 0.0;
        let r = self.frand();
        for (k, key) in keys.iter_mut().enumerate() {
            end += (phase_min[k] + (phase_max[k] - phase_min[k]) * r).max(0.0);
            *key = end;
        }
        if age >= keys[2] {
            return false;
        }
        // Its size and spin keys: between their min and max by one random, or one a key (`per_key_random`); its size
        // times the entity's scale.
        let (one, per_key) = (self.frand(), self.defs[d].em.per_key_random);
        let rs: [f32; 4] = std::array::from_fn(|_| if per_key { self.frand() } else { one });
        let (one, per_key) = (self.frand(), self.defs[d].em.per_key_random);
        let ws: [f32; 4] = std::array::from_fn(|_| if per_key { self.frand() } else { one });
        let scale = frame.x_axis.truncate().length();
        let em = &self.defs[d].em;
        let size = std::array::from_fn(|k| [0, 1].map(|i| (em.size_min[k][i] + (em.size_max[k][i] - em.size_min[k][i]) * rs[k]) * scale));
        let color = em.color.map(|c| c.map(|x| x as f32 / 255.0));
        let (spin_min, spin_max, rotation, frames, anim, frame_rate) = (em.spin_min, em.spin_max, em.rotation, em.frames, em.texture_anim, em.frame_rate);
        let along = em.along_velocity();
        let random_frame = em.random_first_frame();
        let wind = em.wind_factor();
        let floor = if em.kills_below_z_min() { em.z_min } else { f32::NEG_INFINITY };
        let spin: [f32; 4] = if along { [0.0; 4] } else { std::array::from_fn(|k| spin_min[k] + (spin_max[k] - spin_min[k]) * ws[k]) };
        let rot = if along { 0.0 } else { self.range(rotation[0], rotation[1]) };
        let frame0 = if random_frame { (self.frand() * 4096.0) as u32 } else { 0 };
        let acc = Vec3::new(ax, az, -ay) * scale;
        let v0 = frame.transform_vector3(dir).normalize_or_zero() * speed;
        let p0 = frame.transform_point3(local);
        self.list.push(Particle {
            batch,
            halo,
            src,
            pos: p0 + v0 * age + 0.5 * acc * age * age,
            vel: v0 + acc * age,
            acc,
            wind,
            floor,
            along,
            age,
            keys,
            size,
            color,
            rot: rot + spin[0] * age,
            spin,
            frames,
            anim,
            frame_rate,
            frame0,
        });
        true
    }
}

pub struct ParticlePlugin;

impl Plugin for ParticlePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(crate::particle_material::ParticleMaterialPlugin)
            .insert_resource(Particles { rng: 0x2007_1113, ..default() })
            .add_systems(Update, (load_fx, load_world_fx, fx_events.after(crate::character::animate), simulate, draw).chain());
    }
}

fn load_fx(mut p: ResMut<Particles>, dir: Res<crate::GameDir>) {
    if p.data.is_some() || p.failed {
        return;
    }
    let Some(slot) = p.loading.clone() else {
        let dir = dir.0.clone();
        p.loading = Some(on_thread(move || load(&dir)));
        return;
    };
    let Some(r) = take(&slot) else { return };
    p.loading = None;
    match r {
        Ok(mut d) => {
            p.add_textures(std::mem::take(&mut d.textures));
            let defs: Vec<Option<usize>> = d.emitters.iter().map(|e| p.add_def(e)).collect();
            let fx_def = |ems: &[usize], bones: Vec<&'static str>| FxDef { emitters: ems.iter().filter_map(|&i| defs[i]).collect(), bones };
            p.fx = d.fx.iter().map(|(id, ems)| (*id, fx_def(ems, vec![]))).collect();
            p.hay_in = d.hay_in.as_deref().map(|e| fx_def(e, HAY_BONES.to_vec()));
            p.hay_out = d.hay_out.as_deref().map(|e| fx_def(e, HAY_BONES.to_vec()));
            info!("fx: {} contact effects, {} sprite textures, haystack straw {}", d.fx.len(), p.textures.len(), d.hay_in.is_some());
            p.data = Some(Arc::new(d));
        }
        Err(e) => {
            warn!("fx: not loaded: {e:#}");
            p.failed = true;
        }
    }
}

/// A city's effects: loaded on a thread when the city has placed them, then run as sources.
fn load_world_fx(mut p: ResMut<Particles>, dir: Res<crate::GameDir>, world: Option<Res<WorldFx>>) {
    if let Some(world) = world.filter(|w| w.is_changed()) {
        let (dir, fx) = (dir.0.clone(), world.clone());
        p.sources.clear();
        p.world_loading = Some(on_thread(move || load_world(&dir, &fx)));
    }
    let Some(r) = p.world_loading.as_ref().and_then(take) else { return };
    p.world_loading = None;
    let mut d = match r {
        Ok(d) => d,
        Err(e) => {
            warn!("fx: the city's effects not loaded: {e:#}");
            return;
        }
    };
    p.add_textures(std::mem::take(&mut d.textures));
    let defs: Vec<Vec<Option<usize>>> = d.systems.iter().map(|(_, ems)| ems.iter().map(|e| p.add_def(e)).collect()).collect();
    // (`AC1_FX_LOG=1`: each placed effect, where it is.)
    let log = std::env::var("AC1_FX_LOG").is_ok();
    for (world, s) in &d.placed {
        if log {
            let at = world.transform_point3(Vec3::ZERO);
            info!("fx: {} at ({:.1}, {:.1}, {:.1})", d.systems[*s].0, at.x, at.y, at.z);
        }
        for &def in defs[*s].iter().flatten() {
            let frame = *world * Mat4::from_cols_array(&p.defs[def].em.local_matrix);
            let at = frame.transform_point3(Vec3::ZERO);
            let (near, range) = range_of(&p.defs[def].em);
            p.sources.push(Source { def, frame, at, near, range, fade: f32::NAN, wind: Vec3::ZERO, active: false, t: 0.0, carry: 0.0, alive: 0, sent: 0 });
        }
    }
    info!("fx: {} city emitters", p.sources.len());
}

/// The clip keys passed this frame start their effects (see the module header).
fn fx_events(mut p: ResMut<Particles>, level: Res<Level>, players: Query<(&Character, &Transform), With<Player>>, globals: Query<&GlobalTransform>) {
    let Some(data) = p.data.clone() else { return };
    let Ok((ch, root)) = players.single() else { return };
    let slots: Vec<u8> = ch.playing.iter().map(|q| q.slot).collect();
    p.cursors.retain(|k, _| slots.contains(k));
    let bone = |name: &str| ch.joint_named(name).and_then(|e| globals.get(e).ok().map(|g| (Some(e), g.translation())));
    let material = level.sound_material_at(root.translation);
    let ground = root.translation.y;
    for q in &ch.playing {
        let from = from_time(p.cursors.get(&q.slot), q);
        let keys = passed(&q.clip.anim.events, from, q.t, q.looping);
        for e in keys {
            let EventKind::Contact { ty } = e.kind else { continue };
            let name = CONTACT_HUMAN.get(ty as usize).copied().unwrap_or("");
            // Steps at the lower foot, on the ground; the rest at the root.
            let at = if name.contains("land") || name.contains("roll") {
                root.translation
            } else {
                [bone("LeftFoot"), bone("RightFoot")]
                    .into_iter()
                    .flatten()
                    .map(|b| b.1)
                    .min_by(|a, b| a.y.total_cmp(&b.y))
                    .unwrap_or(root.translation)
                    .with_y(ground)
            };
            for id in data.contacts.fx(material, ty).to_vec() {
                if let Some(def) = p.fx.remove(&id) {
                    p.play(&def, at, &[]);
                    p.fx.insert(id, def);
                }
            }
        }
        hay(&mut p, q, from, &bone);
        p.cursors.insert(q.slot, Cursor { clip: q.clip.clone(), t: q.t, synced: q.synced });
    }
}

/// The haystack's straw going in (at the dive's first sound key) and out (as the hop starts).
fn hay(p: &mut Particles, q: &Playing, from: Option<f32>, bone: &impl Fn(&str) -> Option<(Option<Entity>, Vec3)>) {
    let name = q.clip.name.as_str();
    let going_in = if HAY_IN.iter().any(|n| name.contains(n)) {
        let at = q.clip.anim.events.iter().find(|e| matches!(e.kind, EventKind::Audio { .. })).map_or(0.2, |e| e.time);
        let before = from.is_none_or(|f| f < at || f > q.t);
        (before && q.t >= at).then_some(true)
    } else if name.starts_with(HAY_OUT) && !name.contains("_tr_") {
        from.is_none().then_some(false)
    } else {
        None
    };
    let Some(going_in) = going_in else { return };
    let def = if going_in { p.hay_in.take() } else { p.hay_out.take() };
    let Some(def) = def else { return };
    let bones: Vec<(Option<Entity>, Vec3)> = def.bones.iter().filter_map(|b| bone(b).or_else(|| bone("Hips"))).collect();
    p.play(&def, Vec3::ZERO, &bones);
    if going_in {
        p.hay_in = Some(def);
    } else {
        p.hay_out = Some(def);
    }
}

/// Emit, move and age the particles: the bursts, then the city's sources near the camera.
fn simulate(
    mut p: ResMut<Particles>,
    time: Res<Time>,
    globals: Query<&GlobalTransform>,
    cams: Query<&GlobalTransform, With<Camera3d>>,
    wind: Option<Res<crate::wind::WindField>>,
) {
    let dt = time.delta_secs().min(0.1);
    if dt <= 0.0 {
        return;
    }
    let t0 = std::time::Instant::now();
    let mut bursts = std::mem::take(&mut p.bursts);
    for b in &mut bursts {
        if let Some(g) = b.bone.and_then(|e| globals.get(e).ok()) {
            b.frame.w_axis = g.translation().extend(1.0);
        }
        b.t += dt;
        let em = &p.defs[b.def].em;
        let due = ((b.t * em.rate.max(0.0)).ceil() as u32).min(em.max_particles);
        while b.sent < due {
            b.sent += 1;
            p.spawn(b.def, b.frame, NO_SOURCE, 0.0);
        }
    }
    bursts.retain(|b| b.sent < p.defs[b.def].em.max_particles);
    p.bursts = bursts;
    let eye_wind = match (&wind, cams.iter().next()) {
        (Some(w), Some(c)) => w.at(c.translation()),
        _ => Vec3::ZERO,
    };
    if let Some((eye, ahead)) = cams.iter().next().map(|c| (c.translation(), c.forward().as_vec3())) {
        let mut sources = std::mem::take(&mut p.sources);
        for (i, s) in sources.iter_mut().enumerate() {
            let to = s.at - eye;
            let d = to.length();
            // (AC1's: between its distances it fades in, outside them out, over `VIEW_FADE` s; at 0 it stops.)
            let inside = d >= s.near && d <= s.range;
            // (Its first frame: on or off at once, as if the city had been running.)
            s.fade = if s.fade.is_nan() { inside as u8 as f32 } else { (s.fade + if inside { dt } else { -dt } / VIEW_FADE).clamp(0.0, 1.0) };
            if s.fade <= 0.0 || (d > CLOSE && to.dot(ahead) < AHEAD * d) {
                s.active = false;
                continue;
            }
            let em = &p.defs[s.def].em;
            s.wind = match (&wind, em.wind_factor()) {
                (Some(w), f) if f != 0.0 => w.at(s.at) * f,
                _ => Vec3::ZERO,
            };
            let (rate, max, looping, delay) = (em.rate.max(0.0), em.max_particles, em.flow == FLOW_LOOP, em.delay.max(0.0));
            let life: f32 = (0..3).map(|k| em.phase_max[k].max(em.phase_min[k]).max(0.0)).sum();
            if !s.active {
                s.active = true;
                (s.t, s.carry, s.sent) = (0.0, 0.0, 0);
                // A loop coming into range: as if it had been running (its particles of every age).
                if looping && rate > 0.0 {
                    s.t = delay;
                    let n = ((rate * life) as u32).min(max).saturating_sub(s.alive);
                    for k in 0..n {
                        if p.spawn(s.def, s.frame, i as u32, (k as f32 + 0.5) / rate) {
                            s.alive += 1;
                        }
                    }
                }
            }
            s.t += dt;
            if s.t < delay || (!looping && s.sent >= max) {
                continue;
            }
            s.carry += rate * dt;
            while s.carry >= 1.0 && s.alive < max && (looping || s.sent < max) {
                s.carry -= 1.0;
                if !p.spawn(s.def, s.frame, i as u32, 0.0) {
                    break;
                }
                s.alive += 1;
                s.sent += 1;
            }
            s.carry = s.carry.min(1.0);
        }
        p.sources = sources;
    }
    let mut dead = vec![];
    let Particles { list, sources, .. } = &mut *p;
    for q in list.iter_mut() {
        let wind = match sources.get(q.src as usize) {
            Some(s) => s.wind,
            None => eye_wind * q.wind,
        };
        q.vel += (q.acc + wind) * dt;
        q.pos += q.vel * dt;
        q.age += dt;
        if q.pos.y <= q.floor {
            q.age = q.age.max(q.keys[2]);
        }
        let (k, f) = q.key();
        q.rot += (q.spin[k] + (q.spin[k + 1] - q.spin[k]) * f) * dt;
        if q.age >= q.keys[2] && q.src != NO_SOURCE {
            dead.push(q.src as usize);
        }
    }
    for s in dead {
        if let Some(s) = p.sources.get_mut(s) {
            s.alive = s.alive.saturating_sub(1);
        }
    }
    p.list.retain(|q| q.age < q.keys[2]);
    p.cost.0 += t0.elapsed().as_secs_f32();
}

/// Each batch's particles as one mesh of camera-facing quads, far ones first.
#[allow(clippy::too_many_arguments)]
fn draw(
    mut commands: Commands,
    mut p: ResMut<Particles>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<ParticleSurface>>,
    mut images: ResMut<Assets<Image>>,
    look: Res<crate::look::Look>,
    cams: Query<&GlobalTransform, With<Camera3d>>,
    mut vis: Query<&mut Visibility>,
) {
    let Some(cam) = cams.iter().next() else { return };
    let t0 = std::time::Instant::now();
    // The batches' meshes and materials, made as they come (each texture uploaded once).
    while p.batches.len() < p.batch_keys.len() {
        let key = p.batch_keys[p.batches.len()];
        let made = p.batches.iter().zip(&p.batch_keys).find(|(_, k)| k.tex == key.tex).and_then(|((_, m, _), _)| mats.get(m)).and_then(|m| {
            let srgb = m.extension.controls.options.z > 0.5;
            m.base.base_color_texture.clone().map(|t| (t, srgb))
        });
        let (image, srgb) = made.unwrap_or_else(|| {
            let image = p.textures[key.tex].clone();
            let srgb = image.texture_descriptor.format.is_srgb();
            (images.add(image), srgb)
        });
        let blend = match key.blend {
            1 => Blend::Additive,
            3 => Blend::Modulate,
            _ => Blend::Alpha,
        };
        let halo = if key.halo { 1.0 / (HALO * HALO) } else { 0.0 };
        let mat = mats.add(crate::particle_material::surface(image, srgb, blend, key.lit, key.glow as f32 / 1000.0, halo, look.world));
        let mesh = meshes.add(Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default()));
        let e = commands
            .spawn((
                Name::new("particles"),
                Mesh3d(mesh.clone()),
                MeshMaterial3d(mat.clone()),
                NoFrustumCulling,
                bevy::light::NotShadowCaster,
                bevy::light::NotShadowReceiver,
                Transform::default(),
                Visibility::Hidden,
            ))
            .id();
        p.batches.push((mesh, mat, e));
    }
    if look.is_changed() {
        for (_, m, _) in &p.batches {
            if let Some(mut m) = mats.get_mut(m) {
                m.extension.look = look.world;
            }
        }
    }
    let (right, up, eye) = (cam.right().as_vec3(), cam.up().as_vec3(), cam.translation());
    let mut order: Vec<(f32, usize)> = p.list.iter().enumerate().map(|(i, q)| (q.pos.distance_squared(eye), i)).collect();
    order.sort_by(|a, b| b.0.total_cmp(&a.0));
    let mut per_batch: Vec<Vec<usize>> = vec![vec![]; p.batches.len()];
    for (_, i) in order {
        let q = &p.list[i];
        for b in std::iter::once(q.batch).chain(q.halo) {
            if let Some(v) = per_batch.get_mut(b) {
                v.push(i);
            }
        }
    }
    for (bi, (mesh, _, e)) in p.batches.iter().enumerate() {
        let mine = &per_batch[bi];
        if let Ok(mut v) = vis.get_mut(*e) {
            v.set_if_neq(if mine.is_empty() { Visibility::Hidden } else { Visibility::Inherited });
        }
        if mine.is_empty() {
            continue;
        }
        let Some(mut m) = meshes.get_mut(mesh) else { continue };
        let n = mine.len();
        let grow = if p.batch_keys[bi].halo { HALO } else { 1.0 };
        let (mut pos, mut uv, mut col, mut idx) = (Vec::with_capacity(n * 4), Vec::with_capacity(n * 4), Vec::with_capacity(n * 4), Vec::with_capacity(n * 6));
        for (i, q) in mine.iter().map(|&i| &p.list[i]).enumerate() {
            let (k, f) = q.key();
            let [w, h] = [0, 1].map(|c| (q.size[k][c] + (q.size[k + 1][c] - q.size[k][c]) * f) * grow);
            let mut c: [f32; 4] = std::array::from_fn(|c| q.color[k][c] + (q.color[k + 1][c] - q.color[k][c]) * f);
            // (Its emitter's fade: AC1's `g_FadeFactor`, on the alpha.)
            c[3] *= p.sources.get(q.src as usize).map_or(1.0, |s| s.fade);
            // (Its axes: turned by its rotation in the view, or along its velocity and across it as seen, AC1's
            // `g_SpritesAtVector` shaders: the texture's width along the motion.)
            let (rx, ry) = match q.vel.cross(eye - q.pos).try_normalize().filter(|_| q.along) {
                Some(side) => (q.vel.normalize_or_zero(), side),
                None => {
                    let (s, co) = q.rot.sin_cos();
                    (right * co + up * s, up * co - right * s)
                }
            };
            let (hx, hy) = (rx * w * 0.5, ry * h * 0.5);
            // (Its frame's part of the texture: frames left to right, top to bottom.)
            let (fx, fy) = (q.frames[0].max(1) as u32, q.frames[1].max(1) as u32);
            let fr = q.frame();
            let (u0, v0) = ((fr % fx) as f32 / fx as f32, (fr / fx % fy) as f32 / fy as f32);
            let (du, dv) = (1.0 / fx as f32, 1.0 / fy as f32);
            for (corner, t) in [(-hx - hy, [u0, v0 + dv]), (hx - hy, [u0 + du, v0 + dv]), (hx + hy, [u0 + du, v0]), (-hx + hy, [u0, v0])] {
                pos.push((q.pos + corner).to_array());
                uv.push(t);
                col.push(c);
            }
            let b = (i * 4) as u32;
            idx.extend([b, b + 1, b + 2, b, b + 2, b + 3]);
        }
        m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32, 1.0, 0.0]; pos.len()]);
        m.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
        m.insert_attribute(Mesh::ATTRIBUTE_UV_0, uv);
        m.insert_attribute(Mesh::ATTRIBUTE_COLOR, col);
        m.insert_indices(Indices::U32(idx));
    }
    // (`AC1_FPS=1`: what the effects cost, every 120 frames.)
    p.cost.0 += t0.elapsed().as_secs_f32();
    p.cost.1 += 1;
    if p.cost.1 >= 120 {
        if std::env::var("AC1_FPS").is_ok() {
            let running = p.sources.iter().filter(|s| s.active).count();
            info!(
                "fx: {} particles, {running} of {} city emitters running, {:.2} ms a frame (CPU)",
                p.list.len(),
                p.sources.len(),
                p.cost.0 * 1000.0 / p.cost.1 as f32
            );
        }
        p.cost = (0.0, 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn particle(keys: [f32; 3]) -> Particle {
        Particle {
            batch: 0,
            halo: None,
            src: NO_SOURCE,
            pos: Vec3::ZERO,
            vel: Vec3::ZERO,
            acc: Vec3::ZERO,
            wind: 0.0,
            floor: f32::NEG_INFINITY,
            along: false,
            age: 0.0,
            keys,
            size: [[0.0; 2]; 4],
            color: [[0.0; 4]; 4],
            rot: 0.0,
            spin: [0.0; 4],
            frames: [0, 0],
            anim: 0,
            frame_rate: 30.0,
            frame0: 0,
        }
    }

    fn emitter() -> Emitter {
        let mut local_matrix = [0.0; 16];
        for k in 0..4 {
            local_matrix[5 * k] = 1.0;
        }
        Emitter {
            particle_type: 0,
            shape: 2,
            flow: 0,
            local_particles: false,
            flags: [0; 8],
            depth_fade: false,
            near_fade: false,
            depth_fade_scale: 200.0,
            near_fade_scale: 10.0,
            direction_mode: 0,
            camera_relative_spawn: false,
            texture_anim: 0,
            double_sided: false,
            blend: Blend::Alpha,
            lit: false,
            light_color: [255; 4],
            light_ambient: [0, 0, 0, 255],
            glow: false,
            glow_intensity: 0.0,
            effect: 0,
            effect_params: [0.0; 8],
            fog: false,
            alpha_test: None,
            light_pos: [0.0, 0.0, 1.0],
            local_matrix,
            shape_params: [0.1, 1.0, 1.0],
            z_min: 0.0,
            delay: 0.0,
            max_particles: 30,
            rate: 300.0,
            acceleration: [0.0, 0.0, -2.0],
            view_range: [0, 100],
            per_key_random: false,
            velocity: [0.1, 1.0],
            direction_variation: [2.0, 0.0],
            rotation: [0.0, 8.0],
            phase_min: [0.3, 0.9, 0.6],
            phase_max: [0.3, 0.9, 0.6],
            size_min: [[0.1; 2], [0.2; 2], [1.0; 2], [1.4; 2]],
            size_max: [[0.1; 2], [0.2; 2], [1.0; 2], [1.4; 2]],
            scale_min: [[1.0; 3]; 4],
            scale_max: [[1.0; 3]; 4],
            color: [[255, 255, 255, 36]; 4],
            spin_min: [0.0; 4],
            spin_max: [0.0; 4],
            mesh: 0,
            texture: 1,
            frames: [0, 0],
            frame_rate: 30.0,
            unknown_200: [0.0; 4],
            wind: (1, 0.05),
        }
    }

    #[test]
    fn a_particle_goes_key_to_key_at_its_phase_ends() {
        let mut q = particle([0.5, 1.5, 2.0]);
        assert_eq!(q.key(), (0, 0.0));
        q.age = 0.25;
        assert_eq!(q.key(), (0, 0.5));
        q.age = 1.0;
        assert_eq!(q.key(), (1, 0.5));
        q.age = 1.75;
        assert_eq!(q.key(), (2, 0.5));
        q.age = 3.0;
        assert_eq!(q.key(), (2, 1.0));
    }

    #[test]
    fn texture_frames_play_by_their_mode() {
        let mut q = particle([1.0, 2.0, 3.0]);
        q.frames = [2, 2];
        q.frame_rate = 10.0;
        q.age = 0.55;
        q.anim = 1;
        assert_eq!(q.frame(), 3, "once: held on the last");
        q.anim = 2;
        assert_eq!(q.frame(), 1, "loop: 5 % 4");
        q.anim = 3;
        assert_eq!(q.frame(), 1, "ping-pong: 0 1 2 3 2 1");
        q.anim = 0;
        q.frame0 = 6;
        assert_eq!(q.frame(), 2, "off: its own frame");
    }

    fn with_def(em: Emitter) -> Particles {
        let mut p = Particles { rng: 7, ..default() };
        p.defs.push(Def { em, batch: 0, halo: None });
        p
    }

    #[test]
    fn a_burst_sends_its_count_up_and_falling() {
        let mut p = with_def(emitter());
        let at = Vec3::new(1.0, 2.0, 3.0);
        for _ in 0..30 {
            p.spawn(0, Mat4::from_translation(at) * game_to_bevy(), NO_SOURCE, 0.0);
        }
        assert_eq!(p.list.len(), 30);
        for q in &p.list {
            assert!(q.pos.distance(at) <= 0.1 + 1e-5);
            assert!((0.1..=1.0 + 1e-5).contains(&q.vel.length()));
            assert!(q.vel.y >= -0.42, "within 2 rad of up: {:?}", q.vel);
            assert_eq!(q.acc, Vec3::new(0.0, -2.0, 0.0), "game Z down is Bevy Y down");
            assert!((q.keys[2] - 1.8).abs() < 1e-5);
        }
    }

    /// A fountain's jet: its local matrix turned half round X, so it leaves down; gravity stays world down.
    #[test]
    fn the_local_matrix_turns_the_jet_and_gravity_stays_down() {
        let mut em = emitter();
        em.direction_variation = [0.0, 0.0];
        em.velocity = [1.0, 1.0];
        em.local_matrix = [1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.5, 1.0];
        em.shape = 0;
        let mut p = with_def(em.clone());
        let frame = game_to_bevy() * Mat4::from_cols_array(&em.local_matrix);
        p.spawn(0, frame, NO_SOURCE, 0.0);
        let q = p.list[0];
        assert!((q.vel - Vec3::new(0.0, -1.0, 0.0)).length() < 1e-5, "down: {:?}", q.vel);
        assert!((q.pos - Vec3::new(0.0, 0.5, 0.0)).length() < 1e-5, "0.5 m up: {:?}", q.pos);
        assert_eq!(q.acc, Vec3::new(0.0, -2.0, 0.0));
    }

    /// A particle born `age` s ago is where its motion has taken it, and none past its life.
    #[test]
    fn a_prewarmed_particle_has_moved_on() {
        let mut em = emitter();
        em.shape = 0;
        em.direction_variation = [0.0, 0.0];
        em.velocity = [1.0, 1.0];
        let mut p = with_def(em);
        assert!(p.spawn(0, game_to_bevy(), NO_SOURCE, 1.0));
        let q = p.list[0];
        assert!(q.pos.y.abs() < 1e-5, "1 m up, 1 m back down by gravity 2: {:?}", q.pos);
        assert!((q.vel.y + 1.0).abs() < 1e-5);
        assert!(!p.spawn(0, game_to_bevy(), NO_SOURCE, 5.0), "past its 1.8 s");
    }

    /// AC1's spawn rules: one random for every size key unless `per_key_random`, sizes times the entity's scale, no
    /// rotation or spin along the velocity; the view range capped at `WORLD_RANGE`.
    #[test]
    fn sizes_spin_and_range_follow_the_emitter() {
        let mut em = emitter();
        em.size_min = [[0.0; 2], [1.0; 2], [2.0; 2], [3.0; 2]];
        em.size_max = [[1.0; 2], [2.0; 2], [3.0; 2], [4.0; 2]];
        let mut p = with_def(em.clone());
        p.spawn(0, Mat4::from_scale(Vec3::splat(2.0)), NO_SOURCE, 0.0);
        let q = p.list[0];
        let r = q.size[0][0] / 2.0;
        for k in 1..4 {
            assert!((q.size[k][0] / 2.0 - (k as f32 + r)).abs() < 1e-4, "one random, scale 2: {:?}", q.size);
        }
        em.per_key_random = true;
        em.flags[6] = 1;
        em.spin_min = [1.0; 4];
        em.spin_max = [2.0; 4];
        let mut p = with_def(em.clone());
        p.spawn(0, Mat4::IDENTITY, NO_SOURCE, 0.0);
        let q = p.list[0];
        let rs: Vec<f32> = (0..4).map(|k| q.size[k][0] - k as f32).collect();
        assert!(rs.iter().any(|r| (r - rs[0]).abs() > 1e-3), "a random a key: {rs:?}");
        assert!(q.along && q.rot == 0.0 && q.spin == [0.0; 4]);
        em.view_range = [5, 4_000_000];
        assert_eq!(range_of(&em), (5.0, WORLD_RANGE));
    }

    #[test]
    fn game_to_bevy_turns_z_up_to_y_up() {
        assert_eq!(game_to_bevy().transform_vector3(Vec3::Z), Vec3::Y);
        assert_eq!(game_to_bevy().transform_vector3(Vec3::Y), Vec3::NEG_Z);
    }
}
