//! templates <game dir> <city forge> [template name substring] [all]: the shader templates (`MaterialTemplate`, bcfb3c7a) a city's
//! materials use, most used first: how many materials, how many of the city's mesh submeshes name them, and each
//! template's parameters by name (the material's parameter CRC32s matched against the identifiers in the template's
//! own shader constant tables, with their `_<n>` node suffix cut), kind and an example value (`all`: every material's).
use std::collections::{BTreeMap, HashMap};

const CLASS_TEMPLATE: u32 = 0xbcfb_3c7a;

fn identifiers(body: &[u8]) -> HashMap<u32, String> {
    let mut out = HashMap::new();
    let mut i = 0;
    while i < body.len() {
        let start = i;
        while i < body.len() && (body[i].is_ascii_alphanumeric() || body[i] == b'_') {
            i += 1;
        }
        if i - start >= 3 {
            let s = String::from_utf8_lossy(&body[start..i]).into_owned();
            let mut forms = vec![s.clone()];
            // `Name_7`, `Name_7_matrix`: the shader builder's node suffixes.
            let t = s.trim_end_matches("_matrix");
            if let Some(k) = t.rfind('_')
                && t[k + 1..].chars().all(|c| c.is_ascii_digit())
            {
                forms.push(t[..k].to_string());
            }
            for f in forms {
                out.entry(crc32fast::hash(f.as_bytes())).or_insert(f);
            }
        }
        i += 1;
    }
    out
}

fn main() -> anyhow::Result<()> {
    let a: Vec<String> = std::env::args().collect();
    let dir = std::path::Path::new(&a[1]);
    let pat = a.get(3).map(|s| s.to_lowercase()).unwrap_or_default();
    // Templates: DataPC's and the city's and Common's.
    let mut templates: HashMap<u32, (String, HashMap<u32, String>)> = HashMap::new();
    let mut materials: HashMap<u32, (String, forge::material::Material)> = HashMap::new();
    let mut uses: HashMap<u32, usize> = HashMap::new();
    let mut bad = 0;
    for name in ["DataPC.forge", "DataPC_Common.forge", a[2].as_str()] {
        let f = forge::Forge::open(dir.join(name))?;
        let city = name == a[2];
        for e in &f.entries {
            let Ok(d) = f.read(e) else { continue };
            let Ok(objs) = forge::parse_objects(&d) else { continue };
            for o in &objs {
                if o.class == CLASS_TEMPLATE {
                    templates.entry(o.id).or_insert_with(|| (o.name.clone(), identifiers(o.body)));
                } else if city && o.class == forge::datafile::CLASS_MATERIAL {
                    match forge::material::Material::parse(o.body) {
                        Ok(m) => {
                            materials.entry(o.id).or_insert((o.name.clone(), m));
                        }
                        Err(_) => bad += 1,
                    }
                } else if city
                    && o.class == forge::mesh::CLASS_MESH
                    && let Ok(m) = forge::mesh::parse_mesh(o.body)
                {
                    for id in m.materials {
                        *uses.entry(id).or_default() += 1;
                    }
                }
            }
        }
    }
    let mut by: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for (id, (_, m)) in &materials {
        by.entry(m.template).or_default().push(*id);
    }
    let mut rows: Vec<_> = by.into_iter().collect();
    rows.sort_by_key(|(_, ms)| std::cmp::Reverse(ms.iter().map(|m| uses.get(m).copied().unwrap_or(0)).sum::<usize>()));
    println!("{} materials parsed ({bad} not), {} templates", materials.len(), rows.len());
    for (t, ms) in rows {
        let (tname, ids) = templates.get(&t).map_or(("(not found)".to_string(), HashMap::new()), |(n, i)| (n.clone(), i.clone()));
        if !tname.to_lowercase().contains(&pat) {
            continue;
        }
        let used: usize = ms.iter().map(|m| uses.get(m).copied().unwrap_or(0)).sum();
        let all = a.get(4).is_some_and(|x| x == "all");
        println!("{tname} ({t:08x}): {} materials, {used} submeshes", ms.len());
        for m in if all { &ms[..] } else { &ms[..1] } {
            let (ename, em) = &materials[m];
            println!("  {ename}");
            for (id, v) in &em.params {
                let n = ids.get(id).cloned().unwrap_or_else(|| format!("#{id:08x}"));
                println!("    {n:>24}: {v:?}");
            }
        }
    }
    Ok(())
}
