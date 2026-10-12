//! Altaïr's robe tails as AC1 simulates them: the cloth mesh `UCMA_Altair_Cloth` as a soft body (`ik::cloth`) on
//! AC1's own settings (`forge::cloth`: the entity's `Cloth` and `ClothComponent`, the soft mesh's pins, pulls and
//! collision radii, the `humanragdoll_forcloth` capsules on the thighs, shins and feet, the `ClothActionSettings`),
//! blown by the city's wind (`wind`).
//!
//! Each frame the soft mesh's own vertices are skinned on the CPU for where the animation puts them. The cloth steps
//! AC1's 0.033333 s at a time on game time (AC1 steps it once a frame, at its 30 fps); between steps the robe is
//! drawn on the body with its offsets from the animated pose eased from the last step's to this one's, so it moves
//! smoothly at any frame rate and its waist never leaves the body. The character's motion over each step (speed,
//! turn, rise) gives the motion pull and the upward damping; the action playing (the climb's clip, through the move
//! graph) gives the pull's strength, 1 for actions the settings do not list. A world-space copy of the mesh is drawn
//! instead of the skinned one. `AC1_NO_CLOTH` keeps the skinned mesh.

use crate::character::Character;
use bevy::mesh::{Indices, VertexAttributeValues};
use bevy::prelude::*;
use ik::cloth::{Cloth, Collider, Inputs, Motion, Params, STEP};

pub struct Robe {
    /// The soft mesh's vertices (mesh space), skin bones (mesh-bone slots) and weights, in the drawn mesh's order.
    rest: Vec<Vec3>,
    slots: Vec<[u16; 4]>,
    weights: Vec<[f32; 4]>,
    inverse_binds: Vec<Mat4>,
    joints: Vec<Entity>,
    mesh_to_model: Mat4,
    /// The character's model root (its placement: the rest pose carried rigidly, the motion).
    model: Entity,
    /// The drawn mesh's triangles (for its normals).
    triangles: Vec<[usize; 3]>,
    params: Params,
    /// (bone, capsule in the bone's frame).
    colliders: Vec<(Entity, Collider)>,
    /// (action key, strength).
    actions: Vec<(u32, f32)>,
    wind_scale: f32,
    mesh: Handle<Mesh>,
    state: Option<State>,
}

struct State {
    cloth: Cloth,
    /// Game time not yet stepped.
    acc: f32,
    /// The cloth's offsets from the animated pose after the step before last and after the last.
    offsets: [Vec<Vec3>; 2],
    anchor: Vec3,
    facing: Vec3,
}

impl Robe {
    /// From the robe part's skinned mesh (mesh space, Z up), its inverse binds and joint entities, AC1's cloth data
    /// (its capsules' bones as joint entities), the model root and how mesh space sits in it; `shown` is the
    /// world-space copy to write into.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        mesh: &Mesh,
        inverse_binds: Vec<Mat4>,
        joints: Vec<Entity>,
        cloth: &crate::assets::ClothData,
        colliders: Vec<(Entity, Collider)>,
        model: Entity,
        mesh_to_model: Mat4,
        shown: Handle<Mesh>,
    ) -> Option<Self> {
        let Some(VertexAttributeValues::Float32x3(pos)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) else { return None };
        let triangles: Vec<[usize; 3]> = match mesh.indices()? {
            Indices::U16(i) => i.chunks_exact(3).map(|t| [t[0] as usize, t[1] as usize, t[2] as usize]).collect(),
            Indices::U32(i) => i.chunks_exact(3).map(|t| [t[0] as usize, t[1] as usize, t[2] as usize]).collect(),
        };
        let soft = &cloth.soft;
        let order = match soft.order_in(pos) {
            Ok(o) => o,
            Err(e) => {
                warn!("robe: the soft mesh does not match the drawn one ({e:#}); drawn skinned");
                return None;
            }
        };
        let s = &cloth.settings;
        let n = order.len();
        let mut rest = vec![Vec3::ZERO; n];
        let (mut slots, mut weights) = (vec![[0u16; 4]; n], vec![[0.0f32; 4]; n]);
        let (mut pinned, mut pull, mut radius) = (vec![false; n], vec![0.0; n], vec![0.0; n]);
        for (i, &j) in order.iter().enumerate() {
            rest[j] = Vec3::from_array(soft.positions[i]);
            slots[j] = soft.bones[i].map(|b| if b == 255 { 0 } else { b as u16 });
            weights[j] = soft.weights[i];
            let [_, r, p, _] = soft.flags[i].map(|f| f as f32 / 255.0);
            pinned[j] = soft.flags[i][2] == 255;
            pull[j] = s.pull[0] + (s.pull[1] - s.pull[0]) * p;
            radius[j] = s.radius[0] + (s.radius[1] - s.radius[0]) * r;
        }
        let edges: Vec<[usize; 2]> = soft.edges().iter().map(|&[a, b]| [order[a], order[b]]).collect();
        let rest_len2 = edges.iter().map(|&[a, b]| rest[a].distance_squared(rest[b])).collect();
        let params = Params {
            pinned,
            pull,
            radius,
            edges,
            rest_len2,
            triangles: soft.triangles.iter().map(|t| t.map(|v| order[v as usize])).collect(),
            damping: s.damping,
            upward_damping: s.upward_damping,
            gravity: s.gravity,
            iterations: s.iterations,
            pull_motion: s.pull_motion,
            pull_decay: s.pull_decay,
            upward_motion: s.upward_motion,
            force: s.force.map(|f| (f.tangent, f.random)),
        };
        info!(
            "robe cloth: {n} vertices ({} pinned), {} edges, {} capsules, AC1's settings",
            params.pinned.iter().filter(|p| **p).count(),
            params.edges.len(),
            colliders.len()
        );
        Some(Robe {
            rest,
            slots,
            weights,
            inverse_binds,
            joints,
            mesh_to_model,
            model,
            triangles,
            params,
            colliders,
            actions: cloth.actions.clone(),
            wind_scale: s.force.map_or(0.0, |f| f.scale),
            mesh: shown,
            state: None,
        })
    }

    /// How much of the motion pull the clip `clip` lets off: its action's strength in the settings, else 1.
    fn strength(&self, graph: Option<&forge::graph::MoveGraph>, clip: Option<&str>) -> f32 {
        let (Some(g), Some(c)) = (graph, clip) else { return 1.0 };
        g.places(c).iter().find_map(|p| self.actions.iter().find(|a| a.0 == p.action)).map_or(1.0, |a| a.1)
    }
}

/// A capsule of AC1's cloth ragdoll, in its bone's frame.
pub fn collider(c: &forge::cloth::Collider) -> Collider {
    Collider { a: Vec3::from_array(c.a), b: Vec3::from_array(c.b), radius: c.radius, mode: c.mode, threshold: c.threshold }
}

/// Step every robe and write its vertices (after the bones have their world transforms).
#[allow(clippy::too_many_arguments)]
pub fn robe_cloth(
    time: Res<Time>,
    play: Res<crate::character::StatuePlay>,
    frame: Res<bevy::diagnostic::FrameCount>,
    wind: Option<Res<crate::wind::WindField>>,
    lib: Option<Res<crate::animation::AnimLib>>,
    cams: Query<&GlobalTransform, With<Camera3d>>,
    mut chars: Query<(Entity, &mut Character, Has<crate::character::Statue>, Has<crate::net::Remote>)>,
    globals: Query<&GlobalTransform>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let dt = time.delta_secs().clamp(0.0, 0.1);
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
        let clip = ch.wall.as_ref().and_then(|w| w.clip_name()).map(str::to_owned);
        let Some(robe) = ch.robe.as_mut() else { continue };
        let Ok(model) = globals.get(robe.model).map(|g| g.to_matrix()) else { continue };
        let skin: Vec<Mat4> =
            robe.joints.iter().zip(&robe.inverse_binds).map(|(j, ib)| globals.get(*j).map_or(Mat4::IDENTITY, |g| g.to_matrix()) * *ib).collect();
        let targets: Vec<Vec3> = robe
            .rest
            .iter()
            .zip(robe.slots.iter().zip(&robe.weights))
            .map(|(p, (s, w))| (0..4).filter(|&k| w[k] > 0.0).map(|k| skin.get(s[k] as usize).map_or(Vec3::ZERO, |m| m.transform_point3(*p)) * w[k]).sum())
            .collect();
        let placed = model * robe.mesh_to_model;
        let anchor = model.w_axis.truncate();
        // The model's forward (its X) in the world.
        let facing = model.x_axis.truncate().normalize_or_zero();
        let restart = robe.state.as_ref().is_none_or(|s| s.anchor.distance_squared(anchor) > 4.0 || s.cloth.pos.len() != targets.len());
        if restart {
            let zero = vec![Vec3::ZERO; targets.len()];
            robe.state = Some(State { cloth: Cloth::new(&robe.params, &targets), acc: 0.0, offsets: [zero.clone(), zero], anchor, facing });
        }
        let strength = robe.strength(lib.as_ref().map(|l| &l.graph), clip.as_deref());
        let wind_at = wind.as_ref().map_or(Vec3::ZERO, |w| w.at(anchor) * robe.wind_scale);
        let Some(state) = robe.state.as_mut() else { continue };
        state.acc += dt;
        let steps = (state.acc / STEP).floor().min(4.0);
        if steps >= 1.0 {
            let span = steps * STEP;
            let moved = anchor - state.anchor;
            let motion = Motion { speed: moved.length() / span, turn: (facing - state.facing).length() / span, rise: moved.y / span };
            let colliders: Vec<Collider> = robe
                .colliders
                .iter()
                .filter_map(|(j, c)| {
                    let g = globals.get(*j).ok()?;
                    Some(Collider { a: g.transform_point(c.a), b: g.transform_point(c.b), radius: c.radius * g.scale().max_element(), ..*c })
                })
                .collect();
            let rigid: Vec<Vec3> = robe.rest.iter().map(|p| placed.transform_point3(*p)).collect();
            let inputs = Inputs { targets: &targets, rigid: &rigid, colliders: &colliders, motion, strength, wind: wind_at, up: Vec3::Y };
            for _ in 0..steps as usize {
                state.cloth.step(&robe.params, &inputs, STEP);
            }
            state.offsets[0] = std::mem::take(&mut state.offsets[1]);
            state.offsets[1] = state.cloth.pos.iter().zip(&targets).map(|(p, t)| *p - *t).collect();
            state.acc = (state.acc - span).max(0.0);
            state.anchor = anchor;
            state.facing = facing;
        }
        let ease = (state.acc / STEP).clamp(0.0, 1.0);
        let shown: Vec<Vec3> = targets.iter().enumerate().map(|(i, t)| *t + state.offsets[0][i].lerp(state.offsets[1][i], ease)).collect();
        let Some(mut mesh) = meshes.get_mut(&robe.mesh) else { continue };
        // Smooth normals from the triangles.
        let mut nrm = vec![Vec3::ZERO; shown.len()];
        for t in &robe.triangles {
            if t.iter().any(|&i| i >= shown.len()) {
                continue;
            }
            let (a, b, c) = (shown[t[0]], shown[t[1]], shown[t[2]]);
            let n = (b - a).cross(c - a);
            for &i in t {
                nrm[i] += n;
            }
        }
        let nrm: Vec<[f32; 3]> = nrm.iter().map(|n| n.normalize_or(Vec3::Y).to_array()).collect();
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, shown.iter().map(|p| p.to_array()).collect::<Vec<_>>());
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, nrm);
        let _ = mesh.generate_tangents();
    }
}
