fn main() -> anyhow::Result<()> {
    let a: Vec<String> = std::env::args().collect();
    let d = std::fs::read(&a[1])?;
    let id = u32::from_str_radix(&a[2], 16)?;
    let o = forge::parse_objects(&d)?.into_iter().find(|o| o.id == id).unwrap();
    let m = forge::mesh::parse_mesh(o.body)?;
    println!("bones {} verts {} tris {} subs {:?}", m.bones.len(), m.positions.len(), m.indices.len() / 3, m.submeshes);
    Ok(())
}
