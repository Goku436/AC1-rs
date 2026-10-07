//! clipjumps <forge file> <data file> <clip name>... [--min degrees]: frames where a bone's rotation in a clip changes
//! much more than in the frames around it (a jump baked into the clip, or a codec problem), the largest first.
use anyhow::{Context, Result};
use forge::anim::{CLASS_ANIMATION, Channel, parse_animation};

fn angle(a: [f32; 4], b: [f32; 4]) -> f32 {
    let d = (a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3]).abs().min(1.0);
    2.0 * d.acos().to_degrees()
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    anyhow::ensure!(args.len() >= 4, "usage: clipjumps <forge file> <data file> <clip name>... [--min degrees]");
    let min: f32 = args.iter().position(|a| a == "--min").and_then(|i| args.get(i + 1)).and_then(|s| s.parse().ok()).unwrap_or(10.0);
    let f = forge::Forge::open(&args[1])?;
    let e = f.entries.iter().find(|e| e.name == args[2]).with_context(|| format!("no data file {}", args[2]))?;
    let data = f.read(e)?;
    let objs = forge::parse_objects(&data)?;
    for name in args[3..].iter().take_while(|a| *a != "--min") {
        let Some(a) = objs.iter().find(|o| o.class == CLASS_ANIMATION && o.name == *name).and_then(|o| parse_animation(o.bytes).ok()) else {
            println!("{name}: missing");
            continue;
        };
        let frames = (a.duration * forge::anim::FPS).round() as usize;
        let mut jumps = vec![];
        for t in a.tracks.iter().filter(|t| matches!(t.channel, Channel::Rotation(_))) {
            let at = |k: usize| t.rotation_at(k as f32);
            for k in 2..=frames {
                let (Some(q0), Some(q1), Some(q2)) = (at(k - 2), at(k - 1), at(k)) else { continue };
                let (before, now) = (angle(q0, q1), angle(q1, q2));
                if now > min && now > 3.0 * before {
                    jumps.push((now, k, t.bone_hash, before));
                }
            }
        }
        jumps.sort_by(|x, y| y.0.total_cmp(&x.0));
        println!("{name}: {frames} frames, {} jumps", jumps.len());
        for (now, k, bone, before) in jumps.iter().take(8) {
            println!("  frame {k:3}  bone {bone:08x}  {now:6.1} deg (before {before:.1})");
        }
    }
    Ok(())
}
