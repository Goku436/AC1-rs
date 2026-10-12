//! shapebounds <forge file> <data file> [name pattern]: bounds of each collision MeshShape in a data file.
fn main() -> anyhow::Result<()> {
    let a: Vec<String> = std::env::args().collect();
    let f = forge::Forge::open(&a[1])?;
    let e = f.entries.iter().find(|e| e.name == a[2]).ok_or_else(|| anyhow::anyhow!("no {}", a[2]))?;
    let d = f.read(e)?;
    let pat = a.get(3).map(|s| s.to_lowercase()).unwrap_or_default();
    for o in forge::parse_objects(&d)?.iter().filter(|o| o.class == forge::shape::CLASS_MESH_SHAPE && o.name.to_lowercase().contains(&pat)) {
        let s = forge::shape::parse_mesh_shape(o.body)?;
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        for v in &s.vertices {
            for k in 0..3 {
                lo[k] = lo[k].min(v[k]);
                hi[k] = hi[k].max(v[k]);
            }
        }
        println!("{}: {} verts, {:.2?} .. {:.2?}", o.name, s.vertices.len(), lo, hi);
    }
    Ok(())
}
