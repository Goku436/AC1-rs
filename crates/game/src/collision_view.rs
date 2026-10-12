//! F8: the collision round the player, drawn. Every collision face within `RADIUS` of the player as a translucent
//! surface (walls orange, floors and tops green, faces looking down blue), and the faces within `EDGE_RADIUS` outlined,
//! so what stops the body and what it stands on can be told from what is only drawn (cloth, foliage, the cut-outs of
//! the meshes). Rebuilt when the player has moved `REBUILD` from where it was built (or the level changed). `AC1_COLLISION_VIEW=1` starts
//! with it on (screenshots).

use crate::character::Player;
use crate::level::Level;
use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

/// Faces drawn within this of the player (m, on the ground plane, and up to this above and below).
const RADIUS: f32 = 24.0;
/// Faces outlined within this (m).
const EDGE_RADIUS: f32 = 9.0;
/// At most this many faces outlined (the nearest first).
const MAX_EDGES: usize = 12_000;
/// Rebuilt once the player is this far from where it was built (m).
const REBUILD: f32 = 6.0;
/// Faces drawn this far out along their normal (m), off the drawn surface under them.
const LIFT: f32 = 0.015;

#[derive(Resource)]
pub struct CollisionView {
    pub on: bool,
    /// Where the drawn surface was built round, and its entity.
    built: Option<(Vec3, Entity)>,
    material: Option<Handle<StandardMaterial>>,
}

impl Default for CollisionView {
    fn default() -> Self {
        CollisionView { on: std::env::var("AC1_COLLISION_VIEW").is_ok(), built: None, material: None }
    }
}

fn colour(n: Vec3) -> [f32; 4] {
    if n.y > 0.7 {
        [0.25, 0.95, 0.35, 0.30]
    } else if n.y < -0.7 {
        [0.30, 0.45, 1.0, 0.30]
    } else {
        [1.0, 0.45, 0.10, 0.35]
    }
}

/// F8 in and out; the surface rebuilt as the player moves, the near faces outlined every frame.
pub fn collision_view(
    keys: Res<ButtonInput<KeyCode>>,
    mut view: ResMut<CollisionView>,
    level: Res<Level>,
    player: Query<&Transform, With<Player>>,
    mut commands: Commands,
    (mut meshes, mut mats): (ResMut<Assets<Mesh>>, ResMut<Assets<StandardMaterial>>),
    mut gizmos: Gizmos,
) {
    if keys.just_pressed(KeyCode::F8) {
        view.on = !view.on;
        info!("collision view {}", if view.on { "on (F8)" } else { "off" });
    }
    let Ok(tf) = player.single() else { return };
    let at = tf.translation;
    if !view.on {
        if let Some((_, e)) = view.built.take() {
            commands.entity(e).despawn();
        }
        return;
    }
    if level.is_changed() || view.built.is_none_or(|(c, _)| c.distance(at) > REBUILD) {
        if let Some((_, e)) = view.built.take() {
            commands.entity(e).despawn();
        }
        let reach = Vec3::splat(RADIUS);
        let (mut pos, mut nrm, mut col, mut idx) = (vec![], vec![], vec![], vec![]);
        for i in level.tris_near(at - reach, at + reach) {
            let t = &level.tris[i as usize];
            let n = (t.b - t.a).cross(t.c - t.a).normalize_or_zero();
            if n == Vec3::ZERO || [t.a, t.b, t.c].iter().all(|v| (*v - at).with_y(0.0).length() > RADIUS || (v.y - at.y).abs() > RADIUS) {
                continue;
            }
            for v in [t.a, t.b, t.c] {
                idx.push(pos.len() as u32);
                pos.push(v + n * LIFT);
                nrm.push(n);
                col.push(colour(n));
            }
        }
        let faces = idx.len() / 3;
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, nrm);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, col);
        mesh.insert_indices(Indices::U32(idx));
        let material = view
            .material
            .get_or_insert_with(|| {
                mats.add(StandardMaterial {
                    base_color: Color::WHITE,
                    alpha_mode: AlphaMode::Blend,
                    unlit: true,
                    double_sided: true,
                    cull_mode: None,
                    depth_bias: 100.0,
                    ..default()
                })
            })
            .clone();
        let e = commands.spawn((Mesh3d(meshes.add(mesh)), MeshMaterial3d(material), Transform::IDENTITY, Name::new("collision view"))).id();
        view.built = Some((at, e));
        debug!("collision view: {faces} faces within {RADIUS} m");
    }
    // The near faces' edges.
    let reach = Vec3::splat(EDGE_RADIUS);
    let mut near: Vec<(f32, u32)> = level
        .tris_near(at - reach, at + reach)
        .into_iter()
        .map(|i| {
            let t = &level.tris[i as usize];
            (((t.a + t.b + t.c) / 3.0).distance_squared(at), i)
        })
        .filter(|(d, _)| *d < EDGE_RADIUS * EDGE_RADIUS * 4.0)
        .collect();
    if near.len() > MAX_EDGES {
        near.select_nth_unstable_by(MAX_EDGES, |a, b| a.0.total_cmp(&b.0));
        near.truncate(MAX_EDGES);
    }
    let edge = Color::srgba(0.1, 0.05, 0.0, 0.8);
    for (_, i) in near {
        let t = &level.tris[i as usize];
        let n = (t.b - t.a).cross(t.c - t.a).normalize_or_zero() * LIFT * 2.0;
        gizmos.linestrip([t.a + n, t.b + n, t.c + n, t.a + n], edge);
    }
}
