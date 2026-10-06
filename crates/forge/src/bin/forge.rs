use anyhow::Result;
use forge::Forge;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("list") => {
            let f = Forge::open(&args[2])?;
            println!("version {} entries {}", f.version, f.entries.len());
            for e in &f.entries {
                println!("{:016x} {:08x} {:>10} {}", e.file_id, e.type_hash, e.size, e.name);
            }
        }
        Some("extract") => {
            let f = Forge::open(&args[2])?;
            let out = std::path::Path::new(&args[3]);
            std::fs::create_dir_all(out)?;
            let filter = args.get(4);
            for e in &f.entries {
                if filter.is_some_and(|p| !e.name.contains(p.as_str())) {
                    continue;
                }
                match f.read(e) {
                    Ok(bytes) => {
                        std::fs::write(out.join(format!("{}.bin", e.name)), &bytes)?;
                        println!("{:>10} -> {:>10} {}", e.size, bytes.len(), e.name);
                    }
                    Err(err) => eprintln!("FAIL {}: {err:#}", e.name),
                }
            }
        }
        Some("textures") => {
            // textures <file.forge> <outdir> [name-filter]
            let f = Forge::open(&args[2])?;
            let out = std::path::Path::new(&args[3]);
            std::fs::create_dir_all(out)?;
            let filter = args.get(4);
            let (mut ok, mut bad) = (0, 0);
            for e in &f.entries {
                let Ok(data) = f.read(e) else { continue };
                let Ok(objs) = forge::parse_objects(&data) else { continue };
                for o in objs.iter().filter(|o| o.class == forge::CLASS_TEXTURE) {
                    if filter.is_some_and(|p| !o.name.to_lowercase().contains(&p.to_lowercase())) {
                        continue;
                    }
                    match forge::decode_texture(o.body) {
                        Ok(t) => {
                            forge::write_png(out.join(format!("{}.png", o.name)), &t)?;
                            ok += 1;
                        }
                        Err(err) => {
                            eprintln!("FAIL {}: {err:#}", o.name);
                            bad += 1;
                        }
                    }
                }
            }
            println!("textures written {ok}, failed {bad}");
        }
        _ => eprintln!("usage: forge list <file.forge> | extract <file.forge> <outdir> [name-filter]"),
    }
    Ok(())
}
