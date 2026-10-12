//! ambcheck <game dir>: every level's `WorldAmbiance` and `LayeredSky` (`forge::ambiance`): how many parse, and their
//! values.
use anyhow::{Context, Result};

fn main() -> Result<()> {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).context("game dir")?);
    let (mut ok, mut all) = (0, 0);
    let mut forges: Vec<_> =
        std::fs::read_dir(&dir)?.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "forge")).collect();
    forges.sort();
    for path in forges {
        let f = forge::Forge::open(&path)?;
        for e in &f.entries {
            // (Ambiances sit in the level's own small data files, not the cells.)
            if e.name.starts_with("Cell") {
                continue;
            }
            let Ok(data) = f.read(e) else { continue };
            let Ok(objects) = forge::parse_objects(&data) else { continue };
            if !objects.iter().any(|o| o.class == forge::ambiance::CLASS_WORLD_AMBIANCE) {
                continue;
            }
            all += 1;
            match forge::ambiance::find_in(objects.iter()) {
                Ok(forge::ambiance::AmbiancePair { ambiance: a, sky: s }) => {
                    ok += 1;
                    println!("{} / {}:", path.file_name().unwrap_or_default().to_string_lossy(), e.name);
                    println!("  {:?}", a.fog);
                    println!("  {:?}", a.colour_control);
                    println!("  {:?} glow {}", a.color_balance, a.glow);
                    println!("  sun {:?}", s.sun);
                    println!("  character sun ({}) {:?}", s.use_character_sun, s.character_sun);
                    println!("  environment {:?}", s.environment_ambient);
                    println!("  character ({}) {:?}", s.use_character_ambient, s.character_ambient);
                    println!("  sky layers (mesh, material) {:x?}", s.layers);
                }
                Err(err) => println!("{} / {}: {err:#}", path.display(), e.name),
            }
        }
    }
    println!("{ok}/{all} ambiances read");
    Ok(())
}
