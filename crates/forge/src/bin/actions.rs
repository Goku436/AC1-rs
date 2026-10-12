//! actions <game dir> [block name]: parse every action block in the install (pass rate), or print one
//! block of `DataPC.forge`'s `Game Fix` (the human kit's) readably: its actions, items, clips by name.
use anyhow::{Context, Result};
use forge::action::{self, CLASS_ACTION_BLOCK, Slot};
use std::collections::HashMap;

/// A blend, short: kind (0 cut, 1 AROLLBROLL, 2 ASTOPBROLL, 3 AROLLBSTOP, 4 ASTOPBSTOP), time, flags (clamp, ease in,
/// ease out) and modes (B position, displacement source, actuator) when set.
fn blend(b: &action::Blend) -> String {
    let mut s = format!("k{} {:.2}s", b.kind, b.time());
    if b.flags != [0; 3] {
        s += &format!(" flags {:?}", b.flags);
    }
    if b.modes[..2] != [0, 0] {
        s += &format!(" modes {:?}", b.modes);
    }
    s
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let dir = args.get(1).context("game dir")?;
    if let Some(want) = args.get(2) {
        let f = forge::Forge::open(format!("{dir}/DataPC.forge"))?;
        let e = f.entries.iter().find(|e| e.name == "Game Fix").context("no Game Fix")?;
        let data = f.read(e)?;
        let objs = forge::parse_objects(&data)?;
        let names: HashMap<u32, &str> = objs.iter().map(|o| (o.id, o.name.as_str())).collect();
        let name = |id: &u32| names.get(id).copied().map_or(format!("{id:#x}"), str::to_string);
        for o in objs.iter().filter(|o| o.class == CLASS_ACTION_BLOCK && o.name == *want) {
            let b = action::parse_block(o.body)?;
            println!("{} ({} actions)", o.name, b.actions.len());
            for a in b.actions.iter().filter_map(Slot::inline) {
                println!("action {:#x} key {:#x} flags {:?} associated {:?}", a.id, a.key, a.flags, a.associated);
                for t in a.transitions.iter().flatten().filter_map(Slot::inline) {
                    println!("  -> {:#x} / {:#x}, blend {} / {}", t.action, t.action2, blend(&t.blend), blend(&t.blend2));
                }
                for i in &a.items {
                    println!("  item: blend {} {:?} feet {:?} flags {:?} weights {:?}", blend(&i.blend), i.displacement, i.feet, i.flags, i.weights);
                    for c in &i.clips {
                        println!("    {}", name(c));
                    }
                    for t in i.transitions.iter().filter_map(Slot::inline) {
                        println!("    -> {:#x} / {:#x}, blend {} / {}", t.action, t.action2, blend(&t.blend), blend(&t.blend2));
                    }
                }
            }
        }
        return Ok(());
    }
    let mut paths: Vec<_> = std::fs::read_dir(dir)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "forge") && !p.to_string_lossy().contains("Sound"))
        .collect();
    paths.sort();
    let (mut ok, mut bad, mut actions, mut items) = (0, 0, 0, 0);
    for p in paths {
        let f = forge::Forge::open(&p)?;
        for e in &f.entries {
            let Ok(data) = f.read(e) else { continue };
            let Ok(objs) = forge::parse_objects(&data) else { continue };
            for o in objs.iter().filter(|o| o.class == CLASS_ACTION_BLOCK) {
                match action::parse_block(o.body) {
                    Ok(b) => {
                        ok += 1;
                        actions += b.actions.len();
                        items += b.actions.iter().filter_map(Slot::inline).map(|a| a.items.len()).sum::<usize>();
                    }
                    Err(err) => {
                        bad += 1;
                        eprintln!("{} / {} / {}: {err:#}", p.display(), e.name, o.name);
                    }
                }
            }
        }
    }
    println!("action blocks: {ok} parse, {bad} fail ({actions} actions, {items} items)");
    Ok(())
}
