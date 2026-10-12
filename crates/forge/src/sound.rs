//! Sound: Ubisoft BAO objects (the DARE sound engine inside the exe), the event graph that picks what plays, the
//! IMA-ADPCM decoder, and the contact tables that give a character's footsteps.
//!
//! The behaviour follows Banned445's AC1-Movement-Rewritten (MIT), which first played these sounds; the layouts were
//! checked against every file of the install (the `bao` tool).
//!
//! Where BAOs live:
//! - `SoundBao` objects (d8295dcb) in the data files: `DataPC.forge`'s "Game Fix", "Game Bootstrap Settings" and the
//!   rank files, and each city's world file. Body: u32 BAO id, u32 n, n u32 forge object ids the BAO uses (its
//!   resources), u8 ?, u8 variants flag. Flag 0: u32 ?, u32 length, the BAO. Flag 1 (speech, one BAO a language):
//!   u32 count, then per variant u32 index (from 1), u32 length, the BAO.
//! - Streamed data files (`*_BAO_0x5...` in `DataPC_StreamedSounds*.forge`): one BAO each, the whole file.
//!
//! A BAO: a 36-byte header (u32 version 0x00011b01, u32 ?, u32 ?, u32 ?, u32 ?, u32 ?, u64 a FILETIME, u32 kind << 28),
//! then the object. Object offsets below are from the object's start (BAO +0x24); the top nibble of a BAO id is its
//! kind.
//! - 1 event: +4 the id's low 28 bits, +8 type. 1 = play a sound (+0xC sound id, +0x10 pitch range, 16.16); 11 = switch
//!   (+0x10 switch variable, +0x14 default event, +0x1C case count, cases at +0x68, 12 bytes: event, ?, value); 12 =
//!   multi (+0x10 count, event ids at +0x68, all play). Others (2, 3, 4, 6, 10: stops, volumes) start nothing.
//! - 2 sound: +8 type, +0x18 gain in dB (16.16). 1 = one resource (+0x20 resource id, +0x48 channels, +0x4C rate,
//!   +0x50 bytes a second, +0x54 frames, +0x58 resource bytes); 4 = random container (+0x20 count, entries at +0x84,
//!   16 bytes: sound id, weight 16.16, repeat allowed, ?). A resource id 0x5... is a streamed file; its first part is
//!   then the resource 0x3... with the same low bits. Types 2, 5, 6, 8 not read.
//! - 3 resource and 5 stream: +0 u32 ?, then the data, told apart by its first bytes (`codec`): an IMA-ADPCM header
//!   (`decode_ima`), an Ogg Vorbis file, a header of interleaved IMA tracks (`decode_tracks`), or none: the rest of a
//!   stream whose start is the resource 0x3... with the same low bits (not decoded).
//! - 4: one object (type 0x80), not read.

use anyhow::{Context, Result, bail, ensure};
use std::collections::HashMap;
use std::sync::Arc;

/// CRC32("SoundBao").
pub const CLASS_SOUND_BAO: u32 = 0xd8295dcb;
/// CRC32("ContactTable").
pub const CLASS_CONTACT_TABLE: u32 = 0x5a03895e;
const CLASS_CONTACT_SOUND: u32 = 0x54a4c59c;
const CLASS_AUDIO_EVENT: u32 = 0xa7a64a34;
/// A contact table's per-material group (class name not found).
const CLASS_CONTACT_GROUP: u32 = 0xe1cbf60f;
/// The base of a `ContactSound` (class name not found).
const CLASS_CONTACT_ENTRY: u32 = 0x11337755;

pub const BAO_VERSION: u32 = 0x0001_1b01;
pub const BAO_HEADER: usize = 0x24;
/// Nested events and sounds deeper than this are a broken graph.
const MAX_DEPTH: u32 = 8;

fn u32_at(b: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(o..o + 4)?.try_into().ok()?))
}
fn i16_at(b: &[u8], o: usize) -> Option<i16> {
    Some(i16::from_le_bytes(b.get(o..o + 2)?.try_into().ok()?))
}
/// 16.16 fixed point.
fn fixed(v: u32) -> f32 {
    v as i32 as f32 / 65536.0
}

struct Cursor<'a> {
    b: &'a [u8],
    at: usize,
}

impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let Some(s) = self.b.get(self.at..self.at.saturating_add(n)) else { bail!("data ends at {:#x} (wanted {n} bytes)", self.at) };
        self.at += n;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into()?))
    }
    fn expect(&mut self, want: u32, what: &str) -> Result<()> {
        let at = self.at;
        let v = self.u32()?;
        ensure!(v == want, "{what}: {v:#010x} at {at:#x}, wanted {want:#010x}");
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Kind {
    Event,
    Sound,
    Resource,
    Stream,
    Other(u32),
}

/// One BAO: its kind (header +0x20) and its object bytes.
#[derive(Clone, Copy, Debug)]
pub struct Bao<'a> {
    pub kind: Kind,
    pub object: &'a [u8],
}

/// A BAO from its first byte (the version) to its end.
pub fn parse_bao(b: &[u8]) -> Result<Bao<'_>> {
    ensure!(b.len() >= BAO_HEADER, "BAO of {} bytes, shorter than its header", b.len());
    let v = u32_at(b, 0).unwrap_or(0);
    ensure!(v == BAO_VERSION, "BAO version {v:#010x} at 0x0");
    let kind = match u32_at(b, 0x20).unwrap_or(0) {
        0x1000_0000 => Kind::Event,
        0x2000_0000 => Kind::Sound,
        0x3000_0000 => Kind::Resource,
        0x5000_0000 => Kind::Stream,
        k => Kind::Other(k >> 28),
    };
    Ok(Bao { kind, object: &b[BAO_HEADER..] })
}

/// A `SoundBao` object body.
#[derive(Clone, Debug)]
pub struct SoundBao<'a> {
    pub id: u32,
    /// Forge object ids of what the BAO uses (the resources of a sound).
    pub deps: Vec<u32>,
    /// The BAO, or one per language for speech (index from 1, BAO).
    pub variants: Vec<(u32, Bao<'a>)>,
}

impl<'a> SoundBao<'a> {
    /// The BAO (the first variant).
    pub fn bao(&self) -> Bao<'a> {
        self.variants[0].1
    }
}

pub fn parse_sound_bao(body: &[u8]) -> Result<SoundBao<'_>> {
    let mut r = Cursor { b: body, at: 0 };
    let id = r.u32()?;
    let n = r.u32()? as usize;
    ensure!(n <= body.len() / 4, "{n} dependencies at 0x4");
    let deps = (0..n).map(|_| r.u32()).collect::<Result<Vec<_>>>()?;
    r.u8()?;
    let at = r.at;
    let variants = match r.u8()? {
        0 => {
            r.u32()?;
            let len = r.u32()? as usize;
            let at = r.at;
            let bao = parse_bao(r.take(len)?).with_context(|| format!("BAO at {at:#x}"))?;
            vec![(1, bao)]
        }
        1 => {
            let count = r.u32()? as usize;
            ensure!((1..64).contains(&count), "{count} variants at {:#x}", r.at - 4);
            let mut v = Vec::with_capacity(count);
            for _ in 0..count {
                let index = r.u32()?;
                let len = r.u32()? as usize;
                let at = r.at;
                v.push((index, parse_bao(r.take(len)?).with_context(|| format!("BAO at {at:#x}"))?));
            }
            v
        }
        f => bail!("variants flag {f} at {at:#x}"),
    };
    ensure!(r.at == body.len(), "{} bytes left at {:#x}", body.len() - r.at, r.at);
    Ok(SoundBao { id, deps, variants })
}

/// What a resource or stream holds (from its data's first bytes, at object +4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Codec {
    /// An IMA-ADPCM header (version 5).
    Ima,
    /// Interleaved mono IMA tracks (u16 8, u16 4).
    Tracks,
    Ogg,
    /// No header: a stream's continuation.
    Raw,
}

pub fn codec(object: &[u8]) -> Codec {
    match object.get(4..8) {
        Some(b"OggS") => Codec::Ogg,
        Some([5, 0, _, 0]) => Codec::Ima,
        Some([8, 0, 4, 0]) => Codec::Tracks,
        _ => Codec::Raw,
    }
}

/// The Ogg Vorbis file in an Ogg resource or stream.
pub fn ogg_bytes(object: &[u8]) -> Option<&[u8]> {
    (codec(object) == Codec::Ogg).then(|| &object[4..])
}

/// An IMA resource or stream, either layout, decoded to interleaved samples (all the data holds).
pub fn decode(object: &[u8]) -> Result<(u16, Vec<i16>)> {
    match codec(object) {
        Codec::Tracks => decode_tracks(object),
        _ => decode_ima(object, None),
    }
}

const IMA_INDEX: [i32; 16] = [-1, -1, -1, -1, 2, 4, 6, 8, -1, -1, -1, -1, 2, 4, 6, 8];
const IMA_STEP: [i32; 89] = [
    7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 21, 23, 25, 28, 31, 34, 37, 41, 45, 50, 55, 60, 66, 73, 80, 88, 97, 107, 118, 130, 143, 157, 173, 190, 209, 230,
    253, 279, 307, 337, 371, 408, 449, 494, 544, 598, 658, 724, 796, 876, 963, 1060, 1166, 1282, 1411, 1552, 1707, 1878, 2066, 2272, 2499, 2749, 3024, 3327,
    3660, 4026, 4428, 4871, 5358, 5894, 6484, 7132, 7845, 8630, 9493, 10442, 11487, 12635, 13899, 15289, 16818, 18500, 20350, 22385, 24623, 27086, 29794,
    32767,
];

/// One channel's IMA-ADPCM state.
#[derive(Clone, Copy, Debug, Default)]
struct Ima {
    pred: i32,
    index: i32,
}

impl Ima {
    /// The standard IMA step for one 4-bit code.
    fn step(&mut self, n: u8) -> i16 {
        let step = IMA_STEP[self.index as usize];
        let diff = (step * (2 * (n & 7) as i32 + 1)) >> 3;
        self.pred = if n & 8 != 0 { self.pred - diff } else { self.pred + diff }.clamp(-32768, 32767);
        self.index = (self.index + IMA_INDEX[n as usize]).clamp(0, 88);
        self.pred as i16
    }
}

/// The 28-byte header in front of IMA data: u8 version (5), 11 bytes ?, u8 stereo, u8 ?, u16 PCM frames stored before
/// the codes, i16 predictor and u8 step index (left), u8 ?, i16 / u8 (right), u8 ?, u8 a last sample follows (mono),
/// u8 ?, i16 that sample.
#[derive(Clone, Copy, Debug)]
pub struct ImaHeader {
    pub stereo: bool,
    pub pcm_frames: usize,
    state: [Ima; 2],
    pub last: Option<i16>,
}

pub const IMA_HEADER: usize = 28;

pub fn ima_header(h: &[u8]) -> Result<ImaHeader> {
    ensure!(h.len() >= IMA_HEADER, "IMA header of {} bytes", h.len());
    ensure!(h[0] == 5, "IMA header version {}", h[0]);
    let st = |p: usize| Ima { pred: i16_at(h, p).unwrap_or(0) as i32, index: (h[p + 2] as i32).clamp(0, 88) };
    Ok(ImaHeader {
        stereo: h[12] == 1,
        pcm_frames: u16::from_le_bytes([h[14], h[15]]) as usize,
        state: [st(16), st(20)],
        last: (h[24] == 1).then(|| i16_at(h, 26).unwrap_or(0)),
    })
}

/// Decode an IMA resource object (codec at +0, the header at +4, then the PCM frames, then one byte per two mono
/// samples, high nibble first, or per stereo frame, high nibble left) to interleaved samples. `frames`: the count the
/// sound object gives, or `None` for all the data holds. A mono resource's last sample is stored apart.
pub fn decode_ima(object: &[u8], frames: Option<usize>) -> Result<(u16, Vec<i16>)> {
    ensure!(codec(object) == Codec::Ima, "no IMA header");
    let h = ima_header(object.get(4..).unwrap_or(&[]))?;
    let ch = if h.stereo { 2 } else { 1 };
    let data = &object[4 + IMA_HEADER..];
    let raw = h.pcm_frames * ch;
    ensure!(raw * 2 <= data.len(), "{} PCM frames past the end", h.pcm_frames);
    let codes = data.len() - raw * 2;
    let last = h.last.filter(|_| ch == 1);
    let all = raw + codes * 2 + last.is_some() as usize;
    let total = frames.map_or(all, |f| f * ch);
    ensure!(total <= all, "{} frames wanted, the data holds {}", total / ch, all / ch);
    let mut out = Vec::with_capacity(total);
    out.extend(data[..raw * 2].chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]])));
    let mut s = h.state;
    let end = total - last.is_some() as usize;
    for &b in &data[raw * 2..] {
        if out.len() >= end {
            break;
        }
        if ch == 1 {
            out.push(s[0].step(b >> 4));
            if out.len() < end {
                out.push(s[0].step(b & 15));
            }
        } else {
            out.push(s[0].step(b >> 4));
            out.push(s[1].step(b & 15));
        }
    }
    out.truncate(end);
    out.extend(last.filter(|_| out.len() < total));
    Ok((ch as u16, out))
}

/// Interleaved IMA tracks (`Codec::Tracks`), decoded: one mono track per channel. Layout from object +4: u16 8, u16 4,
/// u32 ?, u32 track count, u32 block count, u32 block header size (8 + 4 a track), u32 ?, u32 largest block, a u32
/// header size per track (28), the tracks' IMA headers; then the blocks: u32 ?, u32 block size (header included; 0 in
/// the last block), a u32 data size per track, then each track's data (its first block opens with its PCM frames).
/// Not read: tracks of Ogg pages (track header size 0x1400) and stereo tracks (15 files).
pub fn decode_tracks(object: &[u8]) -> Result<(u16, Vec<i16>)> {
    ensure!(codec(object) == Codec::Tracks, "not interleaved IMA tracks");
    let mut r = Cursor { b: object, at: 4 };
    r.u32()?;
    r.u32()?;
    let tracks = r.u32()? as usize;
    ensure!((1..=8).contains(&tracks), "{tracks} tracks at 0xc");
    let blocks = r.u32()? as usize;
    let block_header = r.u32()? as usize;
    ensure!(block_header == 8 + 4 * tracks, "block header of {block_header} bytes at 0x14 for {tracks} tracks");
    r.u32()?;
    r.u32()?;
    for _ in 0..tracks {
        r.expect(IMA_HEADER as u32, "track header size")?;
    }
    let mut heads = Vec::with_capacity(tracks);
    for _ in 0..tracks {
        let at = r.at;
        let h = ima_header(r.take(IMA_HEADER)?).with_context(|| format!("track header at {at:#x}"))?;
        ensure!(!h.stereo, "stereo track at {at:#x}");
        heads.push(h);
    }
    let mut state: Vec<Ima> = heads.iter().map(|h| h.state[0]).collect();
    let mut chans: Vec<Vec<i16>> = vec![Vec::new(); tracks];
    for b in 0..blocks {
        if r.at == object.len() {
            break;
        }
        let start = r.at;
        r.u32()?;
        let size = r.u32()? as usize;
        let sizes = (0..tracks).map(|_| r.u32().map(|v| v as usize)).collect::<Result<Vec<_>>>()?;
        let used = block_header + sizes.iter().sum::<usize>();
        let size = if size == 0 { used } else { size };
        ensure!(used <= size, "block {b} at {start:#x}: track data past its size {size}");
        // (A resource holding a stream's start is cut inside a block: what is whole is kept.)
        if b > 0 && start + used > object.len() {
            break;
        }
        for (t, &n) in sizes.iter().enumerate() {
            let mut d = r.take(n).with_context(|| format!("block {b} at {start:#x}"))?;
            if b == 0 {
                let raw = heads[t].pcm_frames * 2;
                ensure!(raw <= d.len(), "track {t}'s PCM frames past block 0");
                chans[t].extend(d[..raw].chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]])));
                d = &d[raw..];
            }
            for &c in d {
                chans[t].push(state[t].step(c >> 4));
                chans[t].push(state[t].step(c & 15));
            }
        }
        r.at = start + size.max(block_header);
        ensure!(r.at <= object.len(), "block {b} at {start:#x} of {size} bytes runs past the end");
    }
    let frames = chans.iter().map(Vec::len).min().unwrap_or(0);
    let mut out = Vec::with_capacity(frames * tracks);
    for f in 0..frames {
        out.extend(chans.iter().map(|c| c[f]));
    }
    Ok((tracks as u16, out))
}

/// One sound an event starts.
#[derive(Clone, Debug, PartialEq)]
pub struct Voice {
    /// The resource BAO id.
    pub resource: u32,
    pub channels: u16,
    pub rate: u32,
    pub frames: u32,
    /// The gains of the sounds on the way down, summed (dB).
    pub gain_db: f32,
    /// The play event's pitch factor, picked in 1 - range ..= 1 + range.
    pub speed: f32,
}

/// The random source of the event graph, and each random container's last pick (not repeated unless allowed).
pub struct Picker {
    state: u32,
    last: HashMap<u32, usize>,
}

impl Picker {
    pub fn new(seed: u32) -> Self {
        Picker { state: seed.max(1), last: HashMap::new() }
    }
    /// 0 ..= 65536 (an xorshift).
    fn rand16(&mut self) -> u32 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.state = x;
        ((x >> 17) & 0x7fff) * 65536 / 0x7fff
    }
}

/// A weighted pick over `(sound, weight 16.16, repeat allowed)` with `r` in 0 ..= 65536: the entry `r` falls in,
/// skipping the last pick unless it may repeat (its weight still counted); the first entry when none is hit.
fn pick_weighted(entries: &[(u32, u32, u32)], last: Option<usize>, r: u32) -> Option<usize> {
    if entries.is_empty() {
        return None;
    }
    let skip = last.filter(|&l| entries.get(l).is_some_and(|e| e.2 == 0));
    let mut v = r as i64;
    for (i, e) in entries.iter().enumerate() {
        if v <= e.1 as i64 && skip != Some(i) {
            return Some(i);
        }
        v -= e.1 as i64;
    }
    Some(0)
}

/// The sound objects of the install by BAO id, and which BAO each `SoundBao` forge object holds.
#[derive(Default)]
pub struct Bank {
    objects: HashMap<u32, (Kind, Arc<[u8]>)>,
    by_object: HashMap<u32, u32>,
}

impl Bank {
    pub fn len(&self) -> usize {
        self.objects.len()
    }
    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }

    /// Add a `SoundBao` object (`forge_id` its forge object id); `Err` when its body does not parse.
    pub fn add(&mut self, forge_id: u32, body: &[u8]) -> Result<()> {
        let s = parse_sound_bao(body)?;
        let b = s.bao();
        self.objects.insert(s.id, (b.kind, b.object.into()));
        self.by_object.insert(forge_id, s.id);
        Ok(())
    }

    /// Every `SoundBao` of a decoded data file; returns how many did not parse.
    pub fn add_data_file(&mut self, data: &[u8]) -> Result<usize> {
        let mut bad = 0;
        for o in crate::parse_objects(data)?.iter().filter(|o| o.class == CLASS_SOUND_BAO) {
            if self.add(o.id, o.body).is_err() {
                bad += 1;
            }
        }
        Ok(bad)
    }

    /// The BAO a `SoundBao` forge object holds (an animation's `AudioEvent` names the object).
    pub fn bao_of(&self, forge_id: u32) -> Option<u32> {
        self.by_object.get(&forge_id).copied()
    }

    pub fn object(&self, id: u32) -> Option<&[u8]> {
        self.objects.get(&id).map(|o| &o.1[..])
    }

    /// Every voice event `id` starts. `switch` gives a switch variable's value now (`None`: the default case).
    pub fn resolve(&self, id: u32, switch: &dyn Fn(u32) -> Option<u32>, pick: &mut Picker) -> Vec<Voice> {
        let mut out = Vec::new();
        self.event(id, switch, pick, 0, &mut out);
        out
    }

    fn event(&self, id: u32, switch: &dyn Fn(u32) -> Option<u32>, pick: &mut Picker, depth: u32, out: &mut Vec<Voice>) {
        let Some(o) = self.object(id).filter(|_| depth <= MAX_DEPTH && id >> 28 == 1) else { return };
        match u32_at(o, 8) {
            Some(1) => {
                let (Some(sound), Some(range)) = (u32_at(o, 0xc), u32_at(o, 0x10)) else { return };
                let r = fixed(range).clamp(0.0, 0.9);
                let speed = 1.0 - r + pick.rand16() as f32 / 65536.0 * 2.0 * r;
                self.sound(sound, 0.0, speed, pick, depth + 1, out);
            }
            Some(11) => {
                let (Some(var), Some(default), Some(n)) = (u32_at(o, 0x10), u32_at(o, 0x14), u32_at(o, 0x1c)) else { return };
                let cases: Vec<(u32, u32)> = (0..n.min(256) as usize).map_while(|j| Some((u32_at(o, 0x68 + 12 * j)?, u32_at(o, 0x68 + 12 * j + 8)?))).collect();
                // No value for the variable (or no case for it): the default, else the first case.
                let chosen = switch(var)
                    .and_then(|v| cases.iter().find(|c| c.1 == v).map(|c| c.0))
                    .or_else(|| self.objects.contains_key(&default).then_some(default))
                    .or_else(|| cases.first().map(|c| c.0));
                if let Some(e) = chosen {
                    self.event(e, switch, pick, depth + 1, out);
                }
            }
            Some(12) => {
                let Some(n) = u32_at(o, 0x10) else { return };
                for j in 0..n.min(64) as usize {
                    if let Some(e) = u32_at(o, 0x68 + 4 * j) {
                        self.event(e, switch, pick, depth + 1, out);
                    }
                }
            }
            _ => {}
        }
    }

    fn sound(&self, id: u32, gain_db: f32, speed: f32, pick: &mut Picker, depth: u32, out: &mut Vec<Voice>) {
        let Some(o) = self.object(id).filter(|_| depth <= MAX_DEPTH && id >> 28 == 2) else { return };
        let gain_db = gain_db + u32_at(o, 0x18).map_or(0.0, fixed);
        match u32_at(o, 8) {
            Some(1) => {
                let (Some(resource), Some(ch), Some(rate), Some(frames)) = (u32_at(o, 0x20), u32_at(o, 0x48), u32_at(o, 0x4c), u32_at(o, 0x54)) else {
                    return;
                };
                if self.objects.contains_key(&resource) && (1..=2).contains(&ch) && (4000..=96000).contains(&rate) {
                    out.push(Voice { resource, channels: ch as u16, rate, frames, gain_db, speed });
                }
            }
            Some(4) => {
                let Some(n) = u32_at(o, 0x20) else { return };
                let entries: Vec<(u32, u32, u32)> =
                    (0..n.min(256) as usize).map_while(|j| Some((u32_at(o, 0x84 + 16 * j)?, u32_at(o, 0x88 + 16 * j)?, u32_at(o, 0x8c + 16 * j)?))).collect();
                if let Some(i) = pick_weighted(&entries, pick.last.get(&id).copied(), pick.rand16()) {
                    pick.last.insert(id, i);
                    self.sound(entries[i].0, gain_db, speed, pick, depth + 1, out);
                }
            }
            _ => {}
        }
    }

    /// A voice's samples, interleaved (IMA resources; `None` for the other codecs).
    pub fn decode(&self, v: &Voice) -> Option<Vec<i16>> {
        let (ch, pcm) = decode_ima(self.object(v.resource)?, Some(v.frames as usize)).ok()?;
        (ch == v.channels).then_some(pcm)
    }
}

/// A character's contact table (`ContactTable_AlTair`): per surface material, the event each `ContactEventTypeHuman`
/// (`anim::CONTACT_HUMAN`) plays, as `SoundBao` forge object ids.
#[derive(Clone, Debug, Default)]
pub struct ContactTable {
    pub groups: Vec<(u32, Vec<Option<u32>>)>,
}

impl ContactTable {
    /// The `SoundBao` object of contact type `ty` on `material` (the first group when there is no group for it).
    pub fn sound(&self, material: Option<u32>, ty: u32) -> Option<u32> {
        let g = material.and_then(|m| self.groups.iter().find(|g| g.0 == m)).or(self.groups.first())?;
        g.1.get(ty as usize).copied().flatten()
    }
}

/// A `ContactTable` body: u32 group count; per group u32 id, u32 class (e1cbf60f), u32 material, u32 n, then n
/// entries: u8 0 (inline), u32 id, u32 class (11337755), u32 k, k u32 ?, u32 ?, u32 ?, then the `ContactSound`:
/// u32 class, u32 ?, the `AudioEvent` (u32 class, u32 handle, u32 ?, a sound reference: u32 ?, u32 class, u32 BAO id,
/// u32 `SoundBao` object id), u8 ?, u32 contact type.
pub fn parse_contact_table(body: &[u8]) -> Result<ContactTable> {
    let mut r = Cursor { b: body, at: 0 };
    let n = r.u32()? as usize;
    ensure!(n < 1024, "{n} groups at 0x0");
    let mut groups = Vec::with_capacity(n);
    for _ in 0..n {
        r.u32()?;
        r.expect(CLASS_CONTACT_GROUP, "contact group class")?;
        let material = r.u32()?;
        let k = r.u32()? as usize;
        ensure!(k < 256, "{k} contacts at {:#x}", r.at - 4);
        let mut sounds = vec![None; 15];
        for _ in 0..k {
            let at = r.at;
            ensure!(r.u8()? == 0, "contact entry at {at:#x} not inline");
            r.u32()?;
            r.expect(CLASS_CONTACT_ENTRY, "contact entry class")?;
            let m = r.u32()? as usize;
            ensure!(m < 64, "{m} at {:#x}", r.at - 4);
            r.take(4 * m + 8)?;
            r.expect(CLASS_CONTACT_SOUND, "ContactSound class")?;
            r.u32()?;
            r.expect(CLASS_AUDIO_EVENT, "AudioEvent class")?;
            r.take(16)?;
            r.u32()?;
            let sound = r.u32()?;
            r.u8()?;
            let ty = r.u32()? as usize;
            if let Some(s) = sounds.get_mut(ty) {
                *s = Some(sound);
            }
        }
        groups.push((material, sounds));
    }
    ensure!(r.at == body.len(), "{} bytes left at {:#x}", body.len() - r.at, r.at);
    Ok(ContactTable { groups })
}

/// A 16-bit PCM WAV file.
pub fn wav(channels: u16, rate: u32, samples: &[i16]) -> Vec<u8> {
    let data = samples.len() as u32 * 2;
    let mut w = Vec::with_capacity(44 + data as usize);
    w.extend(b"RIFF");
    w.extend((36 + data).to_le_bytes());
    w.extend(b"WAVEfmt ");
    w.extend(16u32.to_le_bytes());
    w.extend(1u16.to_le_bytes());
    w.extend(channels.to_le_bytes());
    w.extend(rate.to_le_bytes());
    w.extend((rate * channels as u32 * 2).to_le_bytes());
    w.extend((channels * 2).to_le_bytes());
    w.extend(16u16.to_le_bytes());
    w.extend(b"data");
    w.extend(data.to_le_bytes());
    for s in samples {
        w.extend(s.to_le_bytes());
    }
    w
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn class_ids_are_crc32_of_their_names() {
        assert_eq!(crc32fast::hash(b"SoundBao"), CLASS_SOUND_BAO);
        assert_eq!(crc32fast::hash(b"ContactTable"), CLASS_CONTACT_TABLE);
        assert_eq!(crc32fast::hash(b"ContactSound"), CLASS_CONTACT_SOUND);
        assert_eq!(crc32fast::hash(b"AudioEvent"), CLASS_AUDIO_EVENT);
    }

    #[test]
    fn ima_codes_follow_the_standard_tables() {
        let mut s = Ima::default();
        assert_eq!(s.step(7), 13);
        assert_eq!(s.index, 8);
        assert_eq!(s.step(15), 13 - ((16 * 15) >> 3));
        let mut s = Ima { pred: 32760, index: 88 };
        assert_eq!(s.step(7), 32767, "clamped");
    }

    fn ima_object(stereo: bool, pcm: &[i16], last: Option<i16>, codes: &[u8]) -> Vec<u8> {
        let mut o = vec![0; 4];
        let mut h = [0u8; IMA_HEADER];
        h[0] = 5;
        h[12] = stereo as u8;
        h[14] = (pcm.len() / if stereo { 2 } else { 1 }) as u8;
        h[16] = 10;
        if let Some(l) = last {
            h[24] = 1;
            h[26..28].copy_from_slice(&l.to_le_bytes());
        }
        o.extend(h);
        pcm.iter().for_each(|s| o.extend(s.to_le_bytes()));
        o.extend(codes);
        o
    }

    #[test]
    fn ima_resource_reads_pcm_then_codes_high_nibble_first() {
        let o = ima_object(false, &[5, 10], Some(99), &[0x70]);
        // code 7 from predictor 10: +13; code 0 at index 8: +(16 * 1) >> 3 = +2
        assert_eq!(decode_ima(&o, Some(5)).unwrap(), (1, vec![5, 10, 23, 25, 99]));
        assert_eq!(decode_ima(&o, None).unwrap().1.len(), 5);
        assert!(decode_ima(&o, Some(6)).is_err(), "more frames than the data holds");
        assert!(decode_ima(&o[..20], None).is_err(), "truncated header");
        let st = ima_object(true, &[1, 2], None, &[0x77]);
        assert_eq!(decode_ima(&st, None).unwrap(), (2, vec![1, 2, 23, 13]));
    }

    #[test]
    fn stream_interleaves_its_tracks_block_by_block() {
        let mut o = vec![0; 4];
        for v in [0x0004_0008u32, 0x25, 2, 2, 16, 0x80, 0x200, 28, 28] {
            o.extend(v.to_le_bytes());
        }
        for _ in 0..2 {
            let mut h = [0u8; IMA_HEADER];
            h[0] = 5;
            h[14] = 1;
            o.extend(h);
        }
        // block 0: one PCM frame and one code byte a track; block 1: one code byte a track
        for v in [3u32, 16 + 6, 3, 3] {
            o.extend(v.to_le_bytes());
        }
        o.extend([1, 0, 0x70, 2, 0, 0x70]);
        for v in [3u32, 0, 1, 1] {
            o.extend(v.to_le_bytes());
        }
        o.extend([0x00, 0x00]);
        let (ch, pcm) = decode_tracks(&o).unwrap();
        assert_eq!(ch, 2);
        assert_eq!(pcm.len(), 2 * 5);
        assert_eq!(&pcm[..6], &[1, 2, 13, 13, 15, 15]);
        assert_eq!(decode_tracks(&o[..o.len() - 1]).unwrap().1.len(), 2 * 3, "a cut last block is left out");
        assert!(decode_tracks(&o[..o.len() - 12]).is_err(), "a cut first block is an error");
    }

    #[test]
    fn weighted_pick_skips_the_last_unless_it_may_repeat() {
        let e = [(1, 0x5555, 0), (2, 0x5555, 0), (3, 0x5556, 0)];
        assert_eq!(pick_weighted(&e, None, 0), Some(0));
        assert_eq!(pick_weighted(&e, None, 0x6000), Some(1));
        assert_eq!(pick_weighted(&e, Some(1), 0x6000), Some(2));
        assert_eq!(pick_weighted(&e, Some(2), 0x10000), Some(0));
        assert_eq!(pick_weighted(&[(1, 0x8000, 1), (2, 0x8000, 1)], Some(0), 0), Some(0));
    }

    fn sound_bao(id: u32, kind: u32, object: &[u8]) -> Vec<u8> {
        let mut bao = BAO_VERSION.to_le_bytes().to_vec();
        bao.resize(0x20, 0);
        bao.extend((kind << 28).to_le_bytes());
        bao.extend(object);
        let mut b = id.to_le_bytes().to_vec();
        b.extend(0u32.to_le_bytes());
        b.extend([0x0d, 0]);
        b.extend(0u32.to_le_bytes());
        b.extend((bao.len() as u32).to_le_bytes());
        b.extend(bao);
        b
    }

    fn put(o: &mut Vec<u8>, at: usize, v: u32) {
        if o.len() < at + 4 {
            o.resize(at + 4, 0);
        }
        o[at..at + 4].copy_from_slice(&v.to_le_bytes());
    }

    #[test]
    fn events_switch_multi_and_containers_resolve_to_voices() {
        let mut bank = Bank::default();
        let mut add = |forge: u32, id: u32, o: Vec<u8>| bank.add(forge, &sound_bao(id, id >> 28, &o)).unwrap();
        add(1, 0x3000_0001, ima_object(false, &[0], None, &[0; 8]));
        let mut s = vec![];
        put(&mut s, 8, 1);
        put(&mut s, 0x18, (-6i32 << 16) as u32);
        put(&mut s, 0x20, 0x3000_0001);
        put(&mut s, 0x48, 1);
        put(&mut s, 0x4c, 24000);
        put(&mut s, 0x54, 17);
        add(2, 0x2000_0001, s);
        let mut c = vec![];
        put(&mut c, 8, 4);
        put(&mut c, 0x20, 1);
        put(&mut c, 0x84, 0x2000_0001);
        put(&mut c, 0x88, 0x10000);
        put(&mut c, 0x8c, 0);
        add(3, 0x2000_0002, c);
        let play = |sound: u32, range: u32| {
            let mut e = vec![];
            put(&mut e, 8, 1);
            put(&mut e, 0xc, sound);
            put(&mut e, 0x10, range);
            e
        };
        add(4, 0x1000_0001, play(0x2000_0002, 0));
        add(5, 0x1000_0002, play(0x2000_0001, 0x1000));
        let mut m = vec![];
        put(&mut m, 8, 12);
        put(&mut m, 0x10, 2);
        put(&mut m, 0x68, 0x1000_0001);
        put(&mut m, 0x6c, 0x1000_0002);
        add(6, 0x1000_0003, m);
        let mut w = vec![];
        put(&mut w, 8, 11);
        put(&mut w, 0x10, 3);
        put(&mut w, 0x14, 0x1000_0002);
        put(&mut w, 0x1c, 1);
        put(&mut w, 0x68, 0x1000_0003);
        put(&mut w, 0x70, 0);
        add(7, 0x1000_0004, w);
        assert_eq!(bank.bao_of(7), Some(0x1000_0004));

        let mut pick = Picker::new(7);
        let two = bank.resolve(0x1000_0004, &|v| (v == 3).then_some(0), &mut pick);
        assert_eq!(two.len(), 2, "the switch's case plays the multi event's two children");
        assert!(two.iter().all(|v| v.resource == 0x3000_0001 && v.rate == 24000 && (v.gain_db + 6.0).abs() < 1e-4));
        assert_eq!(two[0].speed, 1.0);
        assert!((two[1].speed - 1.0).abs() <= 0.0625 + 1e-4);
        assert_eq!(bank.decode(&two[0]).map(|p| p.len()), Some(17));
        assert_eq!(bank.resolve(0x1000_0004, &|_| None, &mut pick).len(), 1, "no value: the default event");
    }

    #[test]
    fn sound_bao_bodies_parse_or_fail_with_offsets() {
        let b = sound_bao(0x1000_0001, 1, &[0; 16]);
        let s = parse_sound_bao(&b).unwrap();
        assert_eq!((s.id, s.bao().kind, s.bao().object.len()), (0x1000_0001, Kind::Event, 16));
        assert!(parse_sound_bao(&b[..b.len() - 1]).is_err());
        let mut extra = b.clone();
        extra.push(0);
        assert!(parse_sound_bao(&extra).unwrap_err().to_string().contains("1 bytes left"));
    }

    /// The install's player sounds resolve and decode (skips without the install).
    #[test]
    fn install_contact_sounds_decode() {
        let dir = std::env::var("AC1_GAME_DIR").unwrap_or_else(|_| r"P:\SteamLibrary\steamapps\common\Assassins Creed".into());
        let Ok(forge) = crate::Forge::open(std::path::Path::new(&dir).join("DataPC.forge")) else {
            eprintln!("no install, skipped");
            return;
        };
        let mut bank = Bank::default();
        let mut table = None;
        for name in ["Game Fix", "Game Bootstrap Settings", "Rank 9"] {
            let e = forge.entries.iter().find(|e| e.name == name).unwrap();
            let d = forge.read(e).unwrap();
            assert_eq!(bank.add_data_file(&d).unwrap(), 0, "{name}: SoundBaos that do not parse");
            for o in crate::parse_objects(&d).unwrap().iter().filter(|o| o.class == CLASS_CONTACT_TABLE && o.name == "ContactTable_AlTair") {
                table = Some(parse_contact_table(o.body).unwrap());
            }
        }
        let table = table.expect("ContactTable_AlTair");
        assert!(table.groups.len() > 1);
        let mut pick = Picker::new(1);
        let mut voices = 0;
        for ty in 0..15 {
            let Some(e) = table.sound(None, ty).and_then(|s| bank.bao_of(s)) else { panic!("contact {ty} has no event") };
            for v in bank.resolve(e, &|_| None, &mut pick) {
                let pcm = bank.decode(&v).expect("IMA");
                assert_eq!(pcm.len(), v.frames as usize * v.channels as usize);
                let clipped = pcm.iter().filter(|s| s.unsigned_abs() >= 32767).count();
                assert!(clipped * 100 < pcm.len(), "{:08x}: {clipped} of {} clipped", v.resource, pcm.len());
                voices += 1;
            }
        }
        assert!(voices >= 10, "{voices} voices");
    }
}
