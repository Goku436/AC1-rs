//! The live log's pure half (no Windows calls, unit-tested on any target): where the player's data sits in AC1's
//! memory, read through [`Mem`]; the frame line in the flight recorder's format; the event lines; the files' rotation;
//! the action names table.
//!
//! Layouts (v1.02 DX9, base 0x400000; found with Ghidra on the exe, some first named by Banned445's MIT
//! AC1-Movement-Rewritten; ours in plain words, no code from the exe):
//! - bone handles in the `Human` (`BONE_HANDLES`): 8 bytes each, a pointer to a reference block (`[0]` the object,
//!   `[8]` flags, bit 0x10000000 = alive) to the `SkeletonComponent`, then the bone's index (byte, 255 none);
//! - `SkeletonComponent` +0x174 → its skeleton; skeleton +0x24 / +0x28 the bones by name hash (8-byte entries, hash then
//!   index, the count in the low 14 bits); +0x184 its global pose (as last rebuilt: the game rebuilds it when a bone is
//!   read after the entity moved, the log does not);
//! - global pose +0x58 its storage (0 floats, 2 int16 / 2048 m from the origin at +0, otherwise floats from that
//!   origin, the format picking a column), +0x5C its layout (+4 the u16 offsets, 12 bytes a bone; +0x34 a bone's slot,
//!   i16; +0x54 the slot's row, u16), +0x60 the data;
//! - limb IK at `Human` +0x530, four limbs (left hand, right hand, left foot, right foot) of 0x90 bytes: +2 on (byte),
//!   +4 weight, +8 settled on its hold (byte; else moving to a new one), the hold: +0x30 the point in its object's
//!   space, +0x50 the object (reference block), +0x54 its kind (2: on a bone, not read here); the next hold +0x60 / +0x80
//!   / +0x84; an object's world matrix at +0x10 (rows X, Y, Z, position), unless +0x50 says it is elsewhere;
//! - the input interpreter: +0xD0 the pad (16 buttons as bytes at +0x18, the previous frame's at +0x8; the sticks as
//!   4 floats from +0x100), +0x1126 the legs buffer, +0x1129 the empty-hand buffer, +0x112B high profile, +0x1300 the
//!   stick it latched;
//! - the camera: the `CameraManager` at [0x1A11998], +0xC → (reference block) its switcher, +0x74 the camera now, its
//!   world matrix at +0 (rows X, Y, Z, position; it looks along Y);
//! - the frame's length (s) at 0x192DDA8;
//! - the contexts' data in one block at [`Human` +0x30]: ground +0x40 (+0xB0 sub-state, +0xB4 speed 0..1), climb +0xDA0
//!   (+0x20 sub-state).

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};

/// The `Human`'s bone handles (offset, recorder name). Hips has none: found in the skeleton by its hash.
pub const BONE_HANDLES: [(u32, &str); 5] = [(0x280, "Head"), (0x290, "LeftHand"), (0x298, "RightHand"), (0x2A0, "LeftFoot"), (0x2A8, "RightFoot")];
/// CRC32("Hips").
pub const HIPS_HASH: u32 = 0xDED1_0611;
pub const HUMAN_LIMB_IK: u32 = 0x530;
pub const HUMAN_DATA: u32 = 0x30;
pub const CAMERA_MANAGER: u32 = 0x01A1_1998;
pub const FRAME_DT: u32 = 0x0192_DDA8;
const ALIVE: u32 = 0x1000_0000;

/// Reads of the game's memory (32-bit addresses). `bytes` fails (false) when the range is not readable.
pub trait Mem {
    fn bytes(&self, addr: u32, out: &mut [u8]) -> bool;
    fn u32(&self, addr: u32) -> Option<u32> {
        let mut b = [0; 4];
        self.bytes(addr, &mut b).then(|| u32::from_le_bytes(b))
    }
    fn u16(&self, addr: u32) -> Option<u16> {
        let mut b = [0; 2];
        self.bytes(addr, &mut b).then(|| u16::from_le_bytes(b))
    }
    fn u8(&self, addr: u32) -> Option<u8> {
        let mut b = [0; 1];
        self.bytes(addr, &mut b).then_some(b[0])
    }
    fn f32(&self, addr: u32) -> Option<f32> {
        self.u32(addr).map(f32::from_bits)
    }
    fn vec3(&self, addr: u32) -> Option<[f32; 3]> {
        let mut b = [0; 12];
        if !self.bytes(addr, &mut b) {
            return None;
        }
        let f = |k: usize| f32::from_le_bytes([b[4 * k], b[4 * k + 1], b[4 * k + 2], b[4 * k + 3]]);
        Some([f(0), f(1), f(2)]).filter(|v| v.iter().all(|x| x.is_finite()))
    }
    /// 16 floats: a matrix, rows X, Y, Z, position.
    fn matrix(&self, addr: u32) -> Option<[f32; 16]> {
        let mut b = [0; 64];
        if !self.bytes(addr, &mut b) {
            return None;
        }
        let m: [f32; 16] = std::array::from_fn(|k| f32::from_le_bytes([b[4 * k], b[4 * k + 1], b[4 * k + 2], b[4 * k + 3]]));
        m.iter().all(|x| x.is_finite()).then_some(m)
    }
}

/// The object a reference block points to, while alive.
pub fn deref(m: &impl Mem, block: u32) -> Option<u32> {
    if block == 0 || m.u32(block + 8)? & ALIVE == 0 {
        return None;
    }
    m.u32(block).filter(|&o| o != 0)
}

/// A bone handle at `at`: (the `SkeletonComponent`, the bone's index).
pub fn handle(m: &impl Mem, at: u32) -> Option<(u32, u8)> {
    let comp = deref(m, m.u32(at)?)?;
    let bone = m.u8(at + 4)?;
    (bone != 255).then_some((comp, bone))
}

/// A bone's index in the component's skeleton by its name hash.
pub fn bone_index(m: &impl Mem, comp: u32, hash: u32) -> Option<u8> {
    let skel = m.u32(comp + 0x174)?;
    let table = m.u32(skel + 0x24)?;
    let n = m.u32(skel + 0x28)? & 0x3fff;
    (0..n.min(512)).find(|k| m.u32(table + 8 * k) == Some(hash)).and_then(|k| m.u8(table + 8 * k + 4)).filter(|&i| i != 255)
}

/// A bone's world position (game space) in the component's global pose, as last rebuilt.
pub fn bone_world(m: &impl Mem, comp: u32, bone: u8) -> Option<[f32; 3]> {
    let pose = m.u32(comp + 0x184)?;
    let format = m.u32(pose + 0x58)?;
    let layout = m.u32(pose + 0x5C)?;
    let data = m.u32(pose + 0x60)?;
    let slots = m.u32(layout + 0x34)?;
    let rows = m.u32(layout + 0x54)?;
    let offsets = m.u32(layout + 4)?;
    let slot = m.u16(slots + 2 * bone as u32)?.wrapping_add(1);
    let row = m.u16(rows + 2 * slot as u32)? as u32;
    let origin = || m.vec3(pose);
    let p = match format {
        0 => m.vec3(data + m.u16(offsets + 4 + 12 * row)? as u32)?,
        2 => {
            let at = data + m.u16(offsets + 8 + 12 * row)? as u32;
            let s = |k: u32| m.u16(at + 2 * k).map(|v| v as i16 as f32 / 2048.0);
            let o = origin()?;
            [s(0)? + o[0], s(1)? + o[1], s(2)? + o[2]]
        }
        f if f < 6 => {
            let v = m.vec3(data + m.u16(offsets + 4 + 2 * f + 12 * row)? as u32)?;
            let o = origin()?;
            [v[0] + o[0], v[1] + o[1], v[2] + o[2]]
        }
        _ => return None,
    };
    p.iter().all(|x| x.is_finite()).then_some(p)
}

/// The recorder's six bones (game space), those found: the handles' five and Hips.
pub fn bones(m: &impl Mem, human: u32) -> Vec<(&'static str, [f32; 3])> {
    let mut out = Vec::with_capacity(6);
    if let Some((comp, _)) = handle(m, human + BONE_HANDLES[0].0)
        && let Some(p) = bone_index(m, comp, HIPS_HASH).and_then(|i| bone_world(m, comp, i))
    {
        out.push(("Hips", p));
    }
    for (at, name) in BONE_HANDLES {
        if let Some(p) = handle(m, human + at).and_then(|(comp, i)| bone_world(m, comp, i)) {
            out.push((name, p));
        }
    }
    out
}

/// `local` in the object's world matrix (+0x10), if it keeps it there.
fn in_object(m: &impl Mem, obj: u32, local: [f32; 3]) -> Option<[f32; 3]> {
    if m.u32(obj + 0x50)? != 0 {
        return None;
    }
    let r = m.matrix(obj + 0x10)?;
    Some(std::array::from_fn(|k| local[0] * r[k] + local[1] * r[4 + k] + local[2] * r[8 + k] + r[12 + k]))
}

/// One limb's IK goal (game space): its hold, or the one it is moving to. None when the limb is off.
pub fn limb_goal(m: &impl Mem, human: u32, limb: u32) -> Option<[f32; 3]> {
    let l = human + HUMAN_LIMB_IK + 0x90 * limb;
    if m.u8(l + 2)? == 0 {
        return None;
    }
    let (local, obj, kind) = if m.u8(l + 8)? != 0 { (0x30, 0x50, 0x54) } else { (0x60, 0x80, 0x84) };
    if m.u32(l + kind)? == 2 {
        return None;
    }
    in_object(m, deref(m, m.u32(l + obj)?)?, m.vec3(l + local)?)
}

/// The camera: its position and the way it looks (game space).
pub fn camera(m: &impl Mem) -> Option<([f32; 3], [f32; 3])> {
    let manager = m.u32(CAMERA_MANAGER)?;
    let switcher = deref(m, m.u32(manager + 0xC)?)?;
    let cam = m.u32(switcher + 0x74).filter(|&c| c != 0)?;
    let r = m.matrix(cam)?;
    let fwd = [r[4], r[5], r[6]];
    let len = (fwd[0] * fwd[0] + fwd[1] * fwd[1] + fwd[2] * fwd[2]).sqrt();
    ((0.9..1.1).contains(&len)).then_some(([r[12], r[13], r[14]], fwd))
}

/// The input as the interpreter sees it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Input {
    /// The pad's left stick (x right, y up) and the one the interpreter latched.
    pub stick: [f32; 2],
    pub latched: [f32; 2],
    /// The pad's 16 buttons (bit n: button n; 0 legs, 1 weapon hand, 2 head, 3 empty hand, 12 high profile).
    pub buttons: u16,
    pub high: bool,
    pub legs_buffer: bool,
    pub hand_buffer: bool,
}

pub fn input(m: &impl Mem, interp: u32) -> Option<Input> {
    let pad = m.u32(interp + 0xD0).filter(|&p| p != 0)?;
    let mut b = [0u8; 16];
    if !m.bytes(pad + 0x18, &mut b) {
        return None;
    }
    let buttons = b.iter().enumerate().fold(0u16, |acc, (k, &v)| if v != 0 { acc | 1 << k } else { acc });
    let two = |at: u32| Some([m.f32(at)?, m.f32(at + 4)?]).filter(|v| v.iter().all(|x| x.is_finite()));
    Some(Input {
        stick: two(pad + 0x100)?,
        latched: two(interp + 0x1300).unwrap_or_default(),
        buttons,
        high: m.u8(interp + 0x112B)? != 0,
        legs_buffer: m.u8(interp + 0x1126)? != 0,
        hand_buffer: m.u8(interp + 0x1129)? != 0,
    })
}

/// The context's sub-state (and the ground's speed 0..1), where known.
pub fn sub_state(m: &impl Mem, human: u32, context: &str) -> (Option<u32>, Option<f32>) {
    let Some(data) = m.u32(human + HUMAN_DATA).filter(|&d| d != 0) else { return (None, None) };
    match context {
        "HumanGround" => (m.u32(data + 0x40 + 0xB0), m.f32(data + 0x40 + 0xB4).filter(|s| s.is_finite())),
        "HumanClimb" => (m.u32(data + 0xDA0 + 0x20), None),
        _ => (None, None),
    }
}

/// Game space (Z up) to the recorder's Bevy space (Y up): game x = x, game y = -z, game z = y.
pub fn bevy(p: [f32; 3]) -> [f32; 3] {
    [p[0], p[2], -p[1]]
}

/// The recorder's yaw (degrees, Bevy's rotation about Y; ac1-rs faces -Z at 0) for a game yaw (from +X toward +Y).
pub fn bevy_yaw(game_yaw: f32) -> f32 {
    let mut y = game_yaw - 90.0;
    while y <= -180.0 {
        y += 360.0;
    }
    while y > 180.0 {
        y -= 360.0;
    }
    y
}

/// What kind of movement a context is (AC1's `Human*` class), the same words as `mode_of_ours`.
pub fn mode_of_context(context: &str) -> &'static str {
    match context {
        "HumanGround" => "ground",
        "HumanInAir" => "air",
        "HumanLedge" => "hang",
        "HumanClimb" => "climb",
        "HumanWalling" => "wallrun",
        "HumanNarrowObject" => "beam",
        "HumanLadder" => "ladder",
        "HumanHayStack" => "hay",
        "HumanPole" | "HumanRope" => "swing",
        _ => "other",
    }
}

/// The action names table: `id<TAB>key<TAB>block<TAB>clip` lines (the `forge` example `action_index`), by id.
#[derive(Default)]
pub struct Names(HashMap<u32, (String, String)>);

impl Names {
    pub fn parse(text: &str) -> Names {
        let mut map = HashMap::new();
        for l in text.lines() {
            let p: Vec<&str> = l.split('\t').collect();
            if let (4, Ok(id)) = (p.len(), u32::from_str_radix(p[0].trim(), 16)) {
                map.entry(id).or_insert_with(|| (p[2].to_string(), p[3].trim().to_string()));
            }
        }
        Names(map)
    }
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.0.len()
    }
    /// The action's block and its first clip.
    pub fn get(&self, id: u32) -> Option<(&str, &str)> {
        self.0.get(&id).map(|(b, c)| (b.as_str(), c.as_str()))
    }
    /// The clip, or the id in hex.
    pub fn clip(&self, id: u32) -> String {
        self.get(id).map_or_else(|| format!("{id:08x}"), |(_, c)| c.to_string())
    }
}

/// One frame of the player.
#[derive(Clone, Debug, Default)]
pub struct Sample {
    /// Game seconds since the log started (the frames' lengths added), wall seconds, the hook's frame count.
    pub t: f32,
    pub wall: f32,
    pub frame: u32,
    /// Game space.
    pub pos: [f32; 3],
    pub yaw: f32,
    pub speed: f32,
    pub context: String,
    pub sub: Option<u32>,
    pub ground_speed: Option<f32>,
    /// The last action started (id), carried over frames, and the actions started this frame.
    pub playing: Option<u32>,
    pub started: Vec<u32>,
    pub bones: Vec<(&'static str, [f32; 3])>,
    pub targets: Vec<[f32; 3]>,
    pub input: Option<Input>,
    pub camera: Option<([f32; 3], [f32; 3])>,
    pub dt: Option<f32>,
}

fn v3(p: [f32; 3]) -> String {
    format!("{:.3},{:.3},{:.3}", p[0], p[1], p[2])
}

impl Sample {
    /// The flight recorder's line (`crates/game/src/recorder.rs`, Bevy space): `time root x y z yaw <deg> vel <m/s> |
    /// <context> clip <clip> | bones <name x,y,z>... | targets x,y,z... | ac1 key=value...`.
    pub fn line(&self, names: &Names) -> String {
        let p = bevy(self.pos);
        let mut s =
            format!("{:8.3} root {:8.3} {:8.3} {:8.3} yaw {:6.1} vel {:5.2} | {}", self.t, p[0], p[1], p[2], bevy_yaw(self.yaw), self.speed, self.context);
        if let Some(id) = self.playing {
            s.push_str(" clip ");
            s.push_str(&names.clip(id));
        }
        s.push_str(" | bones");
        for (name, b) in &self.bones {
            s.push(' ');
            s.push_str(name);
            s.push(' ');
            s.push_str(&v3(bevy(*b)));
        }
        if !self.targets.is_empty() {
            s.push_str(" | targets");
            for t in &self.targets {
                s.push(' ');
                s.push_str(&v3(bevy(*t)));
            }
        }
        s.push_str(&format!(" | ac1 frame={} wall={:.3} mode={}", self.frame, self.wall, mode_of_context(&self.context)));
        if let Some(dt) = self.dt {
            s.push_str(&format!(" dt={dt:.4}"));
        }
        if let Some(sub) = self.sub {
            s.push_str(&format!(" sub={sub}"));
        }
        if let Some(g) = self.ground_speed {
            s.push_str(&format!(" speed={g:.3}"));
        }
        if let Some(id) = self.playing {
            s.push_str(&format!(" act={id:08x}"));
        }
        if !self.started.is_empty() {
            let started: Vec<String> = self.started.iter().map(|&id| format!("{id:08x}:{}", names.clip(id))).collect();
            s.push_str(" started=");
            s.push_str(&started.join(","));
        }
        if let Some(i) = self.input {
            s.push_str(&format!(
                " stick={:.2},{:.2} latch={:.2},{:.2} btn={:04x} high={} legs={} hand={}",
                i.stick[0], i.stick[1], i.latched[0], i.latched[1], i.buttons, i.high as u8, i.legs_buffer as u8, i.hand_buffer as u8
            ));
        }
        if let Some((at, dir)) = self.camera {
            let d = bevy(dir);
            s.push_str(&format!(" cam={} camdir={:.3},{:.3},{:.3}", v3(bevy(at)), d[0], d[1], d[2]));
        }
        s
    }
}

/// The events worth a line of their own, from one frame to the next: context switches (takeoffs, landings, grabs),
/// actions started, teleports.
#[derive(Default)]
pub struct Events {
    last: Option<Sample>,
    /// While in the air: when it left the ground and the highest point.
    air: Option<(f32, f32)>,
}

impl Events {
    pub fn next(&mut self, s: &Sample, names: &Names) -> Vec<String> {
        let mut out = Vec::new();
        let p = bevy(s.pos);
        let at = format!("at {} (game {:.2} {:.2} {:.2})", v3(p), s.pos[0], s.pos[1], s.pos[2]);
        let head = |what: String| format!("{:8.3} {what}", s.t);
        if let Some(last) = &self.last {
            let d = ((s.pos[0] - last.pos[0]).powi(2) + (s.pos[1] - last.pos[1]).powi(2) + (s.pos[2] - last.pos[2]).powi(2)).sqrt();
            if d > 3.0 {
                out.push(head(format!("teleport {d:.2} m {at}")));
            }
            if s.context != last.context {
                let (from, to) = (mode_of_context(&last.context), mode_of_context(&s.context));
                let kind = match (from, to) {
                    (_, "air") => "takeoff",
                    ("air", "ground") => "land",
                    ("air", "hay") => "land",
                    (_, "hang" | "climb" | "ladder" | "beam" | "swing") if from == "air" || from == "ground" || from == "wallrun" => "grab",
                    _ => "context",
                };
                let mut line = format!("{kind} {} -> {} {at}", last.context, s.context);
                if to != "air"
                    && let Some((t0, top)) = self.air.take()
                {
                    line.push_str(&format!(" after {:.2} s in the air, {:.2} m below its top", s.t - t0, top - s.pos[2]));
                }
                out.push(head(line));
            }
        }
        if mode_of_context(&s.context) == "air" {
            let e = self.air.get_or_insert((s.t, s.pos[2]));
            e.1 = e.1.max(s.pos[2]);
        } else {
            self.air = None;
        }
        for &id in &s.started {
            let what = names.get(id).map_or_else(|| "?".to_string(), |(b, c)| format!("{b} {c}"));
            out.push(head(format!("action {id:08x} {what} in {}", s.context)));
        }
        self.last = Some(s.clone());
        out
    }
}

/// `path` with `.k` before its extension (`ac1-live.txt` → `ac1-live.1.txt`).
pub fn rotated(path: &Path, k: u32) -> PathBuf {
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let name = match path.extension() {
        Some(e) => format!("{stem}.{k}.{}", e.to_string_lossy()),
        None => format!("{stem}.{k}"),
    };
    path.with_file_name(name)
}

/// A text file written through a buffer, started over (the old one kept as `.1`, older ones shifted up to `keep`) when
/// it would pass `limit` bytes.
pub struct Rotating {
    path: PathBuf,
    header: String,
    limit: u64,
    keep: u32,
    file: Option<std::io::BufWriter<std::fs::File>>,
    written: u64,
}

impl Rotating {
    pub fn create(path: PathBuf, header: &str, limit: u64, keep: u32) -> std::io::Result<Rotating> {
        let mut r = Rotating { path, header: header.to_string(), limit: limit.max(4096), keep: keep.max(1), file: None, written: 0 };
        r.open()?;
        Ok(r)
    }
    fn open(&mut self) -> std::io::Result<()> {
        let mut f = std::io::BufWriter::with_capacity(1 << 16, std::fs::File::create(&self.path)?);
        f.write_all(self.header.as_bytes())?;
        self.written = self.header.len() as u64;
        self.file = Some(f);
        Ok(())
    }
    fn rotate(&mut self) -> std::io::Result<()> {
        if let Some(mut f) = self.file.take() {
            f.flush()?;
        }
        for k in (1..self.keep).rev() {
            let from = rotated(&self.path, k);
            if from.exists() {
                std::fs::rename(&from, rotated(&self.path, k + 1))?;
            }
        }
        std::fs::rename(&self.path, rotated(&self.path, 1))?;
        self.open()
    }
    pub fn line(&mut self, text: &str) -> std::io::Result<()> {
        let n = text.len() as u64 + 1;
        if self.written + n > self.limit {
            self.rotate()?;
        }
        let f = self.file.as_mut().ok_or_else(|| std::io::Error::other("closed"))?;
        f.write_all(text.as_bytes())?;
        f.write_all(b"\n")?;
        self.written += n;
        Ok(())
    }
    pub fn flush(&mut self) {
        if let Some(f) = self.file.as_mut() {
            let _ = f.flush();
        }
    }
}

impl Drop for Rotating {
    fn drop(&mut self) {
        self.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Memory as a byte map.
    #[derive(Default)]
    struct Fake(HashMap<u32, u8>);

    impl Fake {
        fn put(&mut self, at: u32, bytes: &[u8]) {
            for (k, b) in bytes.iter().enumerate() {
                self.0.insert(at + k as u32, *b);
            }
        }
        fn set32(&mut self, at: u32, v: u32) {
            self.put(at, &v.to_le_bytes());
        }
        fn f32s(&mut self, at: u32, v: &[f32]) {
            for (k, x) in v.iter().enumerate() {
                self.put(at + 4 * k as u32, &x.to_le_bytes());
            }
        }
        fn set16(&mut self, at: u32, v: u16) {
            self.put(at, &v.to_le_bytes());
        }
        /// A reference block at `block` to `obj`.
        fn reference(&mut self, block: u32, obj: u32) {
            self.set32(block, obj);
            self.set32(block + 4, 1);
            self.set32(block + 8, ALIVE);
        }
    }

    impl Mem for Fake {
        fn bytes(&self, addr: u32, out: &mut [u8]) -> bool {
            for (k, b) in out.iter_mut().enumerate() {
                match self.0.get(&(addr + k as u32)) {
                    Some(v) => *b = *v,
                    None => return false,
                }
            }
            true
        }
    }

    /// A skeleton component at 0x1000 whose pose (format `format`) has bone 3 at `p` (stored as the format keeps it).
    fn skeleton(m: &mut Fake, format: u32) {
        let (comp, pose, layout, data) = (0x1000, 0x2000, 0x3000, 0x4000);
        m.set32(comp + 0x184, pose);
        m.set32(pose + 0x58, format);
        m.set32(pose + 0x5C, layout);
        m.set32(pose + 0x60, data);
        m.f32s(pose, &[10.0, 20.0, 30.0]);
        m.set32(layout + 0x34, 0x3100);
        m.set32(layout + 0x54, 0x3200);
        m.set32(layout + 4, 0x3300);
        // bone 3 → slot 4 (+1 = 5) → row 7
        m.set16(0x3100 + 2 * 3, 4);
        m.set16(0x3200 + 2 * 5, 7);
        m.set16(0x3300 + 4 + 12 * 7, 0x40);
        m.set16(0x3300 + 8 + 12 * 7, 0x80);
        m.set16(0x3300 + 4 + 2 + 12 * 7, 0xC0);
        m.f32s(data + 0x40, &[1.0, 2.0, 3.0]);
        m.put(data + 0x80, &[(1024i16).to_le_bytes(), (-2048i16).to_le_bytes(), (4096i16).to_le_bytes()].concat());
        m.f32s(data + 0xC0, &[0.5, 0.25, 0.125]);
        // the skeleton's hash table: Hips at index 3
        m.set32(comp + 0x174, 0x5000);
        m.set32(0x5000 + 0x24, 0x5100);
        m.set32(0x5000 + 0x28, 0x8000_0002);
        m.set32(0x5100, 0x1234_5678);
        m.put(0x5104, &[9]);
        m.set32(0x5108, HIPS_HASH);
        m.put(0x510C, &[3]);
    }

    #[test]
    fn bone_positions_in_each_storage() {
        let mut m = Fake::default();
        skeleton(&mut m, 0);
        assert_eq!(bone_world(&m, 0x1000, 3), Some([1.0, 2.0, 3.0]));
        m.set32(0x2000 + 0x58, 2);
        assert_eq!(bone_world(&m, 0x1000, 3), Some([10.5, 19.0, 32.0]));
        m.set32(0x2000 + 0x58, 1);
        assert_eq!(bone_world(&m, 0x1000, 3), Some([10.5, 20.25, 30.125]));
        assert_eq!(bone_index(&m, 0x1000, HIPS_HASH), Some(3));
        assert_eq!(bone_index(&m, 0x1000, 0xdead_beef), None);
        // unreadable: none, no panic
        assert_eq!(bone_world(&m, 0x9000, 3), None);
    }

    #[test]
    fn bones_through_the_handles() {
        let mut m = Fake::default();
        skeleton(&mut m, 0);
        let human = 0x10000;
        m.reference(0x6000, 0x1000);
        for (at, _) in BONE_HANDLES {
            m.set32(human + at, 0x6000);
            m.put(human + at + 4, &[3]);
        }
        let b = bones(&m, human);
        assert_eq!(b.len(), 6);
        assert_eq!(b[0], ("Hips", [1.0, 2.0, 3.0]));
        assert_eq!(b[3].0, "RightHand");
        // a dead reference: no bones
        m.set32(0x6008, 0);
        assert!(bones(&m, human).is_empty());
    }

    #[test]
    fn limb_goals_in_their_objects() {
        let mut m = Fake::default();
        let human = 0x10000;
        let l = human + HUMAN_LIMB_IK + 0x90;
        m.put(l, &[0; 0x90]);
        m.put(l + 2, &[1]);
        m.put(l + 8, &[1]);
        m.f32s(l + 0x30, &[1.0, 0.0, 2.0]);
        m.reference(0x7000, 0x8000);
        m.set32(l + 0x50, 0x7000);
        m.set32(0x8000 + 0x50, 0);
        // the object turned a quarter about Z, at (5, 6, 7)
        m.f32s(0x8010, &[0.0, 1.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 5.0, 6.0, 7.0, 1.0]);
        assert_eq!(limb_goal(&m, human, 1), Some([5.0, 7.0, 9.0]));
        // moving to a hold whose object is gone: none
        m.put(l + 8, &[0]);
        assert_eq!(limb_goal(&m, human, 1), None);
        // off
        m.put(l + 2, &[0]);
        assert_eq!(limb_goal(&m, human, 1), None);
    }

    #[test]
    fn input_and_camera() {
        let mut m = Fake::default();
        let interp = 0x20000;
        m.put(interp, &[0; 0x1400]);
        m.set32(interp + 0xD0, 0x30000);
        m.put(0x30000, &[0; 0x120]);
        m.put(0x30000 + 0x18, &[1]);
        m.put(0x30000 + 0x18 + 12, &[1]);
        m.f32s(0x30000 + 0x100, &[0.0, 1.0]);
        m.put(interp + 0x112B, &[1]);
        let i = input(&m, interp).unwrap();
        assert_eq!(i.buttons, 0x1001);
        assert_eq!(i.stick, [0.0, 1.0]);
        assert!(i.high && !i.legs_buffer);

        m.set32(CAMERA_MANAGER, 0x40000);
        m.set32(0x40000 + 0xC, 0x41000);
        m.reference(0x41000, 0x42000);
        m.set32(0x42000 + 0x74, 0x43000);
        m.f32s(0x43000, &[1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 1.0, 2.0, 3.0, 1.0]);
        assert_eq!(camera(&m), Some(([1.0, 2.0, 3.0], [0.0, 1.0, 0.0])));
    }

    #[test]
    fn the_recorder_line() {
        let names = Names::parse("05923bdb\t11111111\tHumanGround\txx_h_run\nbadline\n");
        assert_eq!(names.len(), 1);
        let s = Sample {
            t: 1.5,
            pos: [1.0, 2.0, 3.0],
            yaw: 90.0,
            speed: 4.25,
            context: "HumanGround".into(),
            playing: Some(0x05923bdb),
            started: vec![0x05923bdb, 0xabc],
            bones: vec![("Hips", [1.0, 2.0, 4.0])],
            targets: vec![[0.0, 1.0, 0.0]],
            ..Default::default()
        };
        let l = s.line(&names);
        assert!(l.starts_with("   1.500 root    1.000    3.000   -2.000 yaw    0.0 vel  4.25 | HumanGround clip xx_h_run | bones Hips 1.000,4.000,-2.000 | targets 0.000,0.000,-1.000 | ac1 "), "{l}");
        assert!(l.contains("started=05923bdb:xx_h_run,00000abc:00000abc"), "{l}");
        // the recorder's parse (as the `pops` example reads it): time, "root", three numbers; the state; the bones
        let parts: Vec<&str> = l.split(" | ").collect();
        assert_eq!(parts[0].split_whitespace().nth(1), Some("root"));
        assert!(parts[2].starts_with("bones "));
    }

    #[test]
    fn yaw_and_space() {
        assert_eq!(bevy_yaw(90.0), 0.0);
        assert_eq!(bevy_yaw(0.0), -90.0);
        assert_eq!(bevy_yaw(-90.0), 180.0);
        assert_eq!(bevy([1.0, 2.0, 3.0]), [1.0, 3.0, -2.0]);
    }

    #[test]
    fn events_for_a_jump() {
        let names = Names::default();
        let mut e = Events::default();
        let at = |t: f32, z: f32, c: &str| Sample { t, pos: [0.0, 0.0, z], context: c.into(), ..Default::default() };
        assert!(e.next(&at(0.0, 0.0, "HumanGround"), &names).is_empty());
        assert!(e.next(&at(0.1, 0.5, "HumanInAir"), &names)[0].contains("takeoff HumanGround -> HumanInAir"));
        e.next(&at(0.3, 1.5, "HumanInAir"), &names);
        let land = e.next(&at(0.9, -1.0, "HumanGround"), &names);
        assert!(land[0].contains("land HumanInAir -> HumanGround"), "{land:?}");
        assert!(land[0].contains("after 0.80 s in the air, 2.50 m below its top"), "{land:?}");
        assert!(e.next(&at(1.0, 9.0, "HumanGround"), &names)[0].contains("teleport"));
    }

    #[test]
    fn files_rotate_at_their_limit() {
        let dir = std::env::temp_dir().join(format!("ac1-hook-rotate-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("ac1-live.txt");
        assert_eq!(rotated(&path, 2), dir.join("ac1-live.2.txt"));
        let mut r = Rotating::create(path.clone(), "# head\n", 4096, 2).unwrap();
        let line = "x".repeat(999);
        for _ in 0..13 {
            r.line(&line).unwrap();
        }
        r.flush();
        let size = |p: &Path| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
        assert!(size(&path) <= 4096 && size(&rotated(&path, 1)) <= 4096 && size(&rotated(&path, 2)) <= 4096);
        assert!(!rotated(&path, 3).exists());
        assert!(std::fs::read_to_string(rotated(&path, 1)).unwrap().starts_with("# head\n"));
        drop(r);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
