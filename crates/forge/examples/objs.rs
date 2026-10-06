fn main() -> anyhow::Result<()> {
    let p = std::env::args().nth(1).unwrap();
    let d = std::fs::read(&p)?;
    for o in forge::parse_objects(&d)? {
        println!("{:08x} {:08x} {:>8} {}", o.id, o.class, o.bytes.len(), o.name);
    }
    Ok(())
}
