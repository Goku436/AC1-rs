//! action_index <game dir>: every action of `Game Fix`'s blocks, one per line: id, key, block, its first item's first
//! clip (`id<TAB>key<TAB>block<TAB>clip`). For naming the action ids a hook in the running game logs.
use anyhow::{Context, Result};
use std::collections::HashMap;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let dir = args.get(1).context("game dir")?;
    let f = forge::Forge::open(format!("{dir}/DataPC.forge"))?;
    let e = f.entries.iter().find(|e| e.name == "Game Fix").context("no Game Fix")?;
    let data = f.read(e)?;
    let objs = forge::parse_objects(&data)?;
    let names: HashMap<u32, String> = objs.iter().map(|o| (o.id, o.name.clone())).collect();
    for o in objs.iter().filter(|o| o.class == forge::action::CLASS_ACTION_BLOCK) {
        let Ok(b) = forge::action::parse_block(o.body) else { continue };
        for a in b.actions.iter().filter_map(|s| s.inline()) {
            let clip = a.items.first().and_then(|i| i.clips.first()).map_or("-".to_string(), |c| names.get(c).cloned().unwrap_or(format!("{c:#x}")));
            println!("{:08x}\t{:08x}\t{}\t{clip}", a.id, a.key, o.name);
        }
    }
    Ok(())
}
