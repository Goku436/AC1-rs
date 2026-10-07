//! Altaïr's robe tails as cloth (AC1 runs `UCMA_Altair_Cloth` as a soft body, a `ClothComponent` colliding
//! with capsules on the legs, `humanragdoll_forcloth`): the skinned mesh is skinned on the CPU for where the
//! animation puts each vertex, `ik::cloth` lets the lower vertices swing (pinned at the waist, freer toward
//! the hem) and keeps them out of capsules round the hips and legs, and a world-space copy of the mesh is
//! drawn instead of the skinned one. `AC1_NO_CLOTH` keeps the skinned mesh.

use crate::character::Character;
use bevy::mesh::{Indices, VertexAttributeValues};
use bevy::prelude::*;
use ik::cloth::Cloth;

/// The robe's mesh name.
pub const ROBE: &str = "UCMA_Altair_Cloth";
/// Vertices this close below the robe's top (m) are pinned to the body.
const PINNED: f32 = 0.06;
/// How far a vertex may stray from its animated position per metre below the pins, and at most (m).
const REACH_PER_M: f32 = 0.55;
const MAX_REACH: f32 = 0.3;
const DAMPING: f32 = 0.04;
/// Pull toward the animated drape (1/s): standing, the robe hangs as authored (over the thighs, not sagging off them
/// to show the trousers under it); moving, it still swings out.
const SHAPE_RATE: f32 = 6.0;

pub struct Robe {
    rest: Vec<Vec3>,
    slots: Vec<[u16; 4]>,
    weights: Vec<[f32; 4]>,
    inverse_binds: Vec<Mat4>,
    joints: Vec<Entity>,
    triangles: Vec<[usize; 3]>,
    reach: Vec<f32>,
    mesh: Handle<Mesh>,
    cloth: Option<Cloth>,
}

impl Robe {
    /// From the robe part's skinned mesh (mesh space, Z up), its inverse binds and joint entities; `shown` is
    /// the world-space copy to write into.
    pub fn new(mesh: &Mesh, inverse_binds: Vec<Mat4>, joints: Vec<Entity>, shown: Handle<Mesh>) -> Option<Self> {
        let Some(VertexAttributeValues::Float32x3(pos)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) else { return None };
        let Some(VertexAttributeValues::Uint16x4(slots)) = mesh.attribute(Mesh::ATTRIBUTE_JOINT_INDEX) else { return None };
        let Some(VertexAttributeValues::Float32x4(weights)) = mesh.attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT) else { return None };
        let triangles: Vec<[usize; 3]> = match mesh.indices()? {
            Indices::U16(i) => i.chunks_exact(3).map(|t| [t[0] as usize, t[1] as usize, t[2] as usize]).collect(),
            Indices::U32(i) => i.chunks_exact(3).map(|t| [t[0] as usize, t[1] as usize, t[2] as usize]).collect(),
        };
        let rest: Vec<Vec3> = pos.iter().map(|p| Vec3::from(*p)).collect();
        let top = rest.iter().map(|p| p.z).fold(f32::MIN, f32::max);
        let reach = rest
            .iter()
            .map(|p| {
                let below = top - p.z - PINNED;
                if below <= 0.0 { 0.0 } else { (below * REACH_PER_M).min(MAX_REACH) }
            })
            .collect();
        info!("robe cloth: {} vertices, {} triangles", rest.len(), triangles.len());
        Some(Robe { rest, slots: slots.clone(), weights: weights.clone(), inverse_binds, joints, triangles, reach, mesh: shown, cloth: None })
    }
}

/// Step every robe and write its vertices (after the bones have their world transforms).
pub fn robe_cloth(
    time: Res<Time>,
    play: Res<crate::character::StatuePlay>,
    frame: Res<bevy::diagnostic::FrameCount>,
    cams: Query<&GlobalTransform, With<Camera3d>>,
    mut chars: Query<(Entity, &mut Character, Has<crate::character::Statue>, Has<crate::net::Remote>)>,
    globals: Query<&GlobalTransform>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let dt = time.delta_secs().clamp(1e-4, 0.05);
    let cam = cams.iter().next().map(|g| g.translation());
    for (e, mut ch, statue, remote) in &mut chars {
        // (The network test's friend moves: their robe always swings.)
        if statue
            && !remote
            && globals.get(e).is_ok_and(|g| crate::character::statue_still(&play, frame.0, cam, g.translation(), crate::character::STATUE_CLOTH_NEAR))
        {
            continue;
        }
        let ch = &mut *ch;
        let capsules = ch.leg_capsules(&globals);
        let Some(robe) = ch.robe.as_mut() else { continue };
        let skin: Vec<Mat4> =
            robe.joints.iter().zip(&robe.inverse_binds).map(|(j, ib)| globals.get(*j).map_or(Mat4::IDENTITY, |g| g.to_matrix()) * *ib).collect();
        let targets: Vec<Vec3> = robe
            .rest
            .iter()
            .zip(robe.slots.iter().zip(&robe.weights))
            .map(|(p, (s, w))| (0..4).filter(|&k| w[k] > 0.0).map(|k| skin.get(s[k] as usize).map_or(Vec3::ZERO, |m| m.transform_point3(*p)) * w[k]).sum())
            .collect();
        let cloth = robe.cloth.get_or_insert_with(|| {
            let mut c = Cloth::new(&robe.rest, &targets, &robe.triangles, robe.reach.clone());
            c.shape_rate = SHAPE_RATE;
            c
        });
        cloth.step(&targets, dt, Vec3::NEG_Y * 9.8, DAMPING, &capsules);
        let Some(mut mesh) = meshes.get_mut(&robe.mesh) else { continue };
        let pos: Vec<[f32; 3]> = cloth.pos.iter().map(|p| p.to_array()).collect();
        // Smooth normals from the triangles.
        let mut nrm = vec![Vec3::ZERO; pos.len()];
        for t in &robe.triangles {
            let (a, b, c) = (cloth.pos[t[0]], cloth.pos[t[1]], cloth.pos[t[2]]);
            let n = (b - a).cross(c - a);
            for &i in t {
                nrm[i] += n;
            }
        }
        let nrm: Vec<[f32; 3]> = nrm.iter().map(|n| n.normalize_or(Vec3::Y).to_array()).collect();
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, nrm);
        let _ = mesh.generate_tangents();
    }
}
