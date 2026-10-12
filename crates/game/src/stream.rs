//! What a city draws, and when (`city.rs` builds the gameplay world in full; this is only what is rendered).
//!
//! Each placed mesh is an instance with AC1's levels of detail (`forge::lod`: an entity's `LODSelector`, full detail
//! near, its `_LOD_*` meshes farther, nothing past the last for some): the level drawn is picked by the camera's distance
//! to the instance's box, as the game does. Instances belong to the cells of AC1's streaming grid (`forge::world`: the
//! `Cell<k>_DataBlock` file an entity comes from; big objects sit in coarse cells), and a cell is drawn while it overlaps
//! a square box round the camera, every level, as AC1's streamer loads cells round the player. Past that the city is
//! drawn by the world's stand-ins (`FakeEntities`: per finest cell, its part of the block's merged low-detail mesh),
//! hidden cell by cell once that cell's own instances are drawn.
//!
//! A vertex budget caps what the drawn instances add up to: nearest first, and once it is spent the rest draw at their
//! coarsest level. Meshes are built for the GPU on loader threads, nearest first, and handed to the GPU at most
//! `UPLOAD_BUDGET` vertices a frame; an instance keeps its old level (or its cell's stand-in shows) until its new mesh is
//! there, so nothing near goes missing. A mesh no drawn instance uses for `EVICT_AFTER` seconds leaves the GPU. How far,
//! how fine and how much: `Detail` (`AC1_DETAIL`).
//!
//! The streaming follows Banned445's AC1-Movement-Rewritten (MIT; `src/city/stream.rs`, `fakes.rs`): grid cells round the
//! player, LOD by distance with a vertex budget, the fake meshes on the horizon. Written afresh here over ac1-rs' own
//! city loader.

use crate::city_material::{CitySurface, NO_AO};
use bevy::asset::RenderAssetUsages;
use bevy::camera::primitives::Aabb;
use bevy::camera::visibility::NoAutoAabb;
use bevy::light::NotShadowCaster;
use bevy::mesh::MeshTag;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::render::storage::ShaderBuffer;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Condvar, Mutex, mpsc};

/// How often the drawn set is planned (s).
const PLAN_EVERY: f32 = 0.25;
/// The most vertices of built meshes handed to the GPU in a frame (spreads the cost of coming into a district).
const UPLOAD_BUDGET: usize = 600_000;
/// A mesh no drawn instance uses leaves the GPU after this long (s).
const EVICT_AFTER: f32 = 5.0;

/// A mesh a city draws: its body (re-read when it is built) and what it costs.
pub struct StoredMesh {
    pub body: Arc<[u8]>,
    pub verts: usize,
    /// Its parts' material ids (before an entity's overrides).
    pub materials: Vec<u32>,
    /// A skinned prop's skeleton to stand it in (`AC1_POSE_PROPS`).
    pub pose: Option<Arc<forge::skeleton::Skeleton>>,
}

/// A level of an instance: its mesh (an index into `RenderStore::meshes`; None draws nothing) up to `until` metres.
#[derive(Clone, Copy, Debug)]
pub struct Lod {
    pub mesh: Option<usize>,
    pub until: f32,
}

pub struct Instance {
    /// Bevy space.
    pub world: Mat4,
    pub lo: Vec3,
    pub hi: Vec3,
    pub levels: Vec<Lod>,
    /// Per level, a material slot (`RenderStore::materials`) per part of its mesh.
    pub parts: Vec<Vec<u32>>,
    /// Per level, where its mesh's baked occlusion starts in `RenderStore::ao` (its first vertex's byte), if it has one.
    pub ao: Vec<Option<u32>>,
    /// Whether it casts shadows (not an assassins' bureau's outside shell, whose roof would shade the open courtyard
    /// inside: AC1 swaps it for the inside one there).
    pub shadow: bool,
}

/// A grid cell (game x, y).
pub struct Cell {
    pub min: Vec2,
    pub max: Vec2,
    pub instances: Vec<usize>,
}

/// A stand-in piece: its mesh and the cell whose objects it stands for (None: always drawn).
pub struct FakePiece {
    pub mesh: Mesh,
    pub material: u32,
    pub world: Mat4,
    pub cell: Option<usize>,
}

/// Everything the loader found to draw.
#[derive(Default)]
pub struct RenderStore {
    pub meshes: Vec<StoredMesh>,
    pub instances: Vec<Instance>,
    pub cells: Vec<Cell>,
    pub materials: Vec<Handle<StandardMaterial>>,
    pub fakes: Vec<FakePiece>,
    /// Every placement's baked occlusion, a byte per vertex (`forge::instance`).
    pub ao: Vec<u8>,
}

/// The parts of a mesh drawn: its submeshes whose index and vertex ranges hold (submesh, material id, vertices).
pub fn mesh_parts(pm: &forge::mesh::Mesh) -> Vec<(usize, u32, usize)> {
    pm.submeshes
        .iter()
        .enumerate()
        .filter(|(_, s)| part_range(pm, s).is_some())
        .map(|(si, s)| (si, pm.materials.get(si).copied().unwrap_or(0), s.vertex_count as usize))
        .collect()
}

/// A submesh's vertex range and its indices from 0, if they hold.
fn part_range(pm: &forge::mesh::Mesh, s: &forge::mesh::Submesh) -> Option<(std::ops::Range<usize>, Vec<u32>)> {
    let range = s.first_vertex as usize..(s.first_vertex + s.vertex_count) as usize;
    let idx = pm.indices.get(s.first_index as usize..(s.first_index + s.tri_count * 3) as usize)?;
    let idx: Vec<u32> = idx.iter().map(|&i| i.saturating_sub(s.first_vertex)).collect();
    (pm.positions.get(range.clone()).is_some()
        && pm.normals.get(range.clone()).is_some()
        && pm.uvs.get(range.clone()).is_some()
        && idx.iter().all(|&i| (i as usize) < range.len()))
    .then_some((range, idx))
}

/// A mesh's parts for the GPU (as `mesh_parts`), each with its box and its first vertex in the whole mesh; render-world
/// only (no copy kept here).
pub fn build_parts(m: &StoredMesh) -> Vec<(Mesh, Aabb, u32)> {
    let Ok(mut pm) = forge::mesh::parse_mesh(&m.body) else { return vec![] };
    if let Some(s) = &m.pose {
        crate::city::pose_skinned(&mut pm, s);
    }
    let mut out = vec![];
    for s in &pm.submeshes {
        let Some((range, idx)) = part_range(&pm, s) else { continue };
        let pos = &pm.positions[range.clone()];
        let mut bm = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD);
        bm.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos.to_vec());
        bm.insert_attribute(Mesh::ATTRIBUTE_NORMAL, pm.normals[range.clone()].to_vec());
        bm.insert_attribute(Mesh::ATTRIBUTE_UV_0, pm.uvs[range].to_vec());
        bm.insert_indices(Indices::U32(idx));
        // (Tangents for normal maps.)
        let _ = bm.generate_tangents();
        let (lo, hi) = pos.iter().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(a, b), p| (a.min(Vec3::from(*p)), b.max(Vec3::from(*p))));
        out.push((bm, Aabb::from_min_max(lo, hi), s.first_vertex));
    }
    out
}

/// A mesh on the GPU: its parts, how many drawn instances use it, and how long none has.
struct Built {
    /// Each part, its box and its first vertex in the whole mesh.
    parts: Vec<(Handle<Mesh>, Aabb, u32)>,
    users: u32,
    idle: f32,
    verts: usize,
}

/// A built mesh's parts, each with its box and first vertex.
type Parts = Vec<(Mesh, Aabb, u32)>;

/// What an instance draws now: its level, that level's mesh and the entities drawing it.
type Shown = (usize, Option<usize>, Vec<Entity>);

/// A queue of meshes to build (nearest first) and the threads building them.
type Jobs = Arc<(Mutex<(VecDeque<usize>, bool)>, Condvar)>;

/// Loader threads: they build meshes for the GPU off the main thread.
struct Loader {
    jobs: Jobs,
    done: Mutex<mpsc::Receiver<(usize, Parts)>>,
}

impl Loader {
    fn new(meshes: Arc<Vec<StoredMesh>>) -> Self {
        let jobs: Jobs = Arc::default();
        let (tx, rx) = mpsc::channel();
        // (Two cores left to the game itself.)
        let threads = std::thread::available_parallelism().map_or(2, |n| n.get().saturating_sub(2).clamp(1, 6));
        for _ in 0..threads {
            let (jobs, tx, meshes) = (jobs.clone(), tx.clone(), meshes.clone());
            std::thread::spawn(move || {
                loop {
                    let job = {
                        let (lock, cv) = &*jobs;
                        let Ok(mut q) = lock.lock() else { return };
                        loop {
                            if q.1 {
                                return;
                            }
                            if let Some(j) = q.0.pop_front() {
                                break j;
                            }
                            let Ok(next) = cv.wait(q) else { return };
                            q = next;
                        }
                    };
                    if tx.send((job, build_parts(&meshes[job]))).is_err() {
                        return;
                    }
                }
            });
        }
        Self { jobs, done: Mutex::new(rx) }
    }

    /// Replace the queue (nearest first).
    fn request(&self, list: Vec<usize>) {
        let (lock, cv) = &*self.jobs;
        if let Ok(mut q) = lock.lock() {
            q.0 = list.into();
            cv.notify_all();
        }
    }
}

impl Drop for Loader {
    fn drop(&mut self) {
        let (lock, cv) = &*self.jobs;
        if let Ok(mut q) = lock.lock() {
            q.1 = true;
        }
        cv.notify_all();
    }
}

/// The city as drawn now.
#[derive(Resource)]
pub struct CityRender {
    store: RenderStore,
    meshes: Arc<Vec<StoredMesh>>,
    detail: Detail,
    loader: Loader,
    built: HashMap<usize, Built>,
    /// Meshes asked of the loader and not back yet.
    pending: HashSet<usize>,
    /// Per instance: the level drawn, its mesh and its entities.
    shown: Vec<Option<Shown>>,
    /// Per instance: the level wanted (None: not drawn).
    want: Vec<Option<usize>>,
    /// Per cell: the stand-in pieces that hide while its own objects are drawn, and whether they are hidden.
    fakes: HashMap<usize, (Vec<Entity>, bool)>,
    timer: f32,
    /// Counters for the log: instances drawn, their vertices.
    drawn: (usize, usize),
    /// AC1's city shader's materials, one per `RenderStore::materials` slot (made on the first frame; empty with
    /// `AC1_PLAIN_CITY`), and the stand-ins' entities with their slots (switched to them then).
    surfaces: Vec<Handle<CitySurface>>,
    fake_slots: Vec<(Entity, u32)>,
    made: bool,
}

impl CityRender {
    /// Spawn the stand-ins (shown) and keep the store.
    pub fn new(store: RenderStore, materials: Vec<Handle<StandardMaterial>>, detail: Detail, commands: &mut Commands, meshes: &mut Assets<Mesh>) -> Self {
        let mut fakes: HashMap<usize, (Vec<Entity>, bool)> = HashMap::new();
        let mut fake_slots = vec![];
        let mut store = store;
        store.materials = materials;
        // (At full detail everything is drawn: no stand-ins.)
        for f in std::mem::take(&mut store.fakes).into_iter().filter(|_| !detail.full) {
            let Some(mat) = store.materials.get(f.material as usize).cloned() else { continue };
            let e = commands.spawn((Mesh3d(meshes.add(f.mesh)), MeshMaterial3d(mat), Transform::from_matrix(f.world), NotShadowCaster)).id();
            fake_slots.push((e, f.material));
            if let Some(c) = f.cell {
                fakes.entry(c).or_default().0.push(e);
            }
        }
        let n = store.instances.len();
        let stored = Arc::new(std::mem::take(&mut store.meshes));
        Self {
            store,
            loader: Loader::new(stored.clone()),
            meshes: stored,
            detail,
            built: HashMap::new(),
            pending: HashSet::new(),
            shown: vec![None; n],
            want: vec![None; n],
            fakes,
            timer: 0.0,
            drawn: (0, 0),
            surfaces: vec![],
            fake_slots,
            made: false,
        }
    }

    /// A new detail (the graphics menu): its draw distance, LOD and vertex budget from the next plan on. Its texture
    /// size and `full` (no stand-ins) stay as loaded: they take a restart.
    pub fn set_detail(&mut self, d: Detail) {
        if self.detail.full || d.full {
            return;
        }
        let d = Detail { max_texture: self.detail.max_texture, ..d };
        if (d.view_box, d.lod_scale, d.vertex_budget) != (self.detail.view_box, self.detail.lod_scale, self.detail.vertex_budget) {
            self.detail = d;
            self.timer = 0.0;
        }
    }

    /// Which level each instance wants with the camera at `eye`: see the module docs.
    fn plan(&mut self, eye: Vec3) {
        let p = Vec2::new(eye.x, -eye.z);
        let d = self.detail;
        let (lo, hi) = (p - Vec2::splat(d.view_box), p + Vec2::splat(d.view_box));
        let mut near: Vec<(f32, usize)> = vec![];
        for c in &self.store.cells {
            let on = c.min.x < hi.x && c.max.x > lo.x && c.min.y < hi.y && c.max.y > lo.y;
            for &i in &c.instances {
                self.want[i] = None;
                if on {
                    let inst = &self.store.instances[i];
                    near.push((eye.distance(eye.clamp(inst.lo, inst.hi)), i));
                }
            }
        }
        near.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut spent = 0usize;
        let mut drawn = 0;
        let mut ask: Vec<usize> = vec![];
        let mut asked: HashSet<usize> = HashSet::new();
        for (dist, i) in near {
            let inst = &self.store.instances[i];
            let mut level = pick_level(&inst.levels, dist * self.detail.lod_scale);
            // (Out of budget: its coarsest level that draws, or the one picked if that is coarser.)
            if spent > self.detail.vertex_budget
                && let Some(c) = inst.levels.iter().rposition(|l| l.mesh.is_some())
            {
                level = Some(c.max(level.unwrap_or(0)));
            }
            let mesh = level.and_then(|l| inst.levels[l].mesh);
            let verts = mesh.map_or(0, |m| self.meshes[m].verts);
            spent += verts;
            drawn += usize::from(verts > 0);
            self.want[i] = level;
            // (Its mesh, to build: nearest first.)
            if let Some(m) = mesh
                && !self.built.contains_key(&m)
                && asked.insert(m)
            {
                ask.push(m);
            }
        }
        self.drawn = (drawn, spent);
        self.pending = asked;
        self.loader.request(ask);
    }
}

/// What the city shader's materials are made from.
#[derive(bevy::ecs::system::SystemParam)]
pub struct LookAssets<'w> {
    standard: Res<'w, Assets<StandardMaterial>>,
    surfaces: ResMut<'w, Assets<CitySurface>>,
    buffers: ResMut<'w, Assets<ShaderBuffer>>,
    look: Res<'w, crate::look::Look>,
}

/// AC1's city shader's materials over the city's (`city_material`), the occlusion bytes into their buffer, the
/// stand-ins switched to them (with no occlusion of their own).
fn make_surfaces(city: &mut CityRender, commands: &mut Commands, a: &mut LookAssets) {
    if !crate::city_material::enabled() {
        return;
    }
    let ao = a.buffers.add(crate::city_material::ao_buffer(&std::mem::take(&mut city.store.ao)));
    city.surfaces = city
        .store
        .materials
        .iter()
        .map(|h| {
            let base = a.standard.get(h).cloned().unwrap_or_default();
            a.surfaces.add(CitySurface { base, extension: crate::city_material::CityLayers { look: a.look.world, ao: ao.clone() } })
        })
        .collect();
    for &(e, slot) in &city.fake_slots {
        if let Some(s) = city.surfaces.get(slot as usize) {
            commands.entity(e).remove::<MeshMaterial3d<StandardMaterial>>().insert((MeshMaterial3d(s.clone()), MeshTag(NO_AO)));
        }
    }
}

/// The level drawn at LOD distance `d`: the first whose `until` reaches it, past the last the last (`forge::lod::pick`).
fn pick_level(levels: &[Lod], d: f32) -> Option<usize> {
    levels.iter().position(|l| d <= l.until).or(levels.len().checked_sub(1))
}

/// Per frame: take the meshes the loader built (within `UPLOAD_BUDGET`), plan (every `PLAN_EVERY`), bring the drawn
/// instances to their wanted level where its mesh is there, let unused meshes go, then hide or show the stand-ins.
pub fn stream_city(
    mut commands: Commands,
    time: Res<Time>,
    city: Option<ResMut<CityRender>>,
    cams: Query<&Transform, With<Camera3d>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut log: Local<f32>,
    mut looks: LookAssets,
) {
    let Some(mut city) = city else { return };
    let Ok(cam) = cams.single() else { return };
    let city = &mut *city;
    if !city.made {
        city.made = true;
        make_surfaces(city, &mut commands, &mut looks);
    }
    let dt = time.delta_secs();
    let mut budget = UPLOAD_BUDGET;
    while budget > 0 {
        let Some((m, parts)) = city.loader.done.lock().ok().and_then(|r| r.try_recv().ok()) else { break };
        city.pending.remove(&m);
        if city.built.contains_key(&m) {
            continue;
        }
        let verts = city.meshes[m].verts;
        budget = budget.saturating_sub(verts);
        let parts = parts.into_iter().map(|(mesh, aabb, first)| (meshes.add(mesh), aabb, first)).collect();
        city.built.insert(m, Built { parts, users: 0, idle: 0.0, verts });
    }
    city.timer -= dt;
    if city.timer <= 0.0 {
        city.timer = PLAN_EVERY;
        city.plan(cam.translation);
    }
    for i in 0..city.want.len() {
        let want = city.want[i];
        if city.shown[i].as_ref().map(|s| s.0) == want {
            continue;
        }
        let inst = &city.store.instances[i];
        let mesh = want.and_then(|l| inst.levels[l].mesh);
        // (Not built yet: the old level stays meanwhile.)
        if mesh.is_some_and(|m| !city.built.contains_key(&m)) {
            continue;
        }
        if let Some((_, old_mesh, old)) = city.shown[i].take() {
            for e in old {
                commands.entity(e).despawn();
            }
            if let Some(b) = old_mesh.and_then(|m| city.built.get_mut(&m)) {
                b.users = b.users.saturating_sub(1);
            }
        }
        let Some(l) = want else { continue };
        let tf = Transform::from_matrix(inst.world);
        let mut entities = vec![];
        if let Some(b) = mesh.and_then(|m| city.built.get_mut(&m)) {
            b.users += 1;
            for (k, (h, aabb, first)) in b.parts.iter().enumerate() {
                let Some(&slot) = inst.parts[l].get(k) else { continue };
                let mut e = commands.spawn((Mesh3d(h.clone()), tf, *aabb, NoAutoAabb));
                if !inst.shadow {
                    e.insert(NotShadowCaster);
                }
                match city.surfaces.get(slot as usize) {
                    Some(surface) => {
                        let tag = inst.ao.get(l).copied().flatten().map_or(NO_AO, |at| at + first);
                        e.insert((MeshMaterial3d(surface.clone()), MeshTag(tag)));
                    }
                    None => {
                        let Some(mat) = city.store.materials.get(slot as usize) else { continue };
                        e.insert(MeshMaterial3d(mat.clone()));
                    }
                }
                entities.push(e.id());
            }
        }
        city.shown[i] = Some((l, mesh, entities));
    }
    // Meshes nothing draws: off the GPU after a while.
    let mut gone = vec![];
    for (m, b) in city.built.iter_mut() {
        b.idle = if b.users == 0 { b.idle + dt } else { 0.0 };
        if b.idle > EVICT_AFTER {
            gone.push(*m);
        }
    }
    for m in gone {
        if let Some(b) = city.built.remove(&m) {
            for (h, _, _) in b.parts {
                meshes.remove(&h);
            }
        }
    }
    // A cell's stand-in hides once every instance of the cell is drawn as wanted, and shows again when it is not wanted.
    for (c, (entities, hidden)) in city.fakes.iter_mut() {
        let cell = &city.store.cells[*c];
        let wanted = cell.instances.iter().any(|&i| city.want[i].is_some());
        let settled = cell.instances.iter().all(|&i| city.shown[i].as_ref().map(|s| s.0) == city.want[i]);
        let hide = wanted && settled;
        if hide != *hidden {
            *hidden = hide;
            for &e in entities.iter() {
                commands.entity(e).insert(if hide { Visibility::Hidden } else { Visibility::Inherited });
            }
        }
    }
    *log += dt;
    if *log > 2.0 && std::env::var("AC1_FPS").is_ok() {
        *log = 0.0;
        info!(
            "city stream ({}): {} instances drawn, {} vertices; {} meshes on the GPU ({} vertices), {} building; {} stand-in cells hidden",
            city.detail.name,
            city.drawn.0,
            city.drawn.1,
            city.built.len(),
            city.built.values().map(|b| b.verts).sum::<usize>(),
            city.pending.len(),
            city.fakes.values().filter(|f| f.1).count()
        );
    }
}

/// How much a city draws (`AC1_DETAIL=low|medium|high|ultra|full`, else the graphics menu's, default medium): how far round the camera its cells are
/// drawn, how soon its meshes go to coarser levels, the vertex budget, and how big its textures are.
#[derive(Clone, Copy, Debug)]
pub struct Detail {
    pub name: &'static str,
    /// Half the side of the box round the camera whose cells are drawn (m); past it the stand-ins.
    pub view_box: f32,
    /// LOD distances are multiplied by this (more: coarser levels sooner).
    pub lod_scale: f32,
    /// The most vertices the drawn meshes add up to; past it the farther ones draw at their coarsest level.
    pub vertex_budget: usize,
    /// The largest texture side kept (larger levels of the mip chain are dropped).
    pub max_texture: u32,
    /// Everything at full detail, no stand-ins (the city as drawn before streaming, to compare).
    pub full: bool,
}

impl Detail {
    pub const LOW: Self = Self { name: "low", view_box: 64.0, lod_scale: 1.5, vertex_budget: 1_500_000, max_texture: 256, full: false };
    pub const MEDIUM: Self = Self { name: "medium", view_box: 128.0, lod_scale: 1.0, vertex_budget: 4_000_000, max_texture: 512, full: false };
    pub const HIGH: Self = Self { name: "high", view_box: 256.0, lod_scale: 0.75, vertex_budget: 12_000_000, max_texture: 1024, full: false };
    pub const ULTRA: Self = Self { name: "ultra", view_box: 384.0, lod_scale: 0.5, vertex_budget: 20_000_000, max_texture: 2048, full: false };
    pub const FULL: Self = Self { name: "full", view_box: f32::INFINITY, lod_scale: 0.0, vertex_budget: usize::MAX, max_texture: 512, full: true };

    /// The detail to load a city with: `AC1_DETAIL`, else the player's settings (`settings`, the graphics menu).
    pub fn from_env() -> Self {
        crate::settings::started().city_detail()
    }

    pub fn named(name: &str) -> Option<Self> {
        Some(match name.trim().to_lowercase().as_str() {
            "low" => Self::LOW,
            "medium" => Self::MEDIUM,
            "high" => Self::HIGH,
            "ultra" => Self::ULTRA,
            "full" => Self::FULL,
            _ => return None,
        })
    }
}
