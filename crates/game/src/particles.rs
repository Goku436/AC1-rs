//! AC1's dust and straw: its particle systems (`forge::fx`), drawn as camera-facing sprites of our own.
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
//!
//! An emitter (`forge::fx::Emitter`) sends out `max_particles` at `rate` a second, from within `shape_params[0]` of
//! its point, upward within `direction_variation` radians at `velocity` m/s, with its `acceleration`; each particle
//! lives its three phases, its size, colour and spin going key to key at their ends. Ours: the FX graph's operators
//! (their property overrides, `FXLinkedValue`s) are not run, and blend and lighting modes are not read: every sprite is
//! alpha blended, unlit, its texture times its colour key. Not in headless runs (no GPU).

use crate::animation::Playing;
use crate::character::{Character, Player};
use crate::level::Level;
use crate::sound::{Cursor, from_time, passed};
use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use forge::anim::{CONTACT_HUMAN, EventKind};
use forge::fx::{CLASS_FX, CLASS_PARTICLE_SYSTEM, Emitter};
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
/// At most this many particles at once (the oldest go first).
const MAX_PARTICLES: usize = 3000;
/// The sprites' brightness under the sky (unlit, AC1 lights them: ours by eye).
const LIGHT: f32 = 1.0;

/// An effect: its emitters (the texture index of each, `None`: not loaded), and the bones it plays at (none: the
/// contact point).
struct FxDef {
    emitters: Vec<(Emitter, Option<usize>)>,
    bones: Vec<&'static str>,
}

/// What the effects are made of: the contact table's FX by object id, the hay ones, the sprites' textures.
struct FxData {
    contacts: ContactTable,
    fx: HashMap<u32, Arc<FxDef>>,
    hay_in: Option<Arc<FxDef>>,
    hay_out: Option<Arc<FxDef>>,
    textures: Vec<Image>,
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
    let mut textures: Vec<Image> = vec![];
    let mut tex_at: HashMap<u32, Option<usize>> = HashMap::new();
    // An FX's systems: the entities it links (in order, each once), the particle systems they link.
    let mut def = |fx: &forge::Object, bones: Vec<&'static str>| -> FxDef {
        let mut emitters = vec![];
        let mut seen = vec![];
        for l in forge::fx::links(fx.body) {
            let Some(ent) = objs.get(&l).filter(|o| o.class == forge::datafile::CLASS_ENTITY && !seen.contains(&o.id)) else { continue };
            seen.push(ent.id);
            for p in forge::fx::links(ent.body) {
                let Some(ps) = objs.get(&p).filter(|o| o.class == CLASS_PARTICLE_SYSTEM) else { continue };
                match forge::fx::parse_particle_system(ps.body) {
                    Ok(em) => {
                        for e in em {
                            let tex = *tex_at.entry(e.texture).or_insert_with(|| {
                                let t = objs.get(&e.texture).and_then(|t| forge::decode_texture(t.body).ok())?;
                                textures.push(crate::assets::to_image(t));
                                Some(textures.len() - 1)
                            });
                            emitters.push((e, tex));
                        }
                    }
                    Err(e) => warn!("fx: particle system {}: {e:#}", ps.name),
                }
                break;
            }
        }
        FxDef { emitters, bones }
    };
    let mut fx = HashMap::new();
    for id in contacts.fx.iter().flatten().flatten() {
        if fx.contains_key(id) {
            continue;
        }
        if let Some(o) = objs.get(id).filter(|o| o.class == CLASS_FX) {
            fx.insert(*id, Arc::new(def(o, vec![])));
        }
    }
    let named = |n: &str| objs.values().find(|o| o.class == CLASS_FX && o.name == n);
    let hay_in = named("FX_Clip_Character_Hay_Bal(TopEntry)").map(|o| Arc::new(def(o, HAY_BONES.to_vec())));
    let hay_out = named("FX_Clip_Character_Hay_Bal(Side Exit)").map(|o| Arc::new(def(o, HAY_BONES.to_vec())));
    Ok(FxData { contacts, fx, hay_in, hay_out, textures })
}

type Slot = Arc<Mutex<Option<anyhow::Result<FxData>>>>;

/// One emitter sending out its particles.
struct Burst {
    em: Emitter,
    tex: usize,
    at: Vec3,
    /// The bone it follows while it emits, if any.
    bone: Option<Entity>,
    sent: u32,
    t: f32,
}

#[derive(Clone, Copy)]
struct Particle {
    tex: usize,
    pos: Vec3,
    vel: Vec3,
    acc: Vec3,
    age: f32,
    /// The ends of its phases (s from birth): keys 1, 2, 3.
    keys: [f32; 3],
    size: [[f32; 2]; 4],
    color: [[f32; 4]; 4],
    rot: f32,
    spin: [f32; 4],
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
}

#[derive(Resource, Default)]
pub struct Particles {
    data: Option<Arc<FxData>>,
    loading: Option<Slot>,
    failed: bool,
    cursors: HashMap<u8, Cursor>,
    bursts: Vec<Burst>,
    list: Vec<Particle>,
    /// Per texture: its mesh and the entity drawing it.
    batches: Vec<(Handle<Mesh>, Entity)>,
    rng: u32,
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

    /// Start an effect's emitters at `at` (or each of its bones, from `bones`).
    fn play(&mut self, def: &FxDef, at: Vec3, bones: &[(Option<Entity>, Vec3)]) {
        for (em, tex) in &def.emitters {
            let Some(tex) = *tex else { continue };
            if def.bones.is_empty() {
                self.bursts.push(Burst { em: em.clone(), tex, at, bone: None, sent: 0, t: 0.0 });
            } else {
                for &(bone, p) in bones {
                    self.bursts.push(Burst { em: em.clone(), tex, at: p, bone, sent: 0, t: 0.0 });
                }
            }
        }
    }

    /// A new particle of `em` at `at`.
    fn spawn(&mut self, em: &Emitter, tex: usize, at: Vec3) {
        let r = em.shape_params[0].abs().min(3.0) * if em.shape == 0 { 0.0 } else { 1.0 };
        let (a, d) = (self.frand() * std::f32::consts::TAU, self.frand().sqrt() * r);
        let pos = at + Vec3::new(a.cos() * d, 0.0, a.sin() * d);
        // Up (game Z), tipped within the direction variation.
        let spread = em.direction_variation[0].abs().max(em.direction_variation[1].abs()).min(std::f32::consts::PI);
        let (tilt, turn) = (self.frand() * spread, self.frand() * std::f32::consts::TAU);
        let dir = Vec3::new(tilt.sin() * turn.cos(), tilt.cos(), tilt.sin() * turn.sin());
        let speed = self.range(em.velocity[0], em.velocity[1]);
        let [ax, ay, az] = em.acceleration;
        let mut keys = [0.0; 3];
        let mut end = 0.0;
        for (k, key) in keys.iter_mut().enumerate() {
            end += self.range(em.phase_min[k], em.phase_max[k]).max(0.0);
            *key = end;
        }
        let s = self.frand();
        let size = std::array::from_fn(|k| [0, 1].map(|i| em.size_min[k][i] + (em.size_max[k][i] - em.size_min[k][i]) * s));
        let color = em.color.map(|c| {
            let l = Color::srgba_u8(c[0], c[1], c[2], c[3]).to_linear();
            [l.red * LIGHT, l.green * LIGHT, l.blue * LIGHT, l.alpha]
        });
        let w = self.frand();
        let spin = std::array::from_fn(|k| em.spin_min[k] + (em.spin_max[k] - em.spin_min[k]) * w);
        let rot = self.range(em.rotation[0], em.rotation[1]);
        if self.list.len() >= MAX_PARTICLES {
            self.list.remove(0);
        }
        self.list.push(Particle { tex, pos, vel: dir * speed, acc: Vec3::new(ax, az, -ay), age: 0.0, keys, size, color, rot, spin });
    }
}

pub struct ParticlePlugin;

impl Plugin for ParticlePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Particles { rng: 0x2007_1113, ..default() })
            .add_systems(Update, (load_fx, fx_events.after(crate::character::animate), simulate, draw).chain());
    }
}

fn load_fx(mut p: ResMut<Particles>, dir: Res<crate::GameDir>) {
    if p.data.is_some() || p.failed {
        return;
    }
    let Some(slot) = p.loading.clone() else {
        let dir = dir.0.clone();
        let slot: Slot = Arc::new(Mutex::new(None));
        let out = slot.clone();
        std::thread::spawn(move || {
            let r = load(&dir);
            if let Ok(mut o) = out.lock() {
                *o = Some(r);
            }
        });
        p.loading = Some(slot);
        return;
    };
    let Some(r) = slot.lock().ok().and_then(|mut o| o.take()) else { return };
    p.loading = None;
    match r {
        Ok(d) => {
            info!("fx: {} contact effects, {} sprite textures, haystack straw {}", d.fx.len(), d.textures.len(), d.hay_in.is_some());
            p.data = Some(Arc::new(d));
        }
        Err(e) => {
            warn!("fx: not loaded: {e:#}");
            p.failed = true;
        }
    }
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
                if let Some(def) = data.fx.get(&id) {
                    p.play(def, at, &[]);
                }
            }
        }
        hay(&mut p, &data, q, from, &bone);
        p.cursors.insert(q.slot, Cursor { clip: q.clip.clone(), t: q.t, synced: q.synced });
    }
}

/// The haystack's straw going in (at the dive's first sound key) and out (as the hop starts).
fn hay(p: &mut Particles, data: &FxData, q: &Playing, from: Option<f32>, bone: &impl Fn(&str) -> Option<(Option<Entity>, Vec3)>) {
    let name = q.clip.name.as_str();
    let def = if HAY_IN.iter().any(|n| name.contains(n)) {
        let at = q.clip.anim.events.iter().find(|e| matches!(e.kind, EventKind::Audio { .. })).map_or(0.2, |e| e.time);
        let before = from.is_none_or(|f| f < at || f > q.t);
        (before && q.t >= at).then_some(data.hay_in.as_ref()).flatten()
    } else if name.starts_with(HAY_OUT) && !name.contains("_tr_") {
        from.is_none().then_some(data.hay_out.as_ref()).flatten()
    } else {
        None
    };
    let Some(def) = def.cloned() else { return };
    let bones: Vec<(Option<Entity>, Vec3)> = def.bones.iter().filter_map(|b| bone(b).or_else(|| bone("Hips"))).collect();
    p.play(&def, Vec3::ZERO, &bones);
}

/// Emit, move and age the particles.
fn simulate(mut p: ResMut<Particles>, time: Res<Time>, globals: Query<&GlobalTransform>) {
    let dt = time.delta_secs().min(0.1);
    if dt <= 0.0 {
        return;
    }
    let mut bursts = std::mem::take(&mut p.bursts);
    for b in &mut bursts {
        if let Some(g) = b.bone.and_then(|e| globals.get(e).ok()) {
            b.at = g.translation();
        }
        b.t += dt;
        let due = ((b.t * b.em.rate.max(0.0)).ceil() as u32).min(b.em.max_particles);
        while b.sent < due {
            b.sent += 1;
            let (em, tex, at) = (b.em.clone(), b.tex, b.at);
            p.spawn(&em, tex, at);
        }
    }
    bursts.retain(|b| b.sent < b.em.max_particles);
    p.bursts = bursts;
    for q in &mut p.list {
        q.vel += q.acc * dt;
        q.pos += q.vel * dt;
        q.age += dt;
        let (k, f) = q.key();
        q.rot += (q.spin[k] + (q.spin[k + 1] - q.spin[k]) * f) * dt;
    }
    p.list.retain(|q| q.age < q.keys[2]);
}

/// Each texture's particles as one mesh of camera-facing quads, far ones first.
fn draw(
    mut commands: Commands,
    mut p: ResMut<Particles>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    cams: Query<&GlobalTransform, With<Camera3d>>,
    mut vis: Query<&mut Visibility>,
) {
    let Some(data) = p.data.clone() else { return };
    let Some(cam) = cams.iter().next() else { return };
    // The batches, made once the textures are in.
    if p.batches.is_empty() && !data.textures.is_empty() {
        for t in &data.textures {
            let image = images.add(t.clone());
            let mat = mats.add(StandardMaterial {
                base_color_texture: Some(image),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                cull_mode: None,
                double_sided: true,
                fog_enabled: true,
                ..default()
            });
            let mesh = meshes.add(Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default()));
            let e = commands
                .spawn((
                    Name::new("particles"),
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(mat),
                    NoFrustumCulling,
                    bevy::light::NotShadowCaster,
                    Transform::default(),
                    Visibility::Hidden,
                ))
                .id();
            p.batches.push((mesh, e));
        }
    }
    let (right, up, eye) = (cam.right().as_vec3(), cam.up().as_vec3(), cam.translation());
    let mut order: Vec<usize> = (0..p.list.len()).collect();
    order.sort_by(|&a, &b| p.list[b].pos.distance_squared(eye).total_cmp(&p.list[a].pos.distance_squared(eye)));
    for (ti, (mesh, e)) in p.batches.iter().enumerate() {
        let mine: Vec<&Particle> = order.iter().map(|&i| &p.list[i]).filter(|q| q.tex == ti).collect();
        if let Ok(mut v) = vis.get_mut(*e) {
            v.set_if_neq(if mine.is_empty() { Visibility::Hidden } else { Visibility::Inherited });
        }
        if mine.is_empty() {
            continue;
        }
        let Some(mut m) = meshes.get_mut(mesh) else { continue };
        let n = mine.len();
        let (mut pos, mut uv, mut col, mut nrm, mut idx) =
            (Vec::with_capacity(n * 4), Vec::with_capacity(n * 4), Vec::with_capacity(n * 4), Vec::with_capacity(n * 4), Vec::with_capacity(n * 6));
        for (i, q) in mine.into_iter().enumerate() {
            let (k, f) = q.key();
            let [w, h] = [0, 1].map(|c| q.size[k][c] + (q.size[k + 1][c] - q.size[k][c]) * f);
            let c: [f32; 4] = std::array::from_fn(|c| q.color[k][c] + (q.color[k + 1][c] - q.color[k][c]) * f);
            let (s, co) = q.rot.sin_cos();
            let (rx, ry) = (right * co + up * s, up * co - right * s);
            let (hx, hy) = (rx * w * 0.5, ry * h * 0.5);
            for (corner, t) in [(-hx - hy, [0.0, 1.0]), (hx - hy, [1.0, 1.0]), (hx + hy, [1.0, 0.0]), (-hx + hy, [0.0, 0.0])] {
                pos.push((q.pos + corner).to_array());
                uv.push(t);
                col.push(c);
                nrm.push((eye - q.pos).normalize_or(Vec3::Y).to_array());
            }
            let b = (i * 4) as u32;
            idx.extend([b, b + 1, b + 2, b, b + 2, b + 3]);
        }
        m.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
        m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, nrm);
        m.insert_attribute(Mesh::ATTRIBUTE_UV_0, uv);
        m.insert_attribute(Mesh::ATTRIBUTE_COLOR, col);
        m.insert_indices(Indices::U32(idx));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn particle(keys: [f32; 3]) -> Particle {
        Particle {
            tex: 0,
            pos: Vec3::ZERO,
            vel: Vec3::ZERO,
            acc: Vec3::ZERO,
            age: 0.0,
            keys,
            size: [[0.0; 2]; 4],
            color: [[0.0; 4]; 4],
            rot: 0.0,
            spin: [0.0; 4],
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
    fn a_burst_sends_its_count_at_its_rate_up_and_falling() {
        let mut p = Particles { rng: 7, ..default() };
        let em = Emitter {
            shape: 2,
            shape_params: [0.1, 1.0, 1.0],
            max_particles: 30,
            rate: 300.0,
            acceleration: [0.0, 0.0, -2.0],
            velocity: [0.1, 1.0],
            direction_variation: [2.0, 0.0],
            rotation: [0.0, 8.0],
            phase_min: [0.3, 0.9, 0.6],
            phase_max: [0.3, 0.9, 0.6],
            size_min: [[0.1; 2], [0.2; 2], [1.0; 2], [1.4; 2]],
            size_max: [[0.1; 2], [0.2; 2], [1.0; 2], [1.4; 2]],
            color: [[255, 255, 255, 36]; 4],
            spin_min: [0.0; 4],
            spin_max: [0.0; 4],
            mesh: 0,
            texture: 1,
            frames: [0, 0],
        };
        for _ in 0..30 {
            p.spawn(&em, 0, Vec3::new(1.0, 2.0, 3.0));
        }
        assert_eq!(p.list.len(), 30);
        for q in &p.list {
            assert!(q.pos.distance(Vec3::new(1.0, 2.0, 3.0)) <= 0.1 + 1e-5);
            assert!((0.1..=1.0 + 1e-5).contains(&q.vel.length()));
            assert_eq!(q.acc, Vec3::new(0.0, -2.0, 0.0), "game Z down is Bevy Y down");
            assert!((q.keys[2] - 1.8).abs() < 1e-5);
        }
    }
}
