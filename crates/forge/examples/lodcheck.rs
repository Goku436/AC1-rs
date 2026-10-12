//! lodcheck <game dir>: every `LODSelector` in every `.forge` of the install (how many parse, how many levels they
//! draw, the distances), and every city world's grid and distant stand-ins (`FakeEntities`): how many parse, whether
//! each span's indices lie in its submesh and each span's cell has a cell data file.
use std::collections::BTreeMap;

fn main() -> anyhow::Result<()> {
    let dir = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    let mut forges: Vec<_> = std::fs::read_dir(&dir)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "forge") && !p.to_string_lossy().contains("Sound"))
        .collect();
    forges.sort();
    let (mut sel, mut sel_ok) = (0, 0);
    let mut shapes: BTreeMap<String, (usize, String)> = BTreeMap::new();
    let mut fails: BTreeMap<String, (usize, String)> = BTreeMap::new();
    let mut tails: BTreeMap<[u8; 5], usize> = BTreeMap::new();
    let (mut worlds, mut worlds_ok, mut fakes, mut fakes_ok) = (0, 0, 0, 0);
    for p in forges {
        let f = forge::Forge::open(&p)?;
        let mut layout = None;
        let mut levels = 0;
        let mut fake_bodies = vec![];
        let mut meshes: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
        for e in &f.entries {
            let Ok(d) = f.read(e) else { continue };
            let Ok(objs) = forge::parse_objects(&d) else { continue };
            for o in &objs {
                match o.class {
                    forge::lod::CLASS_LOD_SELECTOR => {
                        sel += 1;
                        match forge::lod::parse_lod_selector(o.body) {
                            Ok(s) => {
                                sel_ok += 1;
                                *tails.entry(s.slots.map(|s| s.tail)).or_default() += 1;
                                let l = s.levels();
                                let key = format!(
                                    "{} levels, keep empty {}, no fade {}: until {:?}",
                                    l.len(),
                                    s.keep_empty,
                                    s.no_fade,
                                    l.iter().map(|l| l.until).collect::<Vec<_>>()
                                );
                                let e = shapes.entry(key).or_insert((0, o.name.clone()));
                                e.0 += 1;
                            }
                            Err(err) => {
                                let e = fails.entry(format!("selector: {err:#}")).or_insert((0, o.name.clone()));
                                e.0 += 1;
                            }
                        }
                    }
                    forge::world::CLASS_WORLD => {
                        worlds += 1;
                        match forge::world::grid_layout(o.body) {
                            Ok(g) => {
                                worlds_ok += 1;
                                println!("{}: {} grid {g:?}", p.file_name().unwrap_or_default().to_string_lossy(), o.name);
                                layout = Some(g);
                            }
                            Err(err) => println!("FAIL world {}: {err:#}", o.name),
                        }
                    }
                    forge::world::CLASS_GRID_PARTITION => {
                        if let Some(g) = &layout {
                            match forge::world::grid_levels(o.body, g) {
                                Ok(l) => levels = l,
                                Err(err) => println!("FAIL grid {}: {err:#}", o.name),
                            }
                        }
                    }
                    forge::world::CLASS_FAKE_ENTITIES => fake_bodies.push((o.name.clone(), o.body.to_vec())),
                    forge::mesh::CLASS_MESH if o.name.contains("FakeMesh") => {
                        if let Ok(m) = forge::mesh::parse_mesh(o.body) {
                            meshes.insert(o.id, m.submeshes.iter().map(|s| s.tri_count * 3).collect());
                        }
                    }
                    _ => {}
                }
            }
        }
        let cells: std::collections::HashSet<String> = f.entries.iter().map(|e| e.name.clone()).filter(|n| n.starts_with("Cell")).collect();
        for (name, body) in fake_bodies {
            fakes += 1;
            let Some(g) = &layout else {
                println!("FAIL {name}: no grid");
                continue;
            };
            match forge::world::fake_entities(&body, g) {
                Ok(blocks) => {
                    let spans: usize = blocks.iter().map(|b| b.spans.len()).sum();
                    // Each span inside its submesh's indices, whole triangles; its cell one with a data file.
                    let inside = blocks
                        .iter()
                        .flat_map(|b| b.spans.iter().map(move |s| (b.mesh, s)))
                        .filter(|(m, s)| {
                            meshes
                                .get(m)
                                .and_then(|sub| sub.get(s.submesh))
                                .is_some_and(|&n| s.first + s.count <= n && s.first.is_multiple_of(3) && s.count.is_multiple_of(3))
                        })
                        .count();
                    let with_file =
                        blocks.iter().flat_map(|b| &b.spans).filter(|s| cells.contains(&format!("Cell{:05}_DataBlock", s.cell.1 * g.width + s.cell.0))).count();
                    println!(
                        "{name}: {} stand-ins ({} meshes found), {spans} spans, {inside} inside their submesh, {with_file} on a cell with a data file; grid {levels} levels",
                        blocks.len(),
                        blocks.iter().filter(|b| meshes.contains_key(&b.mesh)).count()
                    );
                    if inside == spans && blocks.iter().all(|b| meshes.contains_key(&b.mesh)) {
                        fakes_ok += 1;
                    }
                }
                Err(err) => println!("FAIL {name}: {err:#}"),
            }
        }
    }
    for (k, (n, ex)) in &shapes {
        println!("{n:>6} {k}  e.g. {ex}");
    }
    for (t, n) in &tails {
        println!("{n:>6} descriptor tail bytes {t:?}");
    }
    for (err, (n, ex)) in &fails {
        println!("FAIL {n:>6} {err}  e.g. {ex}");
    }
    println!("{sel_ok}/{sel} LOD selectors parse, {worlds_ok}/{worlds} worlds' grids, {fakes_ok}/{fakes} stand-in sets check");
    Ok(())
}
