//! action_keys <game dir>: the actions of `Game Fix`'s blocks the game's code requests by key (key != id: small
//! numbers), with their block and first clips.
use anyhow::{Context, Result};
use std::collections::{BTreeMap, HashMap};

fn main() -> Result<()> {
    let dir = std::env::args().nth(1).context("game dir")?;
    let f = forge::Forge::open(format!("{dir}/DataPC.forge"))?;
    let e = f.entries.iter().find(|e| e.name == "Game Fix").context("no Game Fix")?;
    let data = f.read(e)?;
    let objs = forge::parse_objects(&data)?;
    let names: HashMap<u32, &str> = objs.iter().map(|o| (o.id, o.name.as_str())).collect();
    let mut keys: BTreeMap<u32, Vec<String>> = BTreeMap::new();
    for o in objs.iter().filter(|o| o.class == forge::action::CLASS_ACTION_BLOCK) {
        let Ok(b) = forge::action::parse_block(o.body) else { continue };
        for a in b.actions.iter().filter_map(forge::action::Slot::inline) {
            if a.key == a.id {
                continue;
            }
            let clips: Vec<&str> = a.items.first().map(|i| i.clips.iter().filter_map(|c| names.get(c).copied()).take(2).collect()).unwrap_or_default();
            keys.entry(a.key).or_default().push(format!("{}: {}", o.name, clips.join(" | ")));
        }
    }
    for (k, v) in keys {
        println!("{k:#06x} ({k}): {}", v.join("  ;  "));
    }
    Ok(())
}
