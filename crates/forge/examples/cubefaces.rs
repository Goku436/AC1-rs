//! cubefaces <forge> <cube texture name> <out dir>: a cube map's sampling (colour space, address, filters) and its six
//! faces at full size, raw RGBA8 (`<name>_face_<k>.rgba`, D3D order +X -X +Y -Y +Z -Z; the side in `<name>.txt`).
fn main() -> anyhow::Result<()> {
    let a: Vec<String> = std::env::args().collect();
    let f = forge::Forge::open(&a[1])?;
    for e in &f.entries {
        let Ok(d) = f.read(e) else { continue };
        let Ok(objs) = forge::parse_objects(&d) else { continue };
        let Some(o) = objs.iter().find(|o| o.class == forge::CLASS_TEXTURE && o.name == a[2]) else { continue };
        println!("{:?}", forge::texture::Sampling::read(o.body));
        let c = forge::texture::decode_cube(o.body)?;
        let s = c.size as usize;
        for face in 0..6 {
            let px = &c.mips[0][face * s * s * 4..(face + 1) * s * s * 4];
            std::fs::write(format!("{}/{}_face_{face}.rgba", a[3], a[2]), px)?;
        }
        std::fs::write(format!("{}/{}.txt", a[3], a[2]), s.to_string())?;
        return Ok(());
    }
    anyhow::bail!("no cube {}", a[2])
}
