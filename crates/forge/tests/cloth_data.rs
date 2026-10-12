//! Altaïr's cloth, bone modifiers and the cities' wind, read from the install (skipped when it is not found;
//! `AC1_GAME_DIR` overrides the path).

use forge::cloth;
use forge::modifier::{self, Modifier};
use std::path::PathBuf;

fn game() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var("AC1_GAME_DIR").unwrap_or_else(|_| "P:/SteamLibrary/steamapps/common/Assassins Creed".into()));
    let found = dir.join("DataPC.forge").is_file();
    if !found {
        eprintln!("skipped: no install at {dir:?}");
    }
    found.then_some(dir)
}

fn data_file(game: &std::path::Path, forge_file: &str, name: &str) -> Vec<u8> {
    let f = forge::Forge::open(game.join(forge_file)).unwrap();
    let e = f.entries.iter().find(|e| e.name == name).unwrap();
    f.read(e).unwrap()
}

#[test]
fn altairs_robe_settings() {
    let Some(game) = game() else { return };
    let data = data_file(&game, "DataPC.forge", "Rank 9");
    let objs = forge::parse_objects(&data).unwrap();
    let by_name = |class: u32, name: &str| objs.iter().find(|o| o.class == class && o.name == name).unwrap();
    let entity = by_name(forge::datafile::CLASS_ENTITY, "UCMA_Altair_Rank_9");
    let s = cloth::cloth_settings(entity.body).unwrap();
    assert_eq!((s.damping, s.upward_damping, s.iterations, s.gravity), (0.1, 0.75, 3, -9.8));
    assert_eq!((s.radius, s.pull, s.pull_motion, s.pull_decay, s.upward_motion), ([0.01, 0.07], [0.0, 1.0], [0.03, 0.015], 0.99, [-2.0, 5.0]));
    assert_eq!(s.force, Some(cloth::WindForce { scale: 1.0, tangent: 0.3, random: 1.0 }));
    assert_eq!((s.bones.len(), s.capsules.len()), (21, 19));
    let ragdoll = objs.iter().find(|o| o.id == s.ragdoll).unwrap();
    assert_eq!(ragdoll.name, "humanragdoll_forcloth");
    let colliders = cloth::colliders(&s, ragdoll.body).unwrap();
    assert_eq!(colliders.len(), 6, "six capsules are on");
    let names: Vec<_> = colliders.iter().map(|c| forge::skeleton::bone_name(c.bone).unwrap_or("?")).collect();
    eprintln!("cloth capsules on {names:?}");
    for c in &colliders {
        eprintln!("  {:?} a {:?} b {:?} r {:.3} mode {}", forge::skeleton::bone_name(c.bone), c.a, c.b, c.radius, c.mode);
        assert!(c.radius > 0.05 && c.radius < 0.15);
    }
    let actions = objs.iter().find(|o| o.id == s.action_settings).unwrap();
    let a = cloth::action_settings(actions.body).unwrap();
    assert_eq!(a.len(), 20);

    let mesh = by_name(forge::mesh::CLASS_MESH, "UCMA_Altair_Cloth");
    let soft = cloth::soft_mesh(mesh.body).unwrap();
    let compiled = forge::mesh::parse_mesh(mesh.body).unwrap();
    assert_eq!(soft.positions.len(), 146);
    let order = soft.order_in(&compiled.positions).unwrap();
    // The soft mesh skins to the same bones as the compiled one.
    for (i, &j) in order.iter().enumerate() {
        for k in 0..4 {
            if soft.weights[i][k] > 0.2 {
                assert!(compiled.joints[j].contains(&(soft.bones[i][k] as u16)), "vertex {i}: bone {} not in {:?}", soft.bones[i][k], compiled.joints[j]);
            }
        }
    }
    let pinned = soft.flags.iter().filter(|f| f[2] == 255).count();
    assert_eq!(pinned, 44);
    // The quads' sides, not their diagonals.
    assert_eq!(soft.edges().len(), 253);
}

#[test]
fn altairs_bone_modifiers() {
    let Some(game) = game() else { return };
    let data = data_file(&game, "DataPC.forge", "Rank 9");
    let objs = forge::parse_objects(&data).unwrap();
    let mut seen = vec![];
    for name in ["UCMA_Altair", "UCMA_Altair_Skirt", "Human_Hood", "UCMA_Sword_Tag"] {
        let o = objs.iter().find(|o| o.class == forge::skeleton::CLASS_SKELETON && o.name == name).unwrap();
        let sk = forge::skeleton::parse_skeleton(o.body).unwrap();
        let (m, errors) = modifier::modifiers(o.body, &sk);
        assert!(errors.is_empty(), "{name}: {errors:?}");
        let count = |f: fn(&Modifier) -> bool| m.iter().filter(|m| f(m)).count();
        let counts = [
            count(|m| matches!(m, Modifier::LookAt(_))),
            count(|m| matches!(m, Modifier::Hinge(_))),
            count(|m| matches!(m, Modifier::Compress(_))),
            count(|m| matches!(m, Modifier::RotationPaste(..))),
            count(|m| matches!(m, Modifier::Roll(_))),
            count(|m| matches!(m, Modifier::SpringBox(_))),
        ];
        seen.push((name, counts));
    }
    assert_eq!(
        seen,
        vec![
            ("UCMA_Altair", [0, 14, 10, 0, 6, 1]),
            ("UCMA_Altair_Skirt", [1, 16, 1, 2, 0, 0]),
            ("Human_Hood", [1, 2, 0, 0, 0, 0]),
            ("UCMA_Sword_Tag", [0, 2, 0, 0, 0, 0]),
        ]
    );
    let entity = objs.iter().find(|o| o.class == forge::datafile::CLASS_ENTITY && o.name == "UCMA_Altair_Rank_9").unwrap();
    assert_eq!(modifier::skeleton_wind(entity.body).unwrap().map(|w| w.scale), Some(1.0));
}

#[test]
fn the_cities_wind() {
    let Some(game) = game() else { return };
    for (forge_file, cell, name, dir) in [
        ("DataPC_Damascus.forge", "Cell01364_DataBlock", "Wind_Damascus_001", [0.852, -0.523, 0.0]),
        ("DataPC_Masyaf.forge", "Cell05460_DataBlock", "Wind_Masyaf_001", [-0.884, 0.468, 0.0]),
    ] {
        if !game.join(forge_file).is_file() {
            continue;
        }
        let data = data_file(&game, forge_file, cell);
        let objs = forge::parse_objects(&data).unwrap();
        let o = objs.iter().find(|o| o.name == name).unwrap();
        let w = cloth::wind(o.body).unwrap().unwrap();
        assert_eq!((w.strength, w.noise), (10.0, true));
        assert!((w.percent[0] - 61.3793).abs() < 1e-3 && (w.percent[3] - 11.7241).abs() < 1e-3);
        assert!((0..3).all(|k| (w.direction[k] - dir[k]).abs() < 2e-3), "{name}: {:?}", w.direction);
        // Entities without wind have none.
        let other = objs.iter().find(|o| o.class == forge::datafile::CLASS_ENTITY && o.name != name).unwrap();
        assert!(cloth::wind(other.body).unwrap().is_none());
    }
}
