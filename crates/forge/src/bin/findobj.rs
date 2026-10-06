//! findobj <game dir> <name substring> [class hex]: list objects across all archives.
use anyhow::Result;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let pat = args[2].to_lowercase();
    let class = args.get(3).map(|c| u32::from_str_radix(c, 16).unwrap());
    let mut paths: Vec<_> = std::fs::read_dir(&args[1])?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "forge") && !p.to_string_lossy().contains("Sound"))
        .collect();
    paths.sort();
    for p in paths {
        let f = forge::Forge::open(&p)?;
        let fname = p.file_name().unwrap().to_string_lossy().into_owned();
        for e in &f.entries {
            let Ok(data) = f.read(e) else { continue };
            let Ok(objs) = forge::parse_objects(&data) else { continue };
            for o in objs {
                if class.is_some_and(|c| c != o.class) || !o.name.to_lowercase().contains(&pat) {
                    continue;
                }
                println!("{fname} | {} | {:08x} {:08x} {:>8} {}", e.name, o.id, o.class, o.bytes.len(), o.name);
            }
        }
    }
    Ok(())
}
