//! clipseam <forge file> <data file> <from clip> <to clip>...: how far the first frame of each `to` clip is from the last
//! frame of `from` (the mean and largest bone rotation difference, degrees), the nearest first: which clip a code-driven
//! move goes on into when the move graph does not say.
use anyhow::{Context, Result};
use forge::anim::{Animation, CLASS_ANIMATION, Channel, parse_animation};

fn angle(a: [f32; 4], b: [f32; 4]) -> f32 {
    let d = (a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3]).abs().min(1.0);
    2.0 * d.acos().to_degrees()
}

/// Rotation of each bone (by hash) at `frame`.
fn rotations(a: &Animation, frame: f32) -> Vec<(u32, [f32; 4])> {
    a.tracks.iter().filter(|t| matches!(t.channel, Channel::Rotation(_))).filter_map(|t| t.rotation_at(frame).map(|q| (t.bone_hash, q))).collect()
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    anyhow::ensure!(args.len() >= 5, "usage: clipseam <forge file> <data file> <from clip> <to clip>...");
    let f = forge::Forge::open(&args[1])?;
    let e = f.entries.iter().find(|e| e.name == args[2]).with_context(|| format!("no data file {}", args[2]))?;
    let data = f.read(e)?;
    let objs = forge::parse_objects(&data)?;
    let clip = |name: &str| objs.iter().find(|o| o.class == CLASS_ANIMATION && o.name == name).and_then(|o| parse_animation(o.bytes).ok());
    let from = clip(&args[3]).with_context(|| format!("no clip {}", args[3]))?;
    let end = rotations(&from, from.duration * forge::anim::FPS);
    let mut rows = vec![];
    for name in &args[4..] {
        let Some(to) = clip(name) else {
            println!("{name}: missing");
            continue;
        };
        let start = rotations(&to, 0.0);
        let d: Vec<f32> = end.iter().filter_map(|(b, q)| start.iter().find(|(c, _)| c == b).map(|(_, r)| angle(*q, *r))).collect();
        let mean = d.iter().sum::<f32>() / d.len().max(1) as f32;
        rows.push((mean, d.iter().copied().fold(0.0, f32::max), name));
    }
    rows.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (mean, max, name) in rows {
        println!("{mean:6.1} mean {max:6.1} max  {name}");
    }
    Ok(())
}
