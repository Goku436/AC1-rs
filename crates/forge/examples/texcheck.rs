//! texcheck <game dir>: every TextureMap, TextureGradient and Material in every `.forge` of the install: how many
//! decode, and histograms of the sampling fields (GammaSettings, AddressMode, FilterType) by the map's kind (the
//! name's Diffuse / Normal / Specular / other), of the material templates and of the parameter types.
use std::collections::BTreeMap;

fn kind(name: &str) -> &'static str {
    let n = name.to_lowercase();
    if n.contains("normal") || n.ends_with("_n") || n.contains("_n_") {
        "normal"
    } else if n.contains("spec") || n.ends_with("_s") {
        "specular"
    } else if n.contains("diffuse") || n.ends_with("_d") || n.contains("_d_") {
        "diffuse"
    } else {
        "other"
    }
}

fn main() -> anyhow::Result<()> {
    let dir = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    let mut forges: Vec<_> = std::fs::read_dir(&dir)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "forge") && !p.to_string_lossy().contains("Sound"))
        .collect();
    forges.sort();
    let (mut tex, mut tex_ok, mut grad, mut grad_ok, mut cube, mut cube_ok, mut mat, mut mat_ok) = (0, 0, 0, 0, 0, 0, 0, 0);
    let mut sampling: BTreeMap<(&str, forge::texture::Sampling), (usize, String)> = BTreeMap::new();
    let mut fails: BTreeMap<String, (usize, String)> = BTreeMap::new();
    let mut templates: BTreeMap<u32, (usize, String)> = BTreeMap::new();
    for p in forges {
        let f = forge::Forge::open(&p)?;
        for e in &f.entries {
            let Ok(d) = f.read(e) else { continue };
            let Ok(objs) = forge::parse_objects(&d) else { continue };
            for o in &objs {
                if o.class == forge::CLASS_TEXTURE {
                    tex += 1;
                    match forge::decode_texture(o.body) {
                        Ok(t) => {
                            tex_ok += 1;
                            if t.cube {
                                cube += 1;
                                cube_ok += forge::texture::decode_cube(o.body).is_ok() as usize;
                            }
                            let s = sampling.entry((kind(&o.name), t.sampling)).or_insert((0, String::new()));
                            s.0 += 1;
                            if s.1.is_empty() {
                                s.1 = o.name.clone();
                            }
                        }
                        Err(err) => {
                            let s = fails.entry(format!("{err:#}").chars().take(60).collect()).or_insert((0, String::new()));
                            s.0 += 1;
                            s.1 = o.name.clone();
                        }
                    }
                } else if o.class == forge::texture::CLASS_GRADIENT {
                    grad += 1;
                    match forge::texture::decode_gradient(o.body) {
                        Ok(_) => grad_ok += 1,
                        Err(err) => println!("gradient {}: {err:#}", o.name),
                    }
                } else if o.class == forge::datafile::CLASS_MATERIAL {
                    mat += 1;
                    match forge::material::Material::parse(o.body) {
                        Ok(m) => {
                            mat_ok += 1;
                            let s = templates.entry(m.template).or_insert((0, String::new()));
                            s.0 += 1;
                            if s.1.is_empty() {
                                s.1 = o.name.clone();
                            }
                        }
                        Err(err) => {
                            let s = fails.entry(format!("material: {err:#}").chars().take(60).collect()).or_insert((0, String::new()));
                            s.0 += 1;
                            s.1 = o.name.clone();
                        }
                    }
                }
            }
        }
    }
    for ((k, s), (n, ex)) in &sampling {
        println!("{k:>8} srgb {:>5} address {:?} filter {:?}: {n:>6}  e.g. {ex}", s.srgb, s.address, s.filter);
    }
    for (t, (n, ex)) in &templates {
        println!("template {t:>10}: {n:>5} materials, e.g. {ex}");
    }
    for (err, (n, ex)) in &fails {
        println!("FAIL {n:>6} {err}  e.g. {ex}");
    }
    println!("{tex_ok}/{tex} textures decode ({cube_ok}/{cube} cubes), {grad_ok}/{grad} gradients, {mat_ok}/{mat} materials parse");
    Ok(())
}
