//! matdump <forge file> <data file> [name substring]: the materials' templates, inside materials and parameters
//! (names by CRC32 where known).
use anyhow::{Context, Result};

const NAMES: &[&str] = &[
    "DiffuseTexture",
    "DiffuseMap",
    "NormalTexture",
    "NormalMap",
    "SpecularTexture",
    "SpecularMap",
    "MultiplyTexture",
    "MultiplyMap",
    "CubeMap",
    "MultiplyColor",
    "MultiplyColorValue",
    "SpecularColor",
    "SpecularPower",
    "SpecularFactor",
    "Rim_Power",
    "RimPower",
    "Fresnel",
    "SpecularTiling",
    "NormalMapBlend",
    "EyeColor",
];

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let f = forge::Forge::open(args.get(1).context("forge file")?)?;
    let name = args.get(2).context("data file")?;
    let pat = args.get(3).map(|s| s.to_lowercase()).unwrap_or_default();
    let e = f.entries.iter().find(|e| e.name == *name).context("no data file")?;
    let data = f.read(e)?;
    for o in forge::parse_objects(&data)?.iter().filter(|o| o.class == forge::datafile::CLASS_MATERIAL && o.name.to_lowercase().contains(&pat)) {
        match forge::material::Material::parse(o.body) {
            Ok(m) => {
                println!("{} ({:08x}): template {} {:?}, set {:08x}, inside {:?}", o.name, o.id, m.template, m.style(), m.texture_set, m.inside);
                for (id, v) in &m.params {
                    let n = NAMES.iter().find(|n| forge::material::param_id(n) == *id).map_or(format!("#{id}"), |n| n.to_string());
                    println!("    {n:>20}: {v:?}");
                }
            }
            Err(err) => println!("{}: {err:#}", o.name),
        }
    }
    Ok(())
}
