//! Ragdolls: AC1's human ragdoll (`RagdollNew` `human_ragdoll`, a Havok packfile: 19 capsule bodies with
//! masses, 18 joints with limits, see `forge::hkx`) mapped onto a rig by bone name, simulated by
//! `ik::ragdoll` (particles at the bones, AC1's capsule radii and masses, its knee and elbow ranges).
//! A body whose death clip has ended goes limp: it settles on what is under it, or falls off an edge.

use anyhow::{Context, Result};
use bevy::prelude::*;
use forge::hkx::{BodyShape, JointLimit, Packfile, ragdoll};
use ik::Rig;
use ik::ragdoll::{Bend, BoneDef, RagdollDef};
use std::path::Path;

const CLASS_RAGDOLL: u32 = 0xc8dbde7f;

/// The game's human ragdoll.
#[derive(Resource, Clone)]
pub struct HumanRagdoll(pub forge::hkx::Ragdoll);

/// Read `human_ragdoll` from `DataPC.forge` (its `Rank 9` data file holds it).
pub fn load(game_dir: &Path) -> Result<HumanRagdoll> {
    let f = forge::Forge::open(game_dir.join("DataPC.forge"))?;
    for e in f.entries.iter().filter(|e| e.name == "Rank 9") {
        let data = f.read(e)?;
        if let Some(o) = forge::parse_objects(&data)?.iter().find(|o| o.class == CLASS_RAGDOLL && o.name == "human_ragdoll") {
            let r = ragdoll(&Packfile::find(o.body)?)?;
            info!("ragdoll {}: {} bodies, {} joints", o.name, r.bodies.len(), r.joints.len());
            return Ok(HumanRagdoll(r));
        }
    }
    None.context("no human_ragdoll in DataPC.forge")
}

/// The ragdoll for `rig`: AC1's bodies on the bones of the same names, sorted parents first.
pub fn def_for(h: &HumanRagdoll, rig: &Rig) -> RagdollDef {
    let r = &h.0;
    let bone = |name: &str| rig.find(name);
    let body_bone = |i: usize| bone(r.bodies[i].name.trim_start_matches("Ragdoll_"));
    let rest = rig.rest_pose().model(rig);
    let mut bones: Vec<BoneDef> = vec![];
    for (i, b) in r.bodies.iter().enumerate() {
        let Some(rb) = body_bone(i) else { continue };
        let (radius, far) = match b.shape {
            BodyShape::Capsule { a, b, radius } => (radius, Vec3::from(a).length().max(Vec3::from(b).length()) + radius),
            BodyShape::Box { half, radius } => (half[0].min(half[1]).min(half[2]) + radius, Vec3::from(half).length()),
        };
        let name = rig.names[rb].as_deref().unwrap_or("");
        // Tips: the far end of hands, feet and the head, along the bone toward its end.
        let child_dir = |child: &str| bone(child).map(|c| rig.rest[c].pos.normalize_or_zero());
        let tip_dir = match name {
            "LeftHand" => child_dir("LeftHandMiddle1"),
            "RightHand" => child_dir("RightHandMiddle1"),
            "LeftFoot" | "RightFoot" => rig.parents.iter().position(|p| *p == Some(rb)).map(|c| rig.rest[c].pos.normalize_or_zero()),
            // The head's top: model up, in the head's frame.
            "Head" => Some(rest[rb].rot.inverse() * Vec3::Z),
            _ => None,
        };
        let tip = tip_dir.filter(|d| d.length() > 0.5).map(|d| d * far);
        bones.push(BoneDef { bone: rb, radius, mass: b.mass.max(1.0), tip });
    }
    bones.sort_by_key(|b| b.bone);
    let ids = |names: &[&str]| names.iter().filter_map(|n| bone(n)).collect::<Vec<_>>();
    let rigid = vec![ids(&["Hips", "LeftUpLeg", "RightUpLeg", "Spine"]), ids(&["Spine1", "Spine2", "LeftClavicle", "RightClavicle", "LeftArm", "RightArm"])];
    let mut bends = vec![];
    for j in &r.joints {
        let (Some(a), Some(b)) = (body_bone(j.a), body_bone(j.b)) else { continue };
        // (Havok joints link a child body (a) to its parent (b).)
        let (child, parent) = (a, b);
        let Some(grand) = rig.parents[parent].and_then(|_| {
            let mut p = rig.parents[parent];
            while let Some(q) = p {
                if bones.iter().any(|d| d.bone == q) {
                    return Some(q);
                }
                p = rig.parents[q];
            }
            None
        }) else {
            continue;
        };
        let cname = rig.names[child].as_deref().unwrap_or("");
        match j.limit {
            // Knees and elbows: AC1's hinge range (elbows a little wider, as they read too stiff).
            JointLimit::Hinge { max, .. } if (cname.ends_with("Leg") && !cname.ends_with("UpLeg")) || cname.ends_with("ForeArm") => {
                let max = if cname.ends_with("ForeArm") { max * 1.6 } else { max };
                bends.push(Bend { a: parent, b: child, c: bones_child(rig, &bones, child), min: 0.0, max, relative: false });
            }
            // Spine and neck: within their cone of the starting curve.
            JointLimit::Ragdoll { cone, .. } if cname == "Head" || cname.starts_with("Spine") => {
                let r = cone.max(0.2) * 1.5;
                bends.push(Bend { a: grand, b: parent, c: child, min: -r, max: r, relative: true });
            }
            _ => {}
        }
    }
    RagdollDef { bones, rigid, bends }
}

/// The first driven bone below `bone` (its child in the ragdoll), else `bone` itself.
fn bones_child(rig: &Rig, bones: &[BoneDef], bone: usize) -> usize {
    bones
        .iter()
        .map(|b| b.bone)
        .find(|&c| {
            let mut p = rig.parents[c];
            while let Some(q) = p {
                if q == bone {
                    return true;
                }
                if bones.iter().any(|b| b.bone == q) {
                    return false;
                }
                p = rig.parents[q];
            }
            false
        })
        .unwrap_or(bone)
}
