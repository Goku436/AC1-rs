//! dumpobj <datafile.bin> <object id hex> <out.bin>
fn main() -> anyhow::Result<()> {
    let a: Vec<String> = std::env::args().collect();
    let d = std::fs::read(&a[1])?;
    let id = u32::from_str_radix(&a[2], 16)?;
    let o = forge::parse_objects(&d)?.into_iter().find(|o| o.id == id).expect("no such object");
    std::fs::write(&a[3], o.bytes)?;
    Ok(())
}
