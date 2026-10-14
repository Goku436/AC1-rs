//! buildcheck <forge file>...: every NPC build table and builder (`forge::build`) of the forges: how many columns read
//! against the `BuildColumn` tags, the cells whose column is not the table's, and the columns by kind and property
//! (names by CRC32 where known). `buildcheck dump <forge file> <data file>`: that data file's tables, column by column
//! and row by row.
use std::collections::BTreeMap;

const NAMES: &[&str] = &[
    "Color_R",
    "Color_G",
    "Color_B",
    "Dirt",
    "DirtIntensity",
    "DirtMask",
    "DirtTile",
    "DiffuseMask",
    "MultiplyColor",
    "TextureSet",
    "AlphaTexture",
    "AlphaVector",
    "Visual",
    "SkinColor",
    "HairColor",
    "HairSpecular",
    "Dirt_Color",
    "Dirt_Mask",
    "HeadMultiply",
    "EyeColor",
    "DiffuseColor",
    "MultiplyMask",
    "MultiplyTexture",
    "Scale",
    "DiffuseMap",
    "DiffuseTexture",
    "NormalTexture",
    "SpecularTexture",
    "SpecularColor",
];

fn main() -> anyhow::Result<()> {
    let names: BTreeMap<u32, &str> = NAMES.iter().map(|n| (crc32fast::hash(n.as_bytes()), *n)).collect();
    let name = |p: u32| names.get(&p).map_or(format!("#{p:08x}"), |n| n.to_string());
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).is_some_and(|a| a == "dump") {
        let f = forge::Forge::open(&args[2])?;
        let e = f.entries.iter().find(|e| e.name == args[3]).ok_or_else(|| anyhow::anyhow!("no data file {}", args[3]))?;
        let d = f.read(e)?;
        for o in forge::parse_objects(&d)?.iter().filter(|o| o.class == forge::build::CLASS_BUILD_TABLE || o.class == forge::build::CLASS_BUILDER) {
            let t = forge::build::parse(o.body, o.id);
            println!("{} ({:08x})", o.name, o.id);
            for c in &t.columns {
                let p: Vec<String> = c.path.iter().map(|&p| name(p)).collect();
                println!("  column {}: kind {:02x} material {:08x} template {:08x} {}", c.number, c.kind, c.material, c.template, p.join("."));
            }
            for r in &t.rows {
                let cells: Vec<String> = r.cells.iter().map(|c| format!("{}={:x?}", c.column, c.value)).collect();
                println!("  row {:.3}: {}", r.weight, cells.join(" "));
            }
        }
        return Ok(());
    }
    let (mut tables, mut tags, mut read, mut cells, mut orphans) = (0, 0, 0, 0, 0);
    let mut by: BTreeMap<(u8, String, &str), usize> = BTreeMap::new();
    for path in std::env::args().skip(1) {
        let f = forge::Forge::open(&path)?;
        for e in &f.entries {
            let Ok(d) = f.read(e) else { continue };
            let Ok(objs) = forge::parse_objects(&d) else { continue };
            for o in objs.iter().filter(|o| o.class == forge::build::CLASS_BUILD_TABLE || o.class == forge::build::CLASS_BUILDER) {
                tables += 1;
                let t = forge::build::parse(o.body, o.id);
                let first_row = t.spans.first().map_or(o.body.len(), |s| s.0);
                tags += o.body[..first_row].windows(4).filter(|w| *w == 0x3683_9608u32.to_le_bytes()).count();
                read += t.columns.len();
                for c in &t.columns {
                    let target = if c.material != 0 {
                        "material"
                    } else if c.template != 0 {
                        "template"
                    } else {
                        "none"
                    };
                    let p: Vec<String> = c.path.iter().map(|&p| name(p)).collect();
                    *by.entry((c.kind, p.join("."), target)).or_default() += 1;
                }
                for r in &t.rows {
                    for c in &r.cells {
                        cells += 1;
                        if t.column(c.column).is_none() {
                            orphans += 1;
                            if orphans <= 20 {
                                println!("  {} ({:08x}): a cell of column {} ({:x?}), not the table's", o.name, o.id, c.column, c.value);
                            }
                        }
                    }
                }
            }
        }
    }
    println!("{tables} tables and builders: {read}/{tags} columns read, {cells} cells, {orphans} naming no column of their table");
    for ((kind, p, target), n) in by {
        println!("  kind {kind:02x} {p:>16} on {target:>8}: {n}");
    }
    Ok(())
}
