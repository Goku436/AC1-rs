//! idxgrep <forge file> <name substring> [class hex] [pos]: objects of a forge by name, from its cached index;
//! `pos` prints each one's world position (game space, the transform its body starts with).
fn main() -> anyhow::Result<()> {
    let a: Vec<String> = std::env::args().collect();
    let path = std::path::Path::new(&a[1]);
    let idx = forge::index::ForgeIndex::load(path)?;
    let pat = a[2].to_lowercase();
    let class = a.get(3).and_then(|c| u32::from_str_radix(c, 16).ok());
    let pos = a.iter().any(|x| x == "pos");
    let f = forge::Forge::open(path)?;
    let mut v: Vec<_> = idx.objects.iter().filter(|(_, p)| p.name.to_lowercase().contains(&pat) && class.is_none_or(|c| c == p.class)).collect();
    v.sort_by(|a, b| a.1.name.cmp(&b.1.name));
    for (id, p) in v {
        let mut at = String::new();
        if pos && let Some(e) = f.entries.iter().find(|e| e.name == p.datafile) {
            let d = f.read(e)?;
            if let Some(o) = forge::parse_objects(&d)?.into_iter().find(|o| o.id == *id) && o.body.len() >= 64 {
                let g = |i: usize| f32::from_le_bytes(o.body[i * 4..i * 4 + 4].try_into().unwrap());
                at = format!(" at game ({:.1}, {:.1}, {:.1}) bevy x {:.1} z {:.1}", g(12), g(13), g(14), g(12), -g(13));
            }
        }
        println!("{id:08x} {:08x} {} | {}{at}", p.class, p.name, p.datafile);
    }
    Ok(())
}
