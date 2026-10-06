//! Histogram of object classes across every .forge in a game folder.
//!
//! A class id is the CRC32 of the engine's class name. The names are read at run time from the game's own
//! executable (its RTTI type descriptors, `.?AV<name>@scimitar@@`, and other identifier strings), so each
//! row is labelled with its class name; nothing from the executable is kept in this repo.
use anyhow::Result;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

/// CRC32 -> name for every identifier-like string in the game's executables (`*.exe` in `dir`).
fn class_names(dir: &Path) -> HashMap<u32, String> {
    let mut names = HashMap::new();
    let exes = std::fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "exe"));
    for exe in exes {
        let Ok(bin) = std::fs::read(&exe) else { continue };
        // Runs of [A-Za-z0-9_] starting with a capital, cut at '@' (RTTI's namespace separator).
        let mut start = None;
        for (i, &b) in bin.iter().chain(std::iter::once(&0)).enumerate() {
            if b.is_ascii_alphanumeric() || b == b'_' {
                start.get_or_insert(i);
                continue;
            }
            if let Some(s) = start.take() {
                let word = &bin[s..i];
                if (4..=80).contains(&word.len()) && word[0].is_ascii_uppercase() {
                    names.entry(crc32fast::hash(word)).or_insert_with(|| String::from_utf8_lossy(word).into_owned());
                }
            }
        }
    }
    names
}

fn main() -> Result<()> {
    let dir = std::env::args().nth(1).expect("game dir");
    let mut classes: BTreeMap<u32, (usize, u64, Vec<String>)> = BTreeMap::new();
    let mut paths: Vec<_> = std::fs::read_dir(&dir)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "forge") && !p.to_string_lossy().contains("Sound"))
        .collect();
    paths.sort();
    let (mut ok, mut bad) = (0, 0);
    for p in paths {
        let f = forge::Forge::open(&p)?;
        for e in &f.entries {
            let Ok(data) = f.read(e) else {
                bad += 1;
                continue;
            };
            let Ok(objs) = forge::parse_objects(&data) else {
                bad += 1;
                continue;
            };
            ok += 1;
            for o in objs {
                let c = classes.entry(o.class).or_default();
                c.0 += 1;
                c.1 += o.bytes.len() as u64;
                if c.2.len() < 4 {
                    c.2.push(o.name.clone());
                }
            }
        }
        eprintln!("{} done", p.display());
    }
    println!("datafiles ok {ok} failed {bad}");
    let names = class_names(Path::new(&dir));
    let mut v: Vec<_> = classes.into_iter().collect();
    v.sort_by_key(|(_, c)| std::cmp::Reverse(c.1));
    for (k, (n, b, objs)) in v {
        let class = names.get(&k).map_or("?", |s| s.as_str());
        println!("{k:08x} {class:<30} {n:>8} {:>8}KB  {}", b / 1024, objs.join(" | "));
    }
    Ok(())
}
