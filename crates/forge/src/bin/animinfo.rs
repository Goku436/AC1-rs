//! animinfo <forge file> <data file> <name substring>...: duration, root motion and root yaw of every
//! clip whose name contains one of the substrings. Root motion is printed in clip space (+Y forward,
//! +X right, Z up), with the yaw (degrees, positive = left) at the last frame and the first.
use anyhow::{Context, Result};
use forge::anim::{CLASS_ANIMATION, Channel, parse_animation};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    anyhow::ensure!(args.len() >= 4, "usage: animinfo <forge file> <data file> <name substring>...");
    let f = forge::Forge::open(&args[1])?;
    let e = f.entries.iter().find(|e| e.name == args[2]).with_context(|| format!("no data file {}", args[2]))?;
    let data = f.read(e)?;
    let pats: Vec<String> = args[3..].iter().map(|p| p.to_lowercase()).collect();
    let mut objs: Vec<_> =
        forge::parse_objects(&data)?.into_iter().filter(|o| o.class == CLASS_ANIMATION && pats.iter().any(|p| o.name.to_lowercase().contains(p))).collect();
    objs.sort_by(|a, b| a.name.cmp(&b.name));
    for o in objs {
        match parse_animation(o.bytes) {
            Ok(a) => {
                let [x, y, z] = a.root_motion();
                let yaw_at = |frame: f32| {
                    a.tracks
                        .iter()
                        .find(|t| t.bone_hash == 0 && matches!(t.channel, Channel::Rotation(_)))
                        .and_then(|t| t.rotation_at(frame))
                        .map_or(0.0, |[qx, qy, qz, qw]| (2.0 * (qw * qz + qx * qy)).atan2(1.0 - 2.0 * (qy * qy + qz * qz)).to_degrees())
                };
                let (yaw0, yaw) = (yaw_at(0.0), yaw_at(f32::MAX));
                println!("{:<60} {:6.2}s  right {x:6.2}  fwd {y:6.2}  up {z:6.2}  yaw {yaw:6.1} (from {yaw0:.1})", o.name, a.duration);
            }
            Err(err) => println!("{:<60} error: {err:#}", o.name),
        }
    }
    Ok(())
}
