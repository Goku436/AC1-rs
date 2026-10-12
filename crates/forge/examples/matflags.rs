//! matflags <forge file> [name pattern]: each material's template id and the bytes between its texture set and its
//! parameter table (+0x08..+0x39), counted by distinct byte string.
use std::collections::BTreeMap;
fn main() -> anyhow::Result<()> {
    let a: Vec<String> = std::env::args().collect();
    let f = forge::Forge::open(&a[1])?;
    let pat = a.get(2).map(|s| s.to_lowercase()).unwrap_or_default();
    let mut rows: BTreeMap<(u32, Vec<u8>), (usize, String)> = BTreeMap::new();
    for e in &f.entries {
        let Ok(d) = f.read(e) else { continue };
        let Ok(objs) = forge::parse_objects(&d) else { continue };
        for o in objs.iter().filter(|o| o.class == forge::datafile::CLASS_MATERIAL && o.body.len() >= 0x39 && o.name.to_lowercase().contains(&pat)) {
            let t = u32::from_le_bytes(o.body[0..4].try_into()?);
            let r = rows.entry((t, o.body[8..0x39].to_vec())).or_insert((0, o.name.clone()));
            r.0 += 1;
        }
    }
    for ((t, b), (n, name)) in rows {
        let hex: Vec<String> = b.iter().map(|x| format!("{x:02x}")).collect();
        println!("{t:08x} {n:>5} {} {name}", hex.join(""));
    }
    Ok(())
}
