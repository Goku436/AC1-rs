//! animhex <file.forge> <datafile name> <object id hex> <type byte hex>: hex around the first block of that type
fn main() -> anyhow::Result<()> {
    let a: Vec<String> = std::env::args().collect();
    let f = forge::Forge::open(&a[1])?;
    let e = f.entries.iter().find(|e| e.name == a[2]).unwrap();
    let d = f.read(e)?;
    let id = u32::from_str_radix(&a[3], 16)?;
    let o = forge::parse_objects(&d)?.into_iter().find(|o| o.id == id).unwrap();
    std::fs::write("out/anim/sample.obj", o.bytes)?;
    println!("len {}", o.bytes.len());
    Ok(())
}
