//! AC1 animation playback: a lazily-parsed clip library over the "Game Fix" data file, clip
//! sampling onto the rig, and a locomotion state machine (idle / walk / jog / run half-cycles).

use anyhow::{Context, Result};
use bevy::prelude::*;
use forge::anim::{Animation, CLASS_ANIMATION, FPS, parse_animation};
use ik::{Pose, Rig};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

pub struct Clip {
    pub name: String,
    pub anim: Animation,
    /// (track index, rig bone) for tracks that drive a bone of this rig.
    bind: Vec<(usize, usize)>,
    /// Root-motion speed, m/s.
    pub speed: f32,
}

impl Clip {
    pub fn frames(&self) -> f32 {
        self.anim.frames()
    }
}

/// All animations of one data file, parsed on first use and bound per rig (the player's is rig 0;
/// other characters add theirs with `add_rig`).
#[derive(Resource)]
pub struct AnimLib {
    data: Vec<u8>,
    index: HashMap<String, (usize, usize)>,
    pub names: Vec<String>,
    cache: HashMap<(usize, String), Option<Arc<Clip>>>,
    /// Per rig: bone name hash per bone, and the Reference bone.
    rigs: Vec<(Vec<u32>, Option<usize>)>,
    /// AC1's clip sets by name: (base clip, replacement) names (see `forge::animset`).
    sets: HashMap<String, Vec<(String, String)>>,
    /// Per rig: the clips it plays in place of shared ones (from its clip set).
    replace: HashMap<usize, HashMap<String, String>>,
    /// AC1's move graph over these clips (the data file's action blocks, see `forge::graph`).
    pub graph: forge::graph::MoveGraph,
}

impl AnimLib {
    pub fn load(game_dir: &Path, forge_file: &str, datafile: &str, rig: &Rig, rig_hashes: Vec<u32>) -> Result<Self> {
        let forge = forge::Forge::open(game_dir.join(forge_file))?;
        let e = forge.entries.iter().find(|e| e.name == datafile).with_context(|| format!("no data file {datafile}"))?;
        let data = forge.read(e)?;
        let mut index = HashMap::new();
        let objs = forge::parse_objects(&data)?;
        let mut by_id = HashMap::new();
        for o in objs.iter().filter(|o| o.class == CLASS_ANIMATION) {
            let off = o.bytes.as_ptr() as usize - data.as_ptr() as usize;
            index.insert(o.name.clone(), (off, o.bytes.len()));
            by_id.insert(o.id, o.name.clone());
        }
        let sets: HashMap<String, Vec<(String, String)>> = objs
            .iter()
            .filter(|o| o.class == forge::animset::CLASS_ANIM_SET || o.class == forge::animset::CLASS_MILITARY_ANIM_SET)
            .map(|o| {
                let pairs =
                    forge::animset::anim_pairs(o.body).into_iter().filter_map(|(a, b)| Some((by_id.get(&a)?.clone(), by_id.get(&b)?.clone()))).collect();
                (o.name.clone(), pairs)
            })
            .collect();
        let mut names: Vec<String> = index.keys().cloned().collect();
        names.sort();
        info!("animation library {datafile}: {} clips", names.len());
        info!("clip sets: {}", sets.len());
        let blocks: Vec<(&str, forge::action::ActionBlock)> = objs
            .iter()
            .filter(|o| o.class == forge::action::CLASS_ACTION_BLOCK)
            .filter_map(|o| forge::action::parse_block(o.body).ok().map(|b| (o.name.as_str(), b)))
            .collect();
        let graph = forge::graph::MoveGraph::new(blocks.iter().map(|(n, b)| (*n, b)), |id| by_id.get(&id).cloned());
        info!("move graph: {} actions in {} blocks", graph.len(), blocks.len());
        Ok(Self { data, index, names, cache: HashMap::new(), rigs: vec![(rig_hashes, rig.find("Reference"))], sets, replace: HashMap::new(), graph })
    }

    /// Rig `rig` plays clip set `set`'s clips in place of the shared ones (a soldier's `mmaa_anim_set`).
    pub fn use_set(&mut self, rig: usize, set: &str) {
        let Some(pairs) = self.sets.get(set) else {
            warn!("no clip set {set}");
            return;
        };
        let map = self.replace.entry(rig).or_default();
        for (a, b) in pairs {
            map.insert(a.clone(), b.clone());
        }
        info!("rig {rig}: clip set {set}, {} replacements", pairs.len());
    }

    /// Bind clips to another rig too; returns its id for `get_for`.
    pub fn add_rig(&mut self, rig: &Rig, hashes: Vec<u32>) -> usize {
        self.rigs.push((hashes, rig.find("Reference")));
        self.rigs.len() - 1
    }

    /// A clip bound to the player's rig.
    pub fn get(&mut self, name: &str) -> Option<Arc<Clip>> {
        self.get_for(0, name)
    }

    /// A clip bound to rig `rig`.
    pub fn get_for(&mut self, rig: usize, name: &str) -> Option<Arc<Clip>> {
        let key = (rig, name.to_string());
        if let Some(c) = self.cache.get(&key) {
            return c.clone();
        }
        // A blend of variants (see `mix_name`): each part, mixed by normalized time.
        if let Some(spec) = name.strip_prefix(MIX) {
            let parts: Vec<(Arc<Clip>, f32)> =
                spec.split(',').filter_map(|p| p.rsplit_once('=')).filter_map(|(n, w)| Some((self.get_for(rig, n)?, w.parse().ok()?))).collect();
            let anims: Vec<(&Animation, f32)> = parts.iter().map(|(c, w)| (&c.anim, *w)).collect();
            debug!("animation: mixing {spec}");
            let clip = forge::anim::mix(&anims).map(|anim| {
                // Named after its heaviest part, so name tests still see what it is.
                let lead = parts.iter().max_by(|a, b| a.1.total_cmp(&b.1)).map_or(name, |p| p.0.name.as_str());
                Arc::new(self.bind(rig, lead, anim))
            });
            self.cache.insert(key, clip.clone());
            return clip;
        }
        // (A replacement from the rig's clip set plays under the shared clip's name.)
        let source = self.replace.get(&rig).and_then(|m| m.get(name)).filter(|r| self.index.contains_key(*r)).map_or(name, |r| r.as_str());
        let clip = self.index.get(source).and_then(|&(off, len)| match parse_animation(&self.data[off..off + len]) {
            Ok(anim) => Some(Arc::new(self.bind(rig, name, anim))),
            Err(e) => {
                warn!("animation {name}: {e:#}");
                None
            }
        });
        self.cache.insert(key, clip.clone());
        clip
    }

    /// A parsed clip bound to rig `rig`.
    fn bind(&self, rig: usize, name: &str, anim: Animation) -> Clip {
        let rig_hashes = &self.rigs[rig].0;
        let bind = anim.tracks.iter().enumerate().filter_map(|(t, tr)| rig_hashes.iter().position(|&h| h == tr.bone_hash && h != 0).map(|b| (t, b))).collect();
        let rm = anim.root_motion();
        let speed = (rm[0] * rm[0] + rm[1] * rm[1]).sqrt() / anim.duration.max(1e-3);
        Clip { name: name.to_string(), anim, bind, speed }
    }

    pub fn reference_for(&self, rig: usize) -> Option<usize> {
        self.rigs.get(rig).and_then(|r| r.1)
    }
}

/// Prefix of a mixed clip's name: `mix:<clip>=<weight>,<clip>=<weight>...`.
const MIX: &str = "mix:";

/// The name `AnimLib::get` resolves to a mix of `parts` (AC1's blend of a move's variants, see
/// `forge::anim::mix`). Weights are rounded to hundredths, so nearby mixes share one baked clip; a part
/// with all the weight is just that clip.
pub fn mix_name(parts: &[(&str, f32)]) -> String {
    let total: f32 = parts.iter().map(|p| p.1.max(0.0)).sum::<f32>().max(1e-6);
    let parts: Vec<(&str, f32)> = parts.iter().map(|&(n, w)| (n, (w.max(0.0) / total * 100.0).round() / 100.0)).filter(|p| p.1 > 0.0).collect();
    match parts.as_slice() {
        [] => String::new(),
        [(n, _)] => n.to_string(),
        _ => format!("{MIX}{}", parts.iter().map(|(n, w)| format!("{n}={w:.2}")).collect::<Vec<_>>().join(",")),
    }
}

/// Write a clip's pose at `frame` into `pose` (bones without tracks keep their values).
/// AC1 clips face +Y with the Reference bone turned 90 degrees; turn them back to face +X.
pub fn sample(clip: &Clip, frame: f32, pose: &mut Pose, reference: Option<usize>) {
    let fix = Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2);
    for &(t, bone) in &clip.bind {
        let track = &clip.anim.tracks[t];
        let is_ref = Some(bone) == reference;
        if let Some([x, y, z, w]) = track.rotation_at(frame) {
            let q = Quat::from_xyzw(x, y, z, w);
            pose.local[bone].rot = if is_ref { fix * q } else { q };
        } else if let Some(p) = track.translation_at(frame) {
            let p = Vec3::from(p);
            pose.local[bone].pos = if is_ref { fix * p } else { p };
        }
    }
}

/// Root motion (hash 0 translation) at `frame`, in model space (+X forward, Z up).
pub fn root_motion_at(clip: &Clip, frame: f32) -> Vec3 {
    let fix = Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2);
    clip.anim.tracks.iter().find(|t| t.bone_hash == 0).and_then(|t| t.translation_at(frame)).map_or(Vec3::ZERO, |p| fix * Vec3::from(p))
}

/// Root-motion rotation (hash 0 rotation) at `frame`, in model space.
pub fn root_rotation_at(clip: &Clip, frame: f32) -> Quat {
    let fix = Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2);
    clip.anim
        .tracks
        .iter()
        .find(|t| t.bone_hash == 0 && matches!(t.channel, forge::anim::Channel::Rotation(_)))
        .and_then(|t| t.rotation_at(frame))
        .map_or(Quat::IDENTITY, |[x, y, z, w]| fix * Quat::from_xyzw(x, y, z, w) * fix.inverse())
}

/// AC1's locomotion blend space (`HumanGround`'s walk/jog/run/sprint item): each gait's straight half-cycle
/// clip and the ones banking into a left and a right turn, by `{foot}` (`footl`, `footr`). Neighbouring gaits
/// are mixed by speed, the banked ones by turn rate.
const GAITS: [[&str; 3]; 5] = [
    ["xx_l_walk_slow_hipm_{foot}", "xx_l_walk_slow_hipl_{foot}", "xx_l_walk_slow_hipr_{foot}"],
    ["xx_l_walk_hipm_{foot}", "xx_l_walk_hipl_{foot}", "xx_l_walk_hipr_{foot}"],
    ["xx_h_jog_hipm_{foot}", "xx_h_jog_bank_left_{foot}", "xx_h_jog_bank_right_{foot}"],
    ["xx_h_run_hipm_{foot}", "xx_h_run_bank_left_{foot}", "xx_h_run_bank_right_{foot}"],
    ["xx_h_sprint_hipm_{foot}", "xx_h_run_bank_left_{foot}", "xx_h_run_bank_right_{foot}"],
];
/// Turn rate (rad/s) at which a gait is fully banked.
const FULL_BANK: f32 = 2.5;

/// One gait: per foot, its straight clip and (if any) the left- and right-banking ones.
struct Gait {
    clips: [[Option<Arc<Clip>>; 3]; 2],
    speed: f32,
    duration: f32,
}
const IDLE: &str = "xx_l_wait_hipm_footl";

/// Locomotion: alternating left/right half-cycles of the gait closest to the current speed,
/// played at a rate that matches the clip's root motion to the character's speed.
pub struct Animator {
    idle: Arc<Clip>,
    /// AC1's stand loops by profile (low, high) and by the foot ahead (`xx_<l|h>_wait_hipm_<footl|footr>`);
    /// `stand` is the foot standing ahead now, `high` the profile.
    stands: [[Option<Arc<Clip>>; 2]; 2],
    stand: usize,
    pub high: bool,
    /// Moving last frame (a start begins its gait on the foot it stood on).
    was_moving: bool,
    gaits: Vec<Gait>,
    /// The two gaits mixed now (lower, upper) and the upper one's weight.
    mix: (usize, usize, f32),
    /// Banking: -1 (full right) to 1 (full left), smoothed; and the last root yaw seen.
    bank: f32,
    last_yaw: Option<f32>,
    side: usize,
    phase: f32,
    idle_time: f32,
    move_w: f32,
    /// Debug: play one clip in a loop instead.
    pub forced: Option<(Arc<Clip>, f32)>,
    /// Standing at an edge: looking down over it instead of the idle (`xx_l_ledge_lookdown_front_*`).
    pub idle_alt: Option<Arc<Clip>>,
    /// A legs-only clip played once under the rest (a fighter's 45-degree foot turn), and the time into it.
    pub lower: Option<(Arc<Clip>, f32)>,
    /// An upper-body clip looped over the locomotion (the praying hands of the blend walk), and its time.
    pub layer: Option<(Arc<Clip>, f32)>,
    /// A clip played once over the locomotion (a gentle push), and the time into it.
    pub oneshot: Option<(Arc<Clip>, f32)>,
    /// Idle variations (`xx_l_idle_onspot_footl_v01..`), one played now and then while standing, the time
    /// stood still, the one playing and the next to play.
    breaks: Vec<Arc<Clip>>,
    still: f32,
    brk: Option<(Arc<Clip>, f32)>,
    next_break: usize,
    reference: Option<usize>,
}

/// Points tried per gait step when matching a pose to pick up from (`Animator::resume_matching`).
const RESUME_SAMPLES: usize = 24;

/// Standing still this long (s) shifts into an idle variation.
const IDLE_BREAK: f32 = 7.0;

impl Animator {
    pub fn new(lib: &mut AnimLib) -> Option<Self> {
        Self::new_for(lib, 0)
    }

    /// Locomotion for rig `rig` (see `AnimLib::add_rig`).
    pub fn new_for(lib: &mut AnimLib, rig: usize) -> Option<Self> {
        let idle = lib.get_for(rig, IDLE)?;
        let mut gaits: Vec<Gait> = GAITS
            .iter()
            .filter_map(|names| {
                let clips = ["footl", "footr"].map(|f| names.map(|n| lib.get_for(rig, &n.replace("{foot}", f))));
                let straight = clips[0][0].clone()?;
                clips[1][0].as_ref()?;
                Some(Gait { speed: straight.speed, duration: straight.anim.duration, clips })
            })
            .collect();
        if gaits.is_empty() {
            return None;
        }
        // A gait authored in place (the slow walk barely moves its root): its speed from the next gait's
        // stride over its own cycle time.
        for i in 0..gaits.len().saturating_sub(1) {
            if gaits[i].speed < 0.3 {
                let next = &gaits[i + 1];
                gaits[i].speed = next.speed * next.duration / gaits[i].duration.max(1e-3);
            }
        }
        gaits.sort_by(|a, b| a.speed.total_cmp(&b.speed));
        for g in &gaits {
            let n = |c: &Option<Arc<Clip>>| c.as_ref().map_or("-".to_string(), |c| c.name.clone());
            info!("gait {} (banks {} / {}): {:.2} m/s, {:.2} s", n(&g.clips[0][0]), n(&g.clips[0][1]), n(&g.clips[0][2]), g.speed, g.duration);
        }
        let stands = ["l", "h"].map(|p| ["footl", "footr"].map(|f| lib.get_for(rig, &format!("xx_{p}_wait_hipm_{f}"))));
        Some(Self {
            idle,
            stands,
            stand: 0,
            high: false,
            was_moving: false,
            gaits,
            mix: (0, 0, 0.0),
            bank: 0.0,
            last_yaw: None,
            side: 0,
            phase: 0.0,
            idle_time: 0.0,
            move_w: 0.0,
            forced: None,
            oneshot: None,
            layer: None,
            lower: None,
            idle_alt: None,
            breaks: (1..=6).filter_map(|k| lib.get_for(rig, &format!("xx_l_idle_onspot_footl_v{k:02}"))).collect(),
            still: 0.0,
            brk: None,
            next_break: rig,
            reference: lib.reference_for(rig),
        })
    }

    /// Move time on by `dt` at ground `speed` (m/s), the root facing `yaw` (radians about +Y; its rate of
    /// change banks the gait).
    pub fn advance(&mut self, dt: f32, speed: f32, yaw: f32) {
        let rate = self.last_yaw.map_or(0.0, |l| {
            let mut d = yaw - l;
            if d > std::f32::consts::PI {
                d -= std::f32::consts::TAU;
            } else if d < -std::f32::consts::PI {
                d += std::f32::consts::TAU;
            }
            d / dt.max(1e-4)
        });
        self.last_yaw = Some(yaw);
        let want = if speed > 0.15 { (rate / FULL_BANK).clamp(-1.0, 1.0) } else { 0.0 };
        self.bank += (want - self.bank) * (dt * 6.0).min(1.0);
        if let Some((clip, t)) = &mut self.oneshot {
            *t += dt;
            if *t >= clip.anim.duration {
                self.oneshot = None;
            }
        }
        if let Some((clip, t)) = &mut self.lower {
            *t += dt;
            if *t >= clip.anim.duration {
                self.lower = None;
            }
        }
        if let Some((clip, t)) = &mut self.layer {
            *t = (*t + dt) % clip.anim.duration.max(1e-3);
        }
        if let Some((clip, t)) = &mut self.forced {
            *t = (*t + dt) % clip.anim.duration.max(1e-3);
            return;
        }
        let moving = speed > 0.15;
        // Starting off: the gait from the top of the half-cycle of the foot it stood on (AC1's stands and gaits
        // are named by that foot: `wait_hipm_footl` goes on into `walk_hipm_footl`). Stopping: it stands on the
        // other foot (the half-cycle's foot steps through, as AC1's stops `runstop_footl_tr_h_wait_hipm_footr`).
        if moving && !self.was_moving && self.move_w < 0.3 {
            self.side = self.stand;
            self.phase = 0.0;
        } else if !moving && self.was_moving {
            self.stand = self.side ^ 1;
        }
        self.was_moving = moving;
        let idle_len = self.stand_clip().anim.duration.max(1e-3);
        self.idle_time = (self.idle_time + dt) % idle_len;
        // Now and then, standing still, an idle variation.
        if moving || self.idle_alt.is_some() || self.oneshot.is_some() {
            self.still = 0.0;
            self.brk = None;
        } else {
            self.still += dt;
            if let Some((c, t)) = &mut self.brk {
                *t += dt;
                if *t >= c.anim.duration {
                    self.brk = None;
                }
            } else if self.still > IDLE_BREAK && !self.breaks.is_empty() {
                self.brk = Some((self.breaks[self.next_break % self.breaks.len()].clone(), 0.0));
                self.next_break += 1;
                self.still = 0.0;
            }
        }
        self.move_w = (self.move_w + if moving { dt * 5.0 } else { -dt * 3.0 }).clamp(0.0, 1.0);

        // The two gaits around this speed, mixed by where it falls between their natural speeds; played
        // at the mix's natural speed's rate (beyond the slowest or fastest, faster or slower).
        let n = self.gaits.len();
        let upper = self.gaits.iter().position(|g| g.speed >= speed).unwrap_or(n - 1);
        let lower = upper.saturating_sub(1);
        let (sl, su) = (self.gaits[lower].speed, self.gaits[upper].speed);
        let w = if upper == lower || su <= sl { 0.0 } else { ((speed - sl) / (su - sl)).clamp(0.0, 1.0) };
        if moving {
            self.mix = (lower, upper, w);
        }
        let (l, u, w) = self.mix;
        let natural = self.gaits[l].speed + (self.gaits[u].speed - self.gaits[l].speed) * w;
        let duration = self.gaits[l].duration + (self.gaits[u].duration - self.gaits[l].duration) * w;
        let rate = if moving { (speed / natural.max(0.1)).clamp(0.6, 1.6) } else { 1.0 };
        self.phase += dt * rate / duration.max(1e-3);
        if self.phase >= 1.0 {
            self.phase -= 1.0;
            self.side ^= 1;
        }
    }

    /// Current pose, starting from `base` for bones no clip drives.
    pub fn pose(&self, base: &Pose) -> Pose {
        let mut pose = base.clone();
        if let Some((clip, t)) = &self.forced {
            sample(clip, t * FPS, &mut pose, self.reference);
            return pose;
        }
        sample(self.idle_alt.as_ref().unwrap_or(self.stand_clip()), self.idle_time * FPS, &mut pose, self.reference);
        if let Some((c, t)) = &self.brk {
            let mut brk = base.clone();
            sample(c, t * FPS, &mut brk, self.reference);
            let w = (t / 0.4).min((c.anim.duration - t) / 0.4).min(1.0);
            pose.blend(&brk, smooth(w));
        }
        if self.move_w > 0.0 {
            let mut walk = base.clone();
            // One gait at the shared phase, banked into the turn.
            let gait_pose = |g: usize, out: &mut Pose| {
                let clips = &self.gaits[g].clips[self.side];
                let at = |c: &Arc<Clip>, out: &mut Pose| sample(c, self.phase * c.frames(), out, self.reference);
                if let Some(c) = &clips[0] {
                    at(c, out);
                }
                let banked = if self.bank > 0.0 { &clips[1] } else { &clips[2] };
                if let Some(c) = banked.as_ref().filter(|_| self.bank.abs() > 0.02) {
                    let mut b = base.clone();
                    at(c, &mut b);
                    out.blend(&b, self.bank.abs());
                }
            };
            let (l, u, w) = self.mix;
            gait_pose(l, &mut walk);
            if u != l && w > 0.0 {
                let mut up = base.clone();
                gait_pose(u, &mut up);
                walk.blend(&up, w);
            }
            pose.blend(&walk, smooth(self.move_w));
        }
        pose
    }
}

impl Animator {
    /// The one-shot clip's pose (a push: upper body only, layered over the walk by the caller) and its
    /// weight, faded in and out.
    /// The left foot leads the current walk half-cycle.
    pub fn lead_left(&self) -> bool {
        self.side == 0
    }

    /// The stand loop now: by profile and the foot ahead (the plain idle if the rig lacks it).
    fn stand_clip(&self) -> &Arc<Clip> {
        self.stands[self.high as usize][self.stand].as_ref().or(self.stands[0][self.stand].as_ref()).unwrap_or(&self.idle)
    }

    /// Pick up after a clip chain (a stop, a turn, a landing) from where its last clip `name` ends: standing on
    /// the foot it names (`..._wait_hipm_footr`, `..._h_wait_footr`), or on into the gait half-cycle it names
    /// (`..._jog_hipm_footl`), from its start.
    pub fn resume(&mut self, name: &str) {
        // (The last foot named wins: `runstop_footl_tr_h_wait_hipm_footr` stands on the right.)
        let foot = match (name.rfind("footl"), name.rfind("footr")) {
            (Some(l), Some(r)) => Some(if l > r { 0 } else { 1 }),
            (Some(_), None) => Some(0),
            (None, Some(_)) => Some(1),
            (None, None) => None,
        };
        let Some(foot) = foot else { return };
        let into_gait = ["_walk_hipm_", "_jog_hipm_", "_run_hipm_", "_sprint_hipm_", "_walk_slow_hipm_"].iter().any(|g| name.contains(g));
        if into_gait {
            self.side = foot;
            self.phase = 0.0;
            self.was_moving = true;
        } else if name.contains("wait") {
            self.stand = foot;
            self.idle_time = 0.0;
            self.was_moving = false;
        }
    }

    /// `resume`, and going into a gait, start on the step (left or right clip) and at the point in it whose leg bones
    /// (`legs`) are nearest `pose`: a clip's exit names the gait it hands over to, not where in the stride it does,
    /// and starting that step from its first frame repeated the foot just stepped on and popped the legs.
    /// Returns the gait's natural speed when it goes into one: the character carries on at it (stopping for a frame
    /// would start the gait over from the standing foot, fading in from the stand).
    pub fn resume_matching(&mut self, name: &str, pose: &Pose, base: &Pose, legs: &[usize]) -> Option<f32> {
        self.resume(name);
        let gait = ["_walk_hipm_", "_jog_hipm_", "_run_hipm_", "_sprint_hipm_", "_walk_slow_hipm_"].iter().find(|g| name.contains(*g))?;
        // The gait named (low or high profile by the prefix right before it).
        let at = name.rfind(gait).unwrap_or(0);
        let prefix = &name[at.saturating_sub(1)..at];
        let g = self.gaits.iter().position(|g| g.clips[0][0].as_ref().is_some_and(|c| c.name.contains(&format!("{prefix}{gait}"))))?;
        let mut best = (f32::MAX, self.side, self.phase);
        let mut s = base.clone();
        for side in 0..2 {
            let Some(clip) = self.gaits[g].clips[side][0].clone() else { continue };
            for k in 0..RESUME_SAMPLES {
                let phase = k as f32 / RESUME_SAMPLES as f32;
                sample(&clip, phase * clip.frames(), &mut s, self.reference);
                let cost: f32 =
                    legs.iter().filter(|&&b| b < s.local.len() && b < pose.local.len()).map(|&b| s.local[b].rot.angle_between(pose.local[b].rot)).sum();
                if cost < best.0 {
                    best = (cost, side, phase);
                }
            }
        }
        debug!("resume {name}: step {} at {:.2} (legs off by {:.2} rad)", ["footl", "footr"][best.1], best.2, best.0);
        self.side = best.1;
        self.phase = best.2;
        self.was_moving = true;
        self.move_w = 1.0;
        self.mix = (g, g, 0.0);
        Some(self.gaits[g].speed)
    }

    /// Into the gait nearest `speed` (out of a turn on the spot). AC1's move graph goes from a turn on `foot` into the
    /// gaits' `_hipm_<foot>` step, from its start: so when `foot` is given; else the step and the point in it whose
    /// legs (`legs`) match `pose`. Returns that gait's natural speed.
    pub fn resume_gait(&mut self, speed: f32, foot: Option<usize>, pose: &Pose, base: &Pose, legs: &[usize]) -> f32 {
        let g = (0..self.gaits.len()).min_by(|&a, &b| (self.gaits[a].speed - speed).abs().total_cmp(&(self.gaits[b].speed - speed).abs())).unwrap_or(0);
        if let Some(foot) = foot {
            debug!("resume into gait {g}: step {} from its start (as AC1's move graph goes)", ["footl", "footr"][foot]);
            self.side = foot;
            self.phase = 0.0;
            self.was_moving = true;
            self.move_w = 1.0;
            self.mix = (g, g, 0.0);
            return self.gaits[g].speed;
        }
        let mut best = (f32::MAX, self.side, 0.0);
        let mut s = base.clone();
        for side in 0..2 {
            let Some(clip) = self.gaits[g].clips[side][0].clone() else { continue };
            for k in 0..RESUME_SAMPLES {
                let phase = k as f32 / RESUME_SAMPLES as f32;
                sample(&clip, phase * clip.frames(), &mut s, self.reference);
                let cost: f32 =
                    legs.iter().filter(|&&b| b < s.local.len() && b < pose.local.len()).map(|&b| s.local[b].rot.angle_between(pose.local[b].rot)).sum();
                if cost < best.0 {
                    best = (cost, side, phase);
                }
            }
        }
        debug!("resume into gait {g}: step {} at {:.2} (legs off by {:.2} rad)", ["footl", "footr"][best.1], best.2, best.0);
        self.side = best.1;
        self.phase = best.2;
        self.was_moving = true;
        self.move_w = 1.0;
        self.mix = (g, g, 0.0);
        self.gaits[g].speed
    }

    /// The legs-only one-shot's pose and weight (faded in and out).
    pub fn lower_pose(&self, base: &Pose) -> Option<(Pose, f32)> {
        let (clip, t) = self.lower.as_ref()?;
        let mut pose = base.clone();
        sample(clip, t * FPS, &mut pose, self.reference);
        let w = (t / 0.1).min((clip.anim.duration - t) / 0.15).min(1.0);
        Some((pose, smooth(w)))
    }

    /// The looped upper-body layer's pose.
    pub fn layer_pose(&self, base: &Pose) -> Option<Pose> {
        let (clip, t) = self.layer.as_ref()?;
        let mut pose = base.clone();
        sample(clip, t * FPS, &mut pose, self.reference);
        Some(pose)
    }

    pub fn oneshot_pose(&self, base: &Pose) -> Option<(Pose, f32)> {
        let (clip, t) = self.oneshot.as_ref()?;
        let mut pose = base.clone();
        sample(clip, t * FPS, &mut pose, self.reference);
        let w = (t / 0.15).min((clip.anim.duration - t) / 0.2).min(1.0);
        Some((pose, smooth(w)))
    }
}

fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
