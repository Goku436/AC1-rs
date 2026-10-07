//! Bounds, size and bones of each mesh in a data file whose name contains a pattern.
//! Usage: meshbounds <game dir> <forge file> <data file> [name pattern]
//! Also lists the file's skeletons, and which of them hold the bones named (hex hashes) in `FIND_BONES`.
use forge::Forge;
use forge::datafile::DataFile;
use forge::mesh::CLASS_MESH;

fn main() -> anyhow::Result<()> {
    let a: Vec<String> = std::env::args().collect();
    let forge = Forge::open(std::path::Path::new(&a[1]).join(&a[2]))?;
    let entry = forge.entries.iter().find(|e| e.name == a[3]).ok_or_else(|| anyhow::anyhow!("no data file {}", a[3]))?;
    let bytes = forge.read(entry)?;
    let df = DataFile::parse(&bytes)?;
    let pat = a.get(4).map(|s| s.to_lowercase()).unwrap_or_default();
    for o in df.objects.iter().filter(|o| o.class == CLASS_MESH && o.name.to_lowercase().contains(&pat)) {
        let m = match forge::mesh::parse_mesh(o.body) {
            Ok(m) => m,
            Err(e) => {
                println!("{}: {e:#}", o.name);
                continue;
            }
        };
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        for p in &m.positions {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        let bones: Vec<String> = m.bones.iter().map(|b| format!("{:08x}", b.name_hash)).collect();
        println!(
            "{} ({:08x}): {} verts, {} subs, size {:.2} x {:.2} x {:.2}, from {:.2?} to {:.2?}, bones {:?}",
            o.name,
            o.id,
            m.positions.len(),
            m.submeshes.len(),
            hi[0] - lo[0],
            hi[1] - lo[1],
            hi[2] - lo[2],
            lo,
            hi,
            bones
        );
    }
    let find: Vec<u32> = std::env::var("FIND_BONES").unwrap_or_default().split(',').filter_map(|h| u32::from_str_radix(h.trim(), 16).ok()).collect();
    for o in df.objects.iter().filter(|o| o.class == forge::skeleton::CLASS_SKELETON) {
        let Ok(s) = forge::skeleton::parse_skeleton(o.body) else { continue };
        let held: Vec<String> = find.iter().filter(|h| s.bones.iter().any(|b| b.name_hash == **h)).map(|h| format!("{h:08x}")).collect();
        println!("skeleton {} ({:08x}): {} bones, holds {held:?}", o.name, o.id, s.bones.len());
    }
    Ok(())
}
