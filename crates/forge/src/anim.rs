//! Animation objects (class 0x0fa3067f).
//!
//! Object: the usual class/size/name header, but the name is followed by a dependency table, so the
//! body starts at the second `id, class` pair. Body: f32 duration (s), ...; then a track table
//! (u32 count, then per track 12 bytes: u32 0, u32 0x653caa76, u32 bone name CRC32, sorted by hash;
//! a bone with rotation and translation appears twice), 2 bytes, u32 track count again, then one
//! keyframe block per track:
//!
//! `u8 type, u32 in-memory size, u32 key count n, (n-1) frame keys (u8, or u16 if type bit 0),
//!  n values` — key 0 is implicit, frames are 1/60 s.
//!
//! Value codecs by `type & !1`:
//! - rotations, smallest-three (dropped component positive, `R` = 1/sqrt 2):
//!   - `04` 16 bit: `[idx:2][negate:1][-:1][a:4][b:4][c:4]`, field (v-7)/7*R
//!   - `08` 24 bit: one byte per component, 7-bit field (v-63)/63*R, idx = top bits of b1,b0
//!   - `0c` 32 bit: `[idx:2][a:10][b:10][c:10]`, (v-511)/511*R
//!   - `10` 48 bit: three u16, 15-bit field (v-16383)/16383*R, idx = top bits of u16 1,0
//!   - `18` 96 bit: three f32 of a smallest-three quaternion, idx = low bits of f32 1,0
//!   - `14` 64 bit: not decoded yet (104 tracks, all on two bones of present-day Lucy's `ucfa_lucy_*` clips: none of
//!     Altaïr's)
//! - translations, millimetres: `20` u32 `[x:11][y:11][z:10]` signed; `24` i16 x3; `1c` f32 x3 (m)
//! - scalar byte channels (facial, events): `2c`, `34`, `38`; `28` f32, the camera clips' field of view (radians)
//!
//! Event tracks (sounds and contacts) sit between the body's header and the track table, see `parse_events`.
//!
//! Hash 0 tracks are root motion (translation + rotation); hash 2 is the limb-contact (actuator contacts) track, hash 3 an event channel (audit ANIM-15). Voice-line clips (`vo_*`, 16 of
//! them) have a duration and no track table: read as clips without tracks.

use crate::u32_at;
use anyhow::{Result, bail, ensure};

pub const CLASS_ANIMATION: u32 = 0x0fa3067f;
const TRACK_TAG: u32 = 0x653caa76;
pub const FPS: f32 = 60.0;
const R: f32 = std::f32::consts::FRAC_1_SQRT_2;

#[derive(Debug, Clone)]
pub enum Channel {
    /// Quaternions xyzw, local to the parent bone.
    Rotation(Vec<[f32; 4]>),
    /// Metres, local to the parent bone.
    Translation(Vec<[f32; 3]>),
    Scalar(Vec<u8>),
    /// One float a key: type `28`, the camera clips' (`CAM_*`) field of view in radians (0.785 = 45 degrees).
    Float(Vec<f32>),
    /// Codec not decoded yet (type byte kept).
    Unknown(u8),
}

#[derive(Debug, Clone)]
pub struct Track {
    pub bone_hash: u32,
    pub kind: u8,
    /// Frame numbers (1/60 s), first is 0.
    pub keys: Vec<u16>,
    pub channel: Channel,
}

#[derive(Debug, Clone)]
pub struct Animation {
    pub duration: f32,
    pub tracks: Vec<Track>,
    /// The clip's event keys (sounds, surface contacts), in time order; empty when it has none or they did not
    /// parse (see `parse_events`).
    pub events: Vec<AnimEvent>,
}

/// One key of a clip's event tracks.
#[derive(Debug, Clone, PartialEq)]
pub struct AnimEvent {
    /// Seconds into the clip.
    pub time: f32,
    pub kind: EventKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EventKind {
    /// `ContactEvent`: a foot, hand or body meeting a surface; `ty` indexes `CONTACT_HUMAN`. The character's contact
    /// table turns it into a sound (see `forge::sound::contact_table`).
    Contact { ty: u32 },
    /// `AudioEvent`: a sound; `sound` is the forge object id of the `SoundBao` its reference points at.
    Audio { sound: u32 },
    /// Any other event (speech, sound sets, AI and sync events, blood), by class id; `None` for an event held by
    /// reference.
    Other(Option<u32>),
}

/// `ContactEventTypeHuman`, the contact events' types, in the order of the exe's enum.
pub const CONTACT_HUMAN: [&str; 15] = [
    "ft_body_roll",
    "ft_jump_start",
    "ft_land_hands",
    "ft_land_heavy",
    "ft_land_light",
    "ft_land_medium",
    "ft_pivot",
    "ft_run_cycle",
    "ft_slide",
    "ft_sneak_cycle",
    "ft_step_foot",
    "ft_step_toe",
    "ft_walk_cycle",
    "ft_walk_end",
    "ft_walk_start",
];

/// Class ids (CRC32 of the class names).
const CLASS_ANIM_TRACK_EVENT: u32 = 0x41682213;
const CLASS_EVENT_SEED_LINK_DATA: u32 = 0x6ce04d52;
const CLASS_CONTACT_EVENT: u32 = 0x97840cbf;
const CLASS_AUDIO_EVENT: u32 = 0xa7a64a34;

/// Fields after an event's u32 handle, by class: `u` u32, `b` u8, `R` a sound reference (u32 object id, u32 class,
/// u32 ?, u32 the referenced `SoundBao`'s object id). From each class's serializer, as Banned445's
/// AC1-Movement-Rewritten lists them. `FXEvent` (0x6627c719) is not known: a clip holding one has no events.
fn event_fields(class: u32) -> Option<&'static [u8]> {
    Some(match class {
        CLASS_CONTACT_EVENT => b"ubb",
        CLASS_AUDIO_EVENT => b"uRb",
        // AudioBaseEvent, SpeechEvent
        0xdf260da5 | 0x90d9ec94 => b"uR",
        // SoundSetEvent
        0x0efffe90 => b"uu",
        // BloodSplatterEvent
        0xd84e267f => b"uuuu",
        // AnimAssassinationEvent
        0x28c6e4ee => b"u",
        // EntityEvent, DropObjectEvent, AnimSyncFallEvent, AnimSyncDieEvent, AIAnimationEvent: the handle only
        0x691a6344 | 0x46d8b31e | 0x8ed28ae9 | 0x1b735fbf | 0xf9bfcb7c => b"",
        _ => return None,
    })
}

/// A bounds-checked little-endian reader that reports the offset it failed at.
struct Cursor<'a> {
    b: &'a [u8],
    at: usize,
}

impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let Some(s) = self.b.get(self.at..self.at + n) else { bail!("event data ends at {:#x} (wanted {n} bytes)", self.at) };
        self.at += n;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into()?))
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into()?))
    }
    /// An object pointer: u8 0 = an object inline (u32 id, u32 class: returns the class), 2 = a reference (u32),
    /// 3 = none.
    fn object(&mut self) -> Result<Option<u32>> {
        let at = self.at;
        match self.u8()? {
            0 => {
                self.u32()?;
                Ok(Some(self.u32()?))
            }
            2 => {
                self.u32()?;
                Ok(None)
            }
            3 => Ok(None),
            t => bail!("object pointer tag {t} at {at:#x}"),
        }
    }
}

/// The event tracks of an animation body (`Object::body`, from the duration). At +10: u32 track count, then per track
/// an inline `AnimTrackEvent`: u32 ?, u32 seed count, per seed {seed object pointer, event object pointer (inline: u32
/// handle and the class's fields, `event_fields`), u32 ?, u8 ?, `EventSeedLinkData` pointer (inline: 9 bytes ?)}, u32 ?,
/// u16 key count, then one u16 time a key in 1/120 s, key k firing seed k. The layout follows Banned445's
/// AC1-Movement-Rewritten (MIT).
pub fn parse_events(body: &[u8]) -> Result<Vec<AnimEvent>> {
    let mut r = Cursor { b: body, at: 10 };
    let n = r.u32()?;
    ensure!(n < 256, "{n} event tracks at 0xa");
    let mut out = Vec::new();
    for _ in 0..n {
        let at = r.at;
        ensure!(r.object()? == Some(CLASS_ANIM_TRACK_EVENT), "no AnimTrackEvent at {at:#x}");
        r.u32()?;
        let seeds = r.u32()? as usize;
        ensure!(seeds < 4096, "{seeds} event seeds at {:#x}", r.at - 4);
        let mut kinds = Vec::with_capacity(seeds);
        for _ in 0..seeds {
            r.object()?;
            let at = r.at;
            let kind = match r.object()? {
                Some(class) => {
                    let Some(fields) = event_fields(class) else { bail!("event class {class:08x} at {at:#x} not known") };
                    r.u32()?;
                    let mut vals = Vec::with_capacity(fields.len());
                    for f in fields {
                        vals.push(match f {
                            b'u' => r.u32()?,
                            b'b' => r.u8()? as u32,
                            _ => {
                                r.take(12)?;
                                r.u32()?
                            }
                        });
                    }
                    match class {
                        CLASS_CONTACT_EVENT => EventKind::Contact { ty: vals[0] },
                        CLASS_AUDIO_EVENT => EventKind::Audio { sound: vals[1] },
                        c => EventKind::Other(Some(c)),
                    }
                }
                None => EventKind::Other(None),
            };
            r.u32()?;
            r.u8()?;
            let at = r.at;
            if let Some(c) = r.object()? {
                ensure!(c == CLASS_EVENT_SEED_LINK_DATA, "seed link class {c:08x} at {at:#x}");
                r.take(9)?;
            }
            kinds.push(kind);
        }
        r.u32()?;
        let keys = r.u16()? as usize;
        let mut kinds = kinds.into_iter();
        for _ in 0..keys {
            let time = r.u16()? as f32 / 120.0;
            out.push(AnimEvent { time, kind: kinds.next().unwrap_or(EventKind::Other(None)) });
        }
    }
    out.sort_by(|a, b| a.time.total_cmp(&b.time));
    Ok(out)
}

fn smallest_three(idx: usize, c: [f32; 3], negate: bool) -> [f32; 4] {
    let d = (1.0 - c[0] * c[0] - c[1] * c[1] - c[2] * c[2]).max(0.0).sqrt();
    let mut q = [0.0; 4];
    let mut k = 0;
    for (i, slot) in q.iter_mut().enumerate() {
        if i == idx {
            *slot = if negate { -d } else { d };
        } else {
            *slot = c[k];
            k += 1;
        }
    }
    q
}

fn field(v: u32, bits: u32) -> f32 {
    let c = ((1u32 << (bits - 1)) - 1) as f32;
    (v as f32 - c) / c * R
}

fn quat(kind: u8, b: &[u8]) -> Option<[f32; 4]> {
    Some(match kind {
        0x04 => {
            let x = u16::from_le_bytes([b[0], b[1]]) as u32;
            let c = [field((x >> 8) & 15, 4), field((x >> 4) & 15, 4), field(x & 15, 4)];
            smallest_three((x >> 14) as usize, c, (x >> 13) & 1 == 1)
        }
        0x08 => {
            let idx = (((b[1] >> 7) << 1) | (b[0] >> 7)) as usize;
            let c = [field((b[0] & 0x7f) as u32, 7), field((b[1] & 0x7f) as u32, 7), field((b[2] & 0x7f) as u32, 7)];
            smallest_three(idx, c, false)
        }
        0x0c => {
            let x = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
            let c = [field((x >> 20) & 1023, 10), field((x >> 10) & 1023, 10), field(x & 1023, 10)];
            smallest_three((x >> 30) as usize, c, false)
        }
        0x10 => {
            let a: [u32; 3] = std::array::from_fn(|k| u16::from_le_bytes([b[k * 2], b[k * 2 + 1]]) as u32);
            let idx = (((a[1] >> 15) << 1) | (a[0] >> 15)) as usize;
            smallest_three(idx, a.map(|v| field(v & 0x7fff, 15)), false)
        }
        0x18 => {
            // Three floats of a smallest-three quaternion; the dropped index hides in the low bits
            // of x and y.
            let u: [u32; 3] = std::array::from_fn(|k| u32::from_le_bytes(b[k * 4..k * 4 + 4].try_into().unwrap()));
            let idx = (((u[1] & 1) << 1) | (u[0] & 1)) as usize;
            smallest_three(idx, u.map(f32::from_bits), false)
        }
        _ => return None,
    })
}

fn translation(kind: u8, b: &[u8]) -> Option<[f32; 3]> {
    Some(match kind {
        0x20 => {
            let x = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
            let s = |v: u32, w: u32| if v >> (w - 1) == 1 { v as i32 - (1 << w) } else { v as i32 };
            [s(x >> 21, 11) as f32 / 1000.0, s((x >> 10) & 2047, 11) as f32 / 1000.0, s(x & 1023, 10) as f32 / 1000.0]
        }
        0x24 => std::array::from_fn(|k| i16::from_le_bytes([b[k * 2], b[k * 2 + 1]]) as f32 / 1000.0),
        0x1c => std::array::from_fn(|k| f32::from_le_bytes(b[k * 4..k * 4 + 4].try_into().unwrap())),
        _ => return None,
    })
}

fn value_size(kind: u8) -> Option<usize> {
    Some(match kind {
        0x04 => 2,
        0x08 => 3,
        0x0c | 0x20 | 0x28 => 4,
        0x10 | 0x24 => 6,
        0x14 => 8,
        0x18 | 0x1c => 12,
        0x2c | 0x34 | 0x38 => 1,
        _ => return None,
    })
}

/// Parse an animation from the full object bytes (`Object::bytes`).
pub fn parse_animation(obj: &[u8]) -> Result<Animation> {
    ensure!(obj.len() > 16 && u32_at(obj, 0) == CLASS_ANIMATION, "not an animation");
    let name_len = u32_at(obj, 8) as usize;
    // The body starts after the second occurrence of (id, class); find the class tag after the name.
    let class = CLASS_ANIMATION.to_le_bytes();
    let after_name = 12 + name_len;
    let Some(rel) = obj[after_name..].windows(4).position(|w| w == class) else { bail!("animation body not found") };
    let body = &obj[after_name + rel + 4..];
    let duration = f32::from_le_bytes(body[0..4].try_into().unwrap());

    let tag = TRACK_TAG.to_le_bytes();
    // (Voice lines, `vo_*`: a duration and no bone tracks at all, only what goes with the speech.)
    let Some(first) = body.windows(4).position(|w| w == tag) else {
        ensure!(duration.is_finite() && duration > 0.0, "no track table and no duration");
        return Ok(Animation { duration, tracks: vec![], events: parse_events(body).unwrap_or_default() });
    };
    ensure!(first >= 8, "track table too early");
    let count = u32_at(body, first - 8) as usize;
    ensure!(count > 0 && count < 1024, "track count {count}");
    let mut hashes = Vec::with_capacity(count);
    for k in 0..count {
        let t = first + 12 * k;
        ensure!(u32_at(body, t) == TRACK_TAG, "track table entry {k} malformed");
        hashes.push(u32_at(body, t + 4));
    }
    let mut p = first + 12 * (count - 1) + 8 + 2;
    ensure!(u32_at(body, p) as usize == count, "track data count mismatch");
    p += 4;

    let mut tracks = Vec::with_capacity(count);
    for &bone_hash in &hashes {
        ensure!(p + 9 <= body.len(), "track data truncated");
        let t = body[p];
        // Bit 0: u16 keys. Bit 1: a flag that does not change the layout (meaning unknown).
        let kind = t & !3;
        let n = u32_at(body, p + 5) as usize;
        let Some(vs) = value_size(kind) else { bail!("unknown track type {t:#04x}") };
        let mut keys = Vec::with_capacity(n);
        keys.push(0u16);
        let mut q = p + 9;
        for _ in 1..n {
            if t & 1 == 1 {
                keys.push(u16::from_le_bytes([body[q], body[q + 1]]));
                q += 2;
            } else {
                keys.push(body[q] as u16);
                q += 1;
            }
        }
        ensure!(q + n * vs <= body.len(), "track values truncated");
        let vals = &body[q..q + n * vs];
        let channel = if quat(kind, vals).is_some() {
            Channel::Rotation(vals.chunks_exact(vs).map(|c| quat(kind, c).unwrap()).collect())
        } else if translation(kind, vals).is_some() {
            Channel::Translation(vals.chunks_exact(vs).map(|c| translation(kind, c).unwrap()).collect())
        } else if vs == 1 {
            Channel::Scalar(vals.to_vec())
        } else if kind == 0x28 {
            Channel::Float(vals.chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect())
        } else {
            Channel::Unknown(t)
        };
        tracks.push(Track { bone_hash, kind, keys, channel });
        p = q + n * vs;
    }
    ensure!(p == body.len(), "{} trailing bytes after tracks", body.len() - p);
    Ok(Animation { duration, tracks, events: parse_events(body).unwrap_or_default() })
}

fn segment(keys: &[u16], frame: f32) -> (usize, usize, f32) {
    let last = keys.len() - 1;
    if frame <= 0.0 || last == 0 {
        return (0, 0, 0.0);
    }
    if frame >= keys[last] as f32 {
        return (last, last, 0.0);
    }
    let i = keys.partition_point(|&k| (k as f32) <= frame) - 1;
    let (a, b) = (keys[i] as f32, keys[i + 1] as f32);
    (i, i + 1, (frame - a) / (b - a))
}

fn nlerp(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let dot: f32 = a.iter().zip(&b).map(|(x, y)| x * y).sum();
    let s = if dot < 0.0 { -1.0 } else { 1.0 };
    let q: [f32; 4] = std::array::from_fn(|k| a[k] + (b[k] * s - a[k]) * t);
    let l = q.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-9);
    q.map(|x| x / l)
}

impl Track {
    pub fn rotation_at(&self, frame: f32) -> Option<[f32; 4]> {
        let Channel::Rotation(v) = &self.channel else { return None };
        let (i, j, t) = segment(&self.keys, frame);
        Some(nlerp(v[i], v[j], t))
    }
    pub fn translation_at(&self, frame: f32) -> Option<[f32; 3]> {
        let Channel::Translation(v) = &self.channel else { return None };
        let (i, j, t) = segment(&self.keys, frame);
        Some(std::array::from_fn(|k| v[i][k] + (v[j][k] - v[i][k]) * t))
    }
}

/// A weighted mix of clips, as AC1's action items blend a move's variants (`passover` 030cm and 100cm,
/// `hangwall_tr_hangknee` straight, 45 degrees in and 30 out). Clips are matched by normalized time: the mix
/// lasts the weighted duration, and at each of its frames every clip is sampled at the same fraction of
/// its own length. Rotations blend by weighted normalized sums (signs aligned to the first), translations
/// by weighted sums; a bone some clips lack takes the others' weights. Keys at every frame. Weights need not
/// sum to one; non-positive ones are dropped. `None` when no clip has weight.
pub fn mix(clips: &[(&Animation, f32)]) -> Option<Animation> {
    let clips: Vec<(&Animation, f32)> = clips.iter().copied().filter(|(_, w)| *w > 0.0).collect();
    let total: f32 = clips.iter().map(|c| c.1).sum();
    if clips.is_empty() || total <= 0.0 {
        return None;
    }
    let duration = clips.iter().map(|(a, w)| a.duration * w).sum::<f32>() / total;
    let n = (duration * FPS).round().max(1.0) as u16;
    let keys: Vec<u16> = (0..=n).collect();
    // Every (bone, channel kind) any clip has, in first-seen order.
    let mut slots: Vec<(u32, bool, u8)> = vec![];
    for (a, _) in &clips {
        for t in &a.tracks {
            let rot = match t.channel {
                Channel::Rotation(_) => true,
                Channel::Translation(_) => false,
                _ => continue,
            };
            if !slots.iter().any(|s| s.0 == t.bone_hash && s.1 == rot) {
                slots.push((t.bone_hash, rot, t.kind));
            }
        }
    }
    let tracks = slots
        .into_iter()
        .map(|(bone, rot, kind)| {
            let of = |a: &Animation| a.tracks.iter().find(|t| t.bone_hash == bone && matches!(t.channel, Channel::Rotation(_)) == rot).cloned();
            let have: Vec<(Track, f32, f32)> = clips.iter().filter_map(|(a, w)| of(a).map(|t| (t, *w, a.frames()))).collect();
            let at = |k: u16| -> (f32, Vec<(f32, f32)>) {
                let u = k as f32 / n as f32;
                (u, have.iter().map(|(_, w, f)| (*w, u * f)).collect())
            };
            let channel = if rot {
                Channel::Rotation(
                    keys.iter()
                        .map(|&k| {
                            let (_, fs) = at(k);
                            let mut acc = [0.0f32; 4];
                            let mut first: Option<[f32; 4]> = None;
                            for ((t, ..), (w, f)) in have.iter().zip(fs) {
                                let q = t.rotation_at(f).unwrap_or([0.0, 0.0, 0.0, 1.0]);
                                let r = *first.get_or_insert(q);
                                let s = if q.iter().zip(&r).map(|(a, b)| a * b).sum::<f32>() < 0.0 { -w } else { w };
                                acc.iter_mut().zip(q).for_each(|(a, x)| *a += x * s);
                            }
                            let l = acc.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-9);
                            acc.map(|x| x / l)
                        })
                        .collect(),
                )
            } else {
                Channel::Translation(
                    keys.iter()
                        .map(|&k| {
                            let (_, fs) = at(k);
                            let tw: f32 = have.iter().map(|h| h.1).sum();
                            let mut acc = [0.0f32; 3];
                            for ((t, ..), (w, f)) in have.iter().zip(fs) {
                                let p = t.translation_at(f).unwrap_or([0.0; 3]);
                                acc.iter_mut().zip(p).for_each(|(a, x)| *a += x * w / tw);
                            }
                            acc
                        })
                        .collect(),
                )
            };
            // Rotation weights normalize themselves; translations were divided by the weights present.
            Track { bone_hash: bone, kind, keys: keys.clone(), channel }
        })
        .collect::<Vec<_>>();
    // The limb contacts are the heaviest clip's (Banned445's port takes the dominant clip's), at the same fraction of
    // the mix as of that clip.
    let mut tracks = tracks;
    if let Some((a, _)) = clips.iter().max_by(|a, b| a.1.total_cmp(&b.1))
        && let Some(t) = a.tracks.iter().find(|t| t.bone_hash == CONTACT_TRACK && matches!(t.channel, Channel::Scalar(_)))
    {
        let scale = n as f32 / a.frames().max(1e-3);
        let keys = t.keys.iter().map(|&k| (k as f32 * scale).round().min(u16::MAX as f32) as u16).collect();
        tracks.push(Track { keys, ..t.clone() });
    }
    // So are its events, stretched the same way.
    let events = clips.iter().max_by(|a, b| a.1.total_cmp(&b.1)).map_or_else(Vec::new, |(a, _)| {
        let scale = n as f32 / FPS / a.duration.max(1e-3);
        a.events.iter().map(|e| AnimEvent { time: e.time * scale, ..e.clone() }).collect()
    });
    Some(Animation { duration: n as f32 / FPS, tracks, events })
}

/// Bits of the limb-contact track (hash 2, one byte a key): the limb holds on (a hold, the wall, the ground) while its
/// bit is set (`LimbIK__UpdateContactsFromAnimTags` 0xE56FC0, as Banned445's port reads it; `xx_l_climb_1m_u_1lu`: the
/// left hand and toe let go at 0.067 s, the hand takes hold again at 0.333 s, the toe at 0.533 s).
pub mod contact {
    pub const LEFT_TOE: u8 = 1 << 2;
    pub const RIGHT_TOE: u8 = 1 << 3;
    pub const LEFT_HAND: u8 = 1 << 4;
    pub const RIGHT_HAND: u8 = 1 << 5;
}

/// Bone hash of the limb-contact track.
pub const CONTACT_TRACK: u32 = 2;

impl Animation {
    pub fn frames(&self) -> f32 {
        self.duration * FPS
    }

    /// The limb-contact bits (`contact`) at `frame`: the last key at or before it, held (the bits step, they are not
    /// interpolated). `None` when the clip has no contact track.
    pub fn contacts_at(&self, frame: f32) -> Option<u8> {
        let t = self.tracks.iter().find(|t| t.bone_hash == CONTACT_TRACK)?;
        let Channel::Scalar(v) = &t.channel else { return None };
        let i = t.keys.partition_point(|&k| (k as f32) <= frame + 1e-3).saturating_sub(1);
        v.get(i).copied()
    }
    /// Root motion (hash 0 translation) over the whole clip, metres.
    pub fn root_motion(&self) -> [f32; 3] {
        self.tracks.iter().find(|t| t.bone_hash == 0).and_then(|t| t.translation_at(f32::MAX)).unwrap_or([0.0; 3])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codecs_decode_unit_quaternions() {
        // Values seen in the walk cycle; each must decode to a unit quaternion.
        for (k, hex) in [(0x04u8, "b6cc"), (0x08, "bfbf7e"), (0x0c, "812ed8e1"), (0x10, "153f6b7f1e3f")] {
            let b: Vec<u8> = (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap()).collect();
            let q = quat(k, &b).unwrap();
            let n: f32 = q.iter().map(|x| x * x).sum();
            assert!((n - 1.0).abs() < 0.02, "{k:#x} {q:?}");
        }
        // 24-bit finger value: 90 degrees about z.
        let q = quat(0x08, &[0xbf, 0xbf, 0x7e]).unwrap();
        assert!(q[0].abs() < 0.01 && q[1].abs() < 0.01 && (q[2] - 0.707).abs() < 0.02 && (q[3] - 0.707).abs() < 0.02);
        // 32-bit packed millimetre translation (helper bone at 73, 25, 7 mm).
        let t = translation(0x20, &[0x07, 0x64, 0x20, 0x09]).unwrap();
        assert!((t[0] - 0.073).abs() < 1e-4 && (t[1] - 0.025).abs() < 1e-4 && (t[2] - 0.007).abs() < 1e-4);
    }

    fn clip(duration: f32, x: f32, angle: f32) -> Animation {
        let f = (duration * FPS) as u16;
        let (s, c) = (angle / 2.0).sin_cos();
        Animation {
            duration,
            tracks: vec![
                Track { bone_hash: 0, kind: 0, keys: vec![0, f], channel: Channel::Translation(vec![[0.0; 3], [x, 0.0, 0.0]]) },
                Track { bone_hash: 7, kind: 0, keys: vec![0, f], channel: Channel::Rotation(vec![[0.0, 0.0, 0.0, 1.0], [0.0, 0.0, s, c]]) },
            ],
            events: vec![],
        }
    }

    /// A 30/70 mix of a short and a long clip lasts the weighted time and lands the weighted distance.
    #[test]
    fn mix_blends_by_normalized_time() {
        let (a, b) = (clip(0.5, 0.3, 0.0), clip(1.0, 1.0, 1.0));
        let m = mix(&[(&a, 0.3), (&b, 0.7)]).unwrap();
        assert!((m.duration - 0.85).abs() < 1.0 / FPS);
        let end = m.root_motion();
        assert!((end[0] - (0.3 * 0.3 + 0.7 * 1.0)).abs() < 1e-3, "{end:?}");
        // Halfway, each is sampled halfway through its own length.
        let mid = m.tracks[0].translation_at(m.frames() / 2.0).unwrap();
        assert!((mid[0] - (0.3 * 0.15 + 0.7 * 0.5)).abs() < 0.02, "{mid:?}");
        let q = m.tracks[1].rotation_at(m.frames()).unwrap();
        assert!((q.iter().map(|x| x * x).sum::<f32>() - 1.0).abs() < 1e-4);
        assert!(mix(&[(&a, 0.0)]).is_none());
    }

    /// Contact bits step from key to key; a mix takes its heaviest clip's, stretched to its length.
    #[test]
    fn contacts_step_and_follow_the_heaviest_clip() {
        let mut a = clip(0.5, 0.3, 0.0);
        a.tracks.push(Track { bone_hash: CONTACT_TRACK, kind: 0x2c, keys: vec![0, 4, 20], channel: Channel::Scalar(vec![0b111100, 0b101000, 0b111100]) });
        assert_eq!(a.contacts_at(0.0), Some(0b111100));
        assert_eq!(a.contacts_at(10.0), Some(0b101000));
        assert_eq!(a.contacts_at(25.0), Some(0b111100));
        assert_eq!(clip(0.5, 0.3, 0.0).contacts_at(0.0), None);
        let b = clip(1.0, 1.0, 1.0);
        let m = mix(&[(&a, 0.7), (&b, 0.3)]).unwrap();
        // 0.65 s long: the release at a fifth of the way through a.
        assert_eq!(m.contacts_at(m.frames() * 0.3), Some(0b101000));
        assert_eq!(mix(&[(&a, 0.3), (&b, 0.7)]).unwrap().contacts_at(0.0), None);
    }
}
