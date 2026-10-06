//! meshcheck <file.forge>: parse every mesh, report failures by reason, plus stride histogram.
use std::collections::BTreeMap;
fn main() -> anyhow::Result<()> {
    let f = forge::Forge::open(std::env::args().nth(1).unwrap())?;
    let (mut ok, mut fails, mut strides) = (0, BTreeMap::<String, (usize, String)>::new(), BTreeMap::new());
    for e in &f.entries {
        let Ok(d) = f.read(e) else { continue };
        let Ok(objs) = forge::parse_objects(&d) else { continue };
        for o in objs.iter().filter(|o| o.class == forge::mesh::CLASS_MESH) {
            match forge::mesh::parse_mesh(o.body) {
                Ok(m) => {
                    ok += 1;
                    *strides.entry(m.stride).or_insert(0) += 1;
                }
                Err(err) => {
                    let k = err.to_string().split(' ').take(3).collect::<Vec<_>>().join(" ");
                    let s = fails.entry(k).or_insert((0, o.name.clone()));
                    s.0 += 1;
                }
            }
        }
    }
    println!("ok {ok} strides {strides:?}");
    for (k, (n, ex)) in fails {
        println!("{n:>6} {k}  e.g. {ex}");
    }
    Ok(())
}
