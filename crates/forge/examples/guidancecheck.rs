//! guidancecheck <file.forge>: parse every entity's `GuidanceSystem` (authored holds), report the pass rate,
//! failures by reason and edges by subtype.
use std::collections::BTreeMap;
fn main() -> anyhow::Result<()> {
    let f = forge::Forge::open(std::env::args().nth(1).unwrap())?;
    let (mut ok, mut fails, mut subs, mut enabled) = (0, BTreeMap::<String, (usize, String)>::new(), BTreeMap::new(), 0usize);
    for e in &f.entries {
        let Ok(d) = f.read(e) else { continue };
        let Ok(objs) = forge::parse_objects(&d) else { continue };
        for o in objs.iter().filter(|o| o.class == 0x0984_415e) {
            for r in forge::guidance::find_in_entity(o.body) {
                match r {
                    Ok(g) => {
                        ok += 1;
                        for edge in &g.edges {
                            *subs.entry(format!("{:?}", edge.subtype)).or_insert(0usize) += 1;
                            enabled += usize::from(edge.enabled && g.active);
                        }
                    }
                    Err(err) => {
                        let k = err.to_string().split(' ').take(3).collect::<Vec<_>>().join(" ");
                        fails.entry(k).or_insert((0, o.name.clone())).0 += 1;
                    }
                }
            }
        }
    }
    let failed: usize = fails.values().map(|v| v.0).sum();
    println!("guidance systems: {ok}/{} parse; {enabled} edges enabled; by subtype {subs:?}", ok + failed);
    for (k, (n, ex)) in fails {
        println!("{n:>6} {k}  e.g. {ex}");
    }
    Ok(())
}
