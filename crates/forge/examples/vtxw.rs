//! vtxw <forge> [name substring]: static meshes' vertex colours and layer weights (`forge::mesh::Mesh::colors`,
//! `layers`): how many vertices are white, grey or coloured, how many have a faded alpha or a layer weight, and the
//! meshes with coloured vertices.
fn main() -> anyhow::Result<()> {
    let a: Vec<String> = std::env::args().collect();
    let f = forge::Forge::open(&a[1])?;
    let pat = a.get(2).cloned().unwrap_or_default();
    let (mut n, mut white, mut grey, mut col, mut faded, mut layered) = (0usize, 0usize, 0usize, 0usize, 0usize, 0usize);
    let mut examples = vec![];
    for e in &f.entries {
        let Ok(d) = f.read(e) else { continue };
        let Ok(objs) = forge::parse_objects(&d) else { continue };
        for o in objs.iter().filter(|o| o.class == forge::mesh::CLASS_MESH && o.name.contains(pat.as_str())) {
            let Ok(m) = forge::mesh::parse_mesh(o.body) else { continue };
            let mut coloured = None;
            for (c, l) in m.colors.iter().zip(&m.layers) {
                n += 1;
                if c[..3] == [255, 255, 255] {
                    white += 1;
                } else if c[0] == c[1] && c[1] == c[2] {
                    grey += 1;
                } else {
                    col += 1;
                    coloured.get_or_insert(*c);
                }
                faded += (c[3] < 250) as usize;
                layered += (*l != [0, 0]) as usize;
            }
            if let Some(c) = coloured {
                examples.push(format!("{} ({} vertices), e.g. RGBA {c:?}", o.name, m.colors.len()));
            }
        }
    }
    println!("{n} static vertices: {white} white, {grey} grey, {col} coloured, {faded} with alpha under 250, {layered} with a layer weight");
    for e in examples.iter().filter(|e| !e.contains("FakeMesh")).take(80) {
        println!("  {e}");
    }
    Ok(())
}
