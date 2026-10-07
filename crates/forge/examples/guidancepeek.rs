//! guidancepeek <file.forge> <subtype> [n]: the first n edges of a subtype (entity frame): ends, length, normals.
fn main() -> anyhow::Result<()> {
    let a: Vec<String> = std::env::args().collect();
    let f = forge::Forge::open(&a[1])?;
    let n: usize = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(6);
    let mut shown = 0;
    for e in &f.entries {
        let Ok(d) = f.read(e) else { continue };
        let Ok(objs) = forge::parse_objects(&d) else { continue };
        for o in objs.iter().filter(|o| o.class == 0x0984_415e) {
            for g in forge::guidance::find_in_entity(o.body).into_iter().flatten() {
                for edge in g.edges.iter().filter(|e| format!("{:?}", e.subtype) == a[2]) {
                    let len = (0..3).map(|k| (edge.b[k] - edge.a[k]).powi(2)).sum::<f32>().sqrt();
                    println!("{:40} {:?} -> {:?} len {len:.2} n {:?}", o.name, edge.a, edge.b, edge.normals);
                    shown += 1;
                    if shown >= n {
                        return Ok(());
                    }
                }
            }
        }
    }
    Ok(())
}
