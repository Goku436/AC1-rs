//! tangentcheck <forge file> <data file> [name substring]: how the meshes' stored tangent frames lie against their
//! texture coordinates: per triangle, the sign of the stored tangent along dP/du and of the binormal side (w) against
//! the side dP/dv is on (`+` agree). Generated tangents (MikkTSpace) follow +u and +v.
use anyhow::{Context, Result};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let f = forge::Forge::open(args.get(1).context("forge file")?)?;
    let name = args.get(2).context("data file")?;
    let pat = args.get(3).map(|s| s.to_lowercase()).unwrap_or_default();
    let e = f.entries.iter().find(|e| e.name == *name).context("no data file")?;
    let data = f.read(e)?;
    for o in forge::parse_objects(&data)?.iter().filter(|o| o.class == forge::mesh::CLASS_MESH && o.name.to_lowercase().contains(&pat)) {
        let Ok(m) = forge::mesh::parse_mesh(o.body) else { continue };
        let (mut tu, mut wv, mut n) = ([0usize; 2], [0usize; 2], 0);
        for t in m.indices.chunks_exact(3) {
            let [a, b, c] = [t[0], t[1], t[2]].map(|i| i as usize);
            let p = |i: usize, k: usize| m.positions[i][k];
            let e1 = [0, 1, 2].map(|k| p(b, k) - p(a, k));
            let e2 = [0, 1, 2].map(|k| p(c, k) - p(a, k));
            let (du1, dv1) = (m.uvs[b][0] - m.uvs[a][0], m.uvs[b][1] - m.uvs[a][1]);
            let (du2, dv2) = (m.uvs[c][0] - m.uvs[a][0], m.uvs[c][1] - m.uvs[a][1]);
            let det = du1 * dv2 - du2 * dv1;
            if det.abs() < 1e-9 {
                continue;
            }
            let dpdu = [0, 1, 2].map(|k| (e1[k] * dv2 - e2[k] * dv1) / det);
            let dpdv = [0, 1, 2].map(|k| (e2[k] * du1 - e1[k] * du2) / det);
            let tan = m.tangents[a];
            let nn = m.normals[a];
            let dot = |x: [f32; 3], y: [f32; 3]| x[0] * y[0] + x[1] * y[1] + x[2] * y[2];
            tu[(dot([tan[0], tan[1], tan[2]], dpdu) < 0.0) as usize] += 1;
            // The bitangent Bevy builds: w * (n x t).
            let bt = [nn[1] * tan[2] - nn[2] * tan[1], nn[2] * tan[0] - nn[0] * tan[2], nn[0] * tan[1] - nn[1] * tan[0]].map(|x| x * tan[3]);
            wv[(dot(bt, dpdv) < 0.0) as usize] += 1;
            n += 1;
        }
        println!("{:40} {n:>6} tris: tangent along +u {}+ {}-, w * (n x t) along +v {}+ {}-", o.name, tu[0], tu[1], wv[0], wv[1]);
    }
    Ok(())
}
