//! action_clips <game dir> <action id>...: the clips each action (by the id the game's code asks for) plays, item by item.
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
    let blocks: Vec<(&str, forge::action::ActionBlock)> = objs
        .iter()
        .filter(|o| o.class == forge::action::CLASS_ACTION_BLOCK)
        .filter_map(|o| forge::action::parse_block(o.body).ok().map(|b| (o.name.as_str(), b)))
        .collect();
    let g = forge::graph::MoveGraph::new(blocks.iter().map(|(n, b)| (*n, b)), |id| names.get(&id).cloned());
    for a in &args[2..] {
        let id = u32::from_str_radix(a.trim_start_matches("0x"), 16).context("hex id")?;
        match g.action_clips(id) {
            Some(items) => println!("{id:#010x}: {:?}", items.iter().map(|i| i.join("|")).collect::<Vec<_>>()),
            None => println!("{id:#010x}: -"),
        }
    }
    Ok(())
}
