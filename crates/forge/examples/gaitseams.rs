//! gaitseams <forge file> <data file> <clip without foot>...: how well a gait's half-cycles join: for `<name>_footl`
//! and `<name>_footr`, the largest rotation difference (degrees) over the bones between the end of one and the start
//! of the other, both ways, and the bone (name hash) where it is; and the same for translations (m). A cycle that
//! joins cleanly is near 0.
use anyhow::{Context, Result};
use forge::anim::{Animation, CLASS_ANIMATION, Channel, parse_animation};

fn angle(a: [f32; 4], b: [f32; 4]) -> f32 {
    let d = (a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3]).abs().min(1.0);
    2.0 * d.acos().to_degrees()
}

/// The largest rotation difference over the bones both clips drive (not the reference bone), and its bone.
fn seam(end: &Animation, start: &Animation) -> (f32, u32) {
    let mut worst = (0.0f32, 0u32);
    for t in end.tracks.iter().filter(|t| t.bone_hash != 0 && matches!(t.channel, Channel::Rotation(_))) {
        let Some(u) = start.tracks.iter().find(|u| u.bone_hash == t.bone_hash && matches!(u.channel, Channel::Rotation(_))) else { continue };
        let (Some(a), Some(b)) = (t.rotation_at(f32::MAX), u.rotation_at(0.0)) else { continue };
        let d = angle(a, b);
        if d > worst.0 {
            worst = (d, t.bone_hash);
        }
    }
    worst
}

/// The largest translation difference (m) over the bones but the reference, between the end of one and the start of
/// the other, and its bone.
fn shift(end: &Animation, start: &Animation) -> (f32, u32) {
    let mut worst = (0.0f32, 0u32);
    for t in end.tracks.iter().filter(|t| t.bone_hash != 0 && matches!(t.channel, Channel::Translation(_))) {
        let Some(u) = start.tracks.iter().find(|u| u.bone_hash == t.bone_hash && matches!(u.channel, Channel::Translation(_))) else { continue };
        let (Some(a), Some(b)) = (t.translation_at(f32::MAX), u.translation_at(0.0)) else { continue };
        let d = ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
        if d > worst.0 {
            worst = (d, t.bone_hash);
        }
    }
    worst
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    anyhow::ensure!(args.len() >= 4, "usage: gaitseams <forge file> <data file> <clip without foot>...");
    let f = forge::Forge::open(&args[1])?;
    let e = f.entries.iter().find(|e| e.name == args[2]).with_context(|| format!("no data file {}", args[2]))?;
    let data = f.read(e)?;
    let objs = forge::parse_objects(&data)?;
    let clip =
        |name: &str| -> Option<Animation> { objs.iter().find(|o| o.class == CLASS_ANIMATION && o.name == name).and_then(|o| parse_animation(o.bytes).ok()) };
    for g in &args[3..] {
        let (Some(l), Some(r)) = (clip(&format!("{g}_footl")), clip(&format!("{g}_footr"))) else {
            println!("{g:<40} missing");
            continue;
        };
        let (lr, lb) = seam(&l, &r);
        let (rl, rb) = seam(&r, &l);
        println!("{g:<40} footl end -> footr start {lr:6.1} deg (bone {lb:08x})   footr end -> footl start {rl:6.1} deg (bone {rb:08x})");
        let (tl, tlb) = shift(&l, &r);
        let (tr, trb) = shift(&r, &l);
        println!("{:<40} translations: {:6.3} m (bone {tlb:08x})   {:6.3} m (bone {trb:08x})", "", tl, tr);
    }
    Ok(())
}
