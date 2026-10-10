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
                // `ANIMINFO_JUMPS=1`: every bone rotation that turns more than 60 degrees between two frames (a flip in
                // the clip's data, or in how it is sampled).
                // `ANIMINFO_TURNS=1`: the bones whose yaw (about the clip's up, Z) changes over 45 degrees across the clip,
                // at its start, middle and end.
                if std::env::var("ANIMINFO_TURNS").is_ok() {
                    let yaw = |q: [f32; 4]| (2.0 * (q[3] * q[2] + q[0] * q[1])).atan2(1.0 - 2.0 * (q[1] * q[1] + q[2] * q[2])).to_degrees();
                    let end = a.duration * 30.0;
                    for t in a.tracks.iter().filter(|t| matches!(t.channel, Channel::Rotation(_))) {
                        let ys: Vec<f32> = [0.0, 0.25, 0.5, 0.75, 1.0].iter().filter_map(|k| t.rotation_at(end * k).map(yaw)).collect();
                        if ys.len() == 5 && ys.windows(2).map(|w| ((w[1] - w[0] + 540.0) % 360.0 - 180.0).abs()).sum::<f32>() > 45.0 {
                            println!("    bone {:08x} yaw {:?}", t.bone_hash, ys.iter().map(|y| y.round()).collect::<Vec<_>>());
                        }
                    }
                }
                if std::env::var("ANIMINFO_JUMPS").is_ok() {
                    let frames = (a.duration * 30.0).round() as usize;
                    for t in a.tracks.iter().filter(|t| matches!(t.channel, Channel::Rotation(_))) {
                        for f in 0..frames * 2 {
                            let (Some(p), Some(q)) = (t.rotation_at(f as f32 * 0.5), t.rotation_at(f as f32 * 0.5 + 0.5)) else { continue };
                            let dot = (p[0] * q[0] + p[1] * q[1] + p[2] * q[2] + p[3] * q[3]).abs().min(1.0);
                            let deg = 2.0 * dot.acos().to_degrees();
                            if deg > 60.0 {
                                println!("    bone {:08x}: {deg:5.1} degrees from frame {:.1} to {:.1}", t.bone_hash, f as f32 * 0.5, f as f32 * 0.5 + 0.5);
                            }
                        }
                    }
                }
            }
            Err(err) => println!("{:<60} error: {err:#}", o.name),
        }
    }
    Ok(())
}
