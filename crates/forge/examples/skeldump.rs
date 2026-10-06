//! skeldump <datafile.bin> <skeleton id hex>: hash parent local_pos local_rot
fn main() -> anyhow::Result<()> {
    let a: Vec<String> = std::env::args().collect();
    let d = std::fs::read(&a[1])?;
    let id = u32::from_str_radix(&a[2], 16)?;
    let o = forge::parse_objects(&d)?.into_iter().find(|o| o.id == id).unwrap();
    let s = forge::skeleton::parse_skeleton(o.body)?;
    for b in &s.bones {
        let p = b.parent.map(|p| s.bones[p].name_hash).unwrap_or(0);
        println!(
            "{:08x} {:08x} {} {} {} {} {} {} {}",
            b.name_hash, p, b.local_pos[0], b.local_pos[1], b.local_pos[2], b.local_rot[0], b.local_rot[1], b.local_rot[2], b.local_rot[3]
        );
    }
    Ok(())
}
