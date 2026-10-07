//! decodecheck <game dir>: read every data file in every `.forge` of the install; print the ones that fail (with the
//! error, which names the compression method, or the object table's) and the pass rates.
use anyhow::Result;

fn main() -> Result<()> {
    let dir = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    let mut forges: Vec<_> =
        std::fs::read_dir(&dir)?.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "forge")).collect();
    forges.sort();
    let (mut ok, mut all, mut split) = (0usize, 0usize, 0usize);
    for path in forges {
        let f = forge::Forge::open(&path)?;
        for e in &f.entries {
            all += 1;
            match f.read(e) {
                Ok(d) => {
                    ok += 1;
                    match forge::parse_objects(&d) {
                        Ok(_) => split += 1,
                        Err(err) => println!("{} / {}: objects: {err:#}", path.file_name().unwrap_or_default().to_string_lossy(), e.name),
                    }
                }
                Err(err) => println!("{} / {}: {err:#}", path.file_name().unwrap_or_default().to_string_lossy(), e.name),
            }
        }
    }
    println!("{ok}/{all} data files decompress, {split} split into objects");
    Ok(())
}
