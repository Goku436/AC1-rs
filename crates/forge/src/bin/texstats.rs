use std::collections::BTreeMap;
fn u32a(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
fn main() -> anyhow::Result<()> {
    let dir = std::env::args().nth(1).unwrap();
    let mut st: BTreeMap<(u32, String), (usize, String)> = BTreeMap::new();
    for p in
        std::fs::read_dir(&dir)?.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "forge") && !p.to_string_lossy().contains("Sound"))
    {
        let f = forge::Forge::open(&p)?;
        for e in &f.entries {
            let Ok(d) = f.read(e) else { continue };
            let Ok(objs) = forge::parse_objects(&d) else { continue };
            for o in objs.iter().filter(|o| o.class == 0xa2b7e917) {
                let b = o.body;
                let (w, h, fmt) = (u32a(b, 0), u32a(b, 4), u32a(b, 12));
                let tag = u32a(b, 0x43);
                let size = u32a(b, 0x47) as f64;
                let bpp = if tag == 0x13237fe9 { format!("{:.2}", size * 8.0 / (w * h) as f64) } else { format!("tag {tag:08x}") };
                let s = st.entry((fmt, bpp)).or_insert((0, String::new()));
                s.0 += 1;
                if s.1.is_empty() {
                    s.1 = format!("{} {}x{} mips{}", o.name, w, h, u32a(b, 0x34));
                }
            }
        }
    }
    for ((fmt, bpp), (n, ex)) in st {
        println!("fmt {fmt:>3} bpp {bpp:>8} n {n:>6}  e.g. {ex}");
    }
    Ok(())
}
