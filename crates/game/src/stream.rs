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
//! coarsest level. Meshes are built for the GPU when first drawn, at most `BUILD_BUDGET` vertices a frame.
//!
//! The streaming follows Banned445's AC1-Movement-Rewritten (MIT; `src/city/stream.rs`, `fakes.rs`): grid cells round the
//! player, LOD by distance with a vertex budget, the fake meshes on the horizon. Written afresh here over ac1-rs' own
//! city loader.

use bevy::asset::RenderAssetUsages;
use bevy::camera::primitives::Aabb;
use bevy::camera::visibility::NoAutoAabb;
use bevy::light::NotShadowCaster;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;

/// How often the drawn set is planned (s).
const PLAN_EVERY: f32 = 0.25;
/// Half the side of the box round the camera whose cells are drawn (m).
const VIEW_BOX: f32 = 128.0;
/// The most vertices the drawn instances add up to.
const VERTEX_BUDGET: usize = 6_000_000;
/// The most vertices of meshes built for the GPU in a frame (spreads the cost of coming into a district).
const BUILD_BUDGET: usize = 400_000;

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

/// A mesh's parts for the GPU (as `mesh_parts`), each with its box; render-world only (no copy kept here).
pub fn build_parts(m: &StoredMesh) -> Vec<(Mesh, Aabb)> {
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
        out.push((bm, Aabb::from_min_max(lo, hi)));
    }
    out
}

/// A mesh on the GPU: its parts.
struct Built {
    parts: Vec<(Handle<Mesh>, Aabb)>,
}

/// The city as drawn now.
#[derive(Resource)]
pub struct CityRender {
    store: RenderStore,
    built: HashMap<usize, Built>,
    /// Per instance: the level drawn and its entities.
    shown: Vec<Option<(usize, Vec<Entity>)>>,
    /// Per instance: the level wanted (None: not drawn).
    want: Vec<Option<usize>>,
    /// Per cell: the stand-in pieces that hide while its own objects are drawn, and whether they are hidden.
    fakes: HashMap<usize, (Vec<Entity>, bool)>,
    timer: f32,
    /// Counters for the log: instances drawn, their vertices.
    drawn: (usize, usize),
}

impl CityRender {
    /// Spawn the stand-ins (shown) and keep the store.
    pub fn new(store: RenderStore, materials: Vec<Handle<StandardMaterial>>, commands: &mut Commands, meshes: &mut Assets<Mesh>) -> Self {
        let mut fakes: HashMap<usize, (Vec<Entity>, bool)> = HashMap::new();
        let mut store = store;
        store.materials = materials;
        for f in std::mem::take(&mut store.fakes) {
            let Some(mat) = store.materials.get(f.material as usize).cloned() else { continue };
            let e = commands.spawn((Mesh3d(meshes.add(f.mesh)), MeshMaterial3d(mat), Transform::from_matrix(f.world), NotShadowCaster)).id();
            if let Some(c) = f.cell {
                fakes.entry(c).or_default().0.push(e);
            }
        }
        let n = store.instances.len();
        Self { store, built: HashMap::new(), shown: vec![None; n], want: vec![None; n], fakes, timer: 0.0, drawn: (0, 0) }
    }

    /// Which level each instance wants with the camera at `eye`: see the module docs.
    fn plan(&mut self, eye: Vec3) {
        let p = Vec2::new(eye.x, -eye.z);
        let (lo, hi) = (p - Vec2::splat(VIEW_BOX), p + Vec2::splat(VIEW_BOX));
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
        for (d, i) in near {
            let inst = &self.store.instances[i];
            let mut level = pick_level(&inst.levels, d);
            // (Out of budget: its coarsest level that draws, or the one picked if that is coarser.)
            if spent > VERTEX_BUDGET
                && let Some(c) = inst.levels.iter().rposition(|l| l.mesh.is_some())
            {
                level = Some(c.max(level.unwrap_or(0)));
            }
            let verts = level.and_then(|l| inst.levels[l].mesh).map_or(0, |m| self.store.meshes[m].verts);
            spent += verts;
            drawn += usize::from(verts > 0);
            self.want[i] = level;
        }
        self.drawn = (drawn, spent);
    }
}

/// The level drawn at LOD distance `d`: the first whose `until` reaches it, past the last the last (`forge::lod::pick`).
fn pick_level(levels: &[Lod], d: f32) -> Option<usize> {
    levels.iter().position(|l| d <= l.until).or(levels.len().checked_sub(1))
}

/// Per frame: plan (every `PLAN_EVERY`), then bring the drawn instances to their wanted level (building meshes within
/// `BUILD_BUDGET`), then hide or show the stand-ins.
pub fn stream_city(
    mut commands: Commands,
    time: Res<Time>,
    city: Option<ResMut<CityRender>>,
    cams: Query<&Transform, With<Camera3d>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut log: Local<f32>,
) {
    let Some(mut city) = city else { return };
    let Ok(cam) = cams.single() else { return };
    let city = &mut *city;
    city.timer -= time.delta_secs();
    if city.timer <= 0.0 {
        city.timer = PLAN_EVERY;
        city.plan(cam.translation);
    }
    let mut budget = BUILD_BUDGET;
    for i in 0..city.want.len() {
        let want = city.want[i];
        if city.shown[i].as_ref().map(|s| s.0) == want {
            continue;
        }
        let inst = &city.store.instances[i];
        let mesh = want.and_then(|l| inst.levels[l].mesh);
        if let Some(m) = mesh
            && !city.built.contains_key(&m)
        {
            let verts = city.store.meshes[m].verts;
            if verts > budget && budget < BUILD_BUDGET {
                continue;
            }
            budget = budget.saturating_sub(verts);
            let parts = build_parts(&city.store.meshes[m]).into_iter().map(|(mesh, aabb)| (meshes.add(mesh), aabb)).collect();
            city.built.insert(m, Built { parts });
        }
        if let Some((_, old)) = city.shown[i].take() {
            for e in old {
                commands.entity(e).despawn();
            }
        }
        let Some(l) = want else { continue };
        let tf = Transform::from_matrix(inst.world);
        let mut entities = vec![];
        if let Some(b) = mesh.and_then(|m| city.built.get(&m)) {
            for (k, (h, aabb)) in b.parts.iter().enumerate() {
                let Some(mat) = inst.parts[l].get(k).and_then(|&s| city.store.materials.get(s as usize)) else { continue };
                entities.push(commands.spawn((Mesh3d(h.clone()), MeshMaterial3d(mat.clone()), tf, *aabb, NoAutoAabb)).id());
            }
        }
        city.shown[i] = Some((l, entities));
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
    *log += time.delta_secs();
    if *log > 2.0 && std::env::var("AC1_FPS").is_ok() {
        *log = 0.0;
        info!(
            "city stream: {} instances drawn, {} vertices, {} meshes built, {} stand-in cells hidden",
            city.drawn.0,
            city.drawn.1,
            city.built.len(),
            city.fakes.values().filter(|f| f.1).count()
        );
    }
}
