//! graph_next <game dir> <clip>...: where each clip sits in AC1's move graph, what may follow it and what may lead to it.
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
    for clip in &args[2..] {
        println!(
            "{clip}: {:?} code-driven {}",
            g.places(clip).iter().map(|p| format!("{} {:#x} item {}", p.block, p.action, p.item)).collect::<Vec<_>>(),
            g.code_driven(clip)
        );
        let mut next: Vec<&str> = g.successors(clip).into_iter().collect();
        next.sort();
        for n in next {
            println!("    -> {n}");
        }
        for n in g.predecessors(clip) {
            println!("    <- {n}");
        }
    }
    Ok(())
}
