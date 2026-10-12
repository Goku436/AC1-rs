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
//!   16 bytes: sound id, weight 16.16, repeat allowed, ?); 6 = a streamed file of interleaved tracks, the ambience
//!   loops (+0x24 track count, +0x50 the stream's id, per track 0x30 bytes from +0x84: u32 rate, ...). A resource id
//!   0x5... is a streamed file; its first part is then the resource 0x3... with the same low bits. Types 2, 5, 8 not
//!   read.
//!
//! A city's ambience: some grid cells' data files (`Cell<k>_DataBlock`) hold the `AssassinAmbienceRegionCellData` of a
//! region (its cell's own record, class ac2bbf68, links it; +8 the region's `SoundAmbienceSet`, then u8 ?, three more
//! sets: the crowd murmur by density),
//! and the sets hold sound references (u32 class bb8e3eec, u32 event BAO id, u32 `SoundBao` object id): the loop and
//! the one-off sounds (gusts, birds) played now and then.
//! - 3 resource and 5 stream: +0 u32 ?, then the data, told apart by its first bytes (`codec`): an IMA-ADPCM header
//!   (`decode_ima`), an Ogg Vorbis file (`decode_ogg`, lewton), a header of interleaved tracks (`decode_tracks`: IMA
//!   or Ogg Vorbis, mono or stereo), or none: the rest of a stream whose start is the resource 0x3... with the same
//!   low bits (`join_stream`; speech, of the variant in the stream's language).
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
        Some([8, 0, 4, 0]) => Codec::Tracks,
        // Version 5, then (some files) a time; the stereo flag, the step indexes and the last-sample flag in range.
        Some([5, ..]) if object.len() >= 4 + IMA_HEADER && object[16] <= 1 && object[22] <= 88 && object[26] <= 88 && object[28] <= 1 => Codec::Ima,
        _ => Codec::Raw,
    }
}

/// A stream whose start is a resource (`Codec::Raw` streams: the resource 0x3... with the same low bits holds the
/// start, the stream file the rest, from the byte after): the two objects' data joined into one object to decode.
pub fn join_stream(resource: &[u8], stream: &[u8]) -> Vec<u8> {
    let mut v = resource.to_vec();
    v.extend_from_slice(stream.get(4..).unwrap_or(&[]));
    v
}

/// The Ogg Vorbis file in an Ogg resource or stream.
pub fn ogg_bytes(object: &[u8]) -> Option<&[u8]> {
    (codec(object) == Codec::Ogg).then(|| &object[4..])
}

/// A resource or stream, any codec but `Raw`, decoded to interleaved samples (all the data holds).
pub fn decode(object: &[u8]) -> Result<(u16, Vec<i16>)> {
    match codec(object) {
        Codec::Tracks => decode_tracks(object),
        Codec::Ogg => decode_ogg(&object[4..]).map(|(ch, _, pcm)| (ch, pcm)),
        Codec::Ima => decode_ima(object, None),
        Codec::Raw => bail!("no header: the rest of a stream"),
    }
}

/// The sample rate the data gives, where it does: an Ogg file's or the first Ogg track's Vorbis header. (IMA data
/// does not hold it: its sound object does.)
pub fn data_rate(object: &[u8]) -> Option<u32> {
    let ogg = match codec(object) {
        Codec::Ogg => &object[4..],
        Codec::Tracks if u32_at(object, 0x20) == Some(OGG_TRACK_HEADER as u32) => {
            let tracks = u32_at(object, 0xc)? as usize;
            object.get(0x20 + 4 * tracks..)?
        }
        _ => return None,
    };
    // The first page holds the identification packet: 1 "vorbis", u32 version, u8 channels, u32 rate.
    let k = ogg.windows(7).take(256).position(|w| w == b"\x01vorbis")?;
    u32_at(ogg, k + 12).filter(|r| (4000..=192000).contains(r))
}

/// Whether `frames` decoded frames are all the data holds: for Ogg data, as many as the last page header's granule
/// position says (within a block of 4096: the decoder's trimming of the last packet); IMA data always is.
pub fn complete(object: &[u8], frames: usize) -> bool {
    if !matches!(codec(object), Codec::Ogg | Codec::Tracks) {
        return true;
    }
    let Some(k) = object.windows(4).rposition(|w| w == b"OggS") else { return true };
    match object.get(k + 6..k + 14).map(|g| i64::from_le_bytes(g.try_into().unwrap_or([0; 8]))) {
        Some(g) if g > 0 => (frames as i64 - g).abs() <= 4096,
        _ => true,
    }
}

/// An Ogg Vorbis file decoded (lewton): channels, rate, interleaved samples. A stream that breaks off keeps what
/// decoded before.
pub fn decode_ogg(file: &[u8]) -> Result<(u16, u32, Vec<i16>)> {
    // (lewton is not known to panic on bad data, but a parser must not: a panic is an error here.)
    std::panic::catch_unwind(|| -> Result<(u16, u32, Vec<i16>)> {
        let mut r = lewton::inside_ogg::OggStreamReader::new(std::io::Cursor::new(file)).map_err(|e| anyhow::anyhow!("Ogg Vorbis headers: {e}"))?;
        let ch = r.ident_hdr.audio_channels as u16;
        let rate = r.ident_hdr.audio_sample_rate;
        ensure!((1..=8).contains(&ch), "{ch} channels");
        let mut out = Vec::new();
        loop {
            match r.read_dec_packet_itl() {
                Ok(Some(p)) => out.extend(p),
                Ok(None) => break,
                Err(_) if !out.is_empty() => break,
                Err(e) => bail!("Ogg Vorbis: {e}"),
            }
        }
        Ok((ch, rate, out))
    })
    .unwrap_or_else(|_| Err(anyhow::anyhow!("Ogg Vorbis decoder panicked")))
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

/// A track header of this size is the start of a mono Ogg Vorbis stream (its header pages, cut at that size), not an
/// IMA header.
const OGG_TRACK_HEADER: usize = 0x1400;

/// Interleaved tracks (`Codec::Tracks`), decoded: the tracks' channels side by side. Layout from object +4: u16 8, u16
/// 4, u32 ?, u32 track count, u32 block count, u32 block header size (8 + 4 a track), u32 ?, u32 largest block, a u32
/// header size per track, the tracks' headers; then the blocks: u32 ?, u32 block size (header included; 0 in the last
/// block), a u32 data size per track, then each track's data.
/// - Header size 28: an IMA header (mono, or stereo: one byte a frame, high nibble left); a track's first block opens
///   with its PCM frames.
/// - Header size 0x1400: each track is a mono Ogg Vorbis stream, its first 0x1400 bytes in the header, the rest of its
///   pages split over the blocks (music, 48 kHz).
pub fn decode_tracks(object: &[u8]) -> Result<(u16, Vec<i16>)> {
    ensure!(codec(object) == Codec::Tracks, "not interleaved tracks");
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
    let at = r.at;
    let head_size = r.u32()? as usize;
    for _ in 1..tracks {
        r.expect(head_size as u32, "track header size")?;
    }
    let ogg = match head_size {
        IMA_HEADER => false,
        OGG_TRACK_HEADER => true,
        n => bail!("track header size {n:#x} at {at:#x}"),
    };
    // Each track's bytes: its header, then its data block by block.
    let mut heads = Vec::with_capacity(tracks);
    for _ in 0..tracks {
        heads.push(r.take(head_size)?);
    }
    let mut data: Vec<Vec<u8>> = vec![Vec::new(); tracks];
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
            data[t].extend_from_slice(r.take(n).with_context(|| format!("block {b} at {start:#x}"))?);
        }
        r.at = start + size.max(block_header);
        ensure!(r.at <= object.len(), "block {b} at {start:#x} of {size} bytes runs past the end");
    }
    let mut chans: Vec<Vec<i16>> = Vec::new();
    for (t, (head, d)) in heads.iter().zip(&data).enumerate() {
        if ogg {
            let mut file = ogg_pages(head).to_vec();
            file.extend_from_slice(d);
            let (ch, _, pcm) = decode_ogg(&file).with_context(|| format!("track {t}"))?;
            let ch = ch as usize;
            chans.extend((0..ch).map(|c| pcm.iter().skip(c).step_by(ch).copied().collect()));
            continue;
        }
        let h = ima_header(head).with_context(|| format!("track {t}'s header"))?;
        let ch = if h.stereo { 2 } else { 1 };
        let raw = h.pcm_frames * 2 * ch;
        ensure!(raw <= d.len(), "track {t}'s PCM frames past its data");
        let mut out = vec![Vec::new(); ch];
        for (k, c) in d[..raw].chunks_exact(2).enumerate() {
            out[k % ch].push(i16::from_le_bytes([c[0], c[1]]));
        }
        let mut s = h.state;
        for &c in &d[raw..] {
            if ch == 1 {
                out[0].push(s[0].step(c >> 4));
                out[0].push(s[0].step(c & 15));
            } else {
                out[0].push(s[0].step(c >> 4));
                out[1].push(s[1].step(c & 15));
            }
        }
        chans.extend(out);
    }
    let n = chans.len();
    ensure!((1..=16).contains(&n), "{n} channels");
    let frames = chans.iter().map(Vec::len).min().unwrap_or(0);
    let mut out = Vec::with_capacity(frames * n);
    for f in 0..frames {
        out.extend(chans.iter().map(|c| c[f]));
    }
    Ok((n as u16, out))
}

/// An Ogg track's header bytes as far as they are pages: the whole ones, and the cut one the blocks carry on (the
/// padding after the last page left out).
fn ogg_pages(b: &[u8]) -> &[u8] {
    let mut at = 0;
    while b.get(at..at + 4) == Some(b"OggS") {
        let Some(&segs) = b.get(at + 26) else { return b };
        let Some(table) = b.get(at + 27..at + 27 + segs as usize) else { return b };
        let len = 27 + segs as usize + table.iter().map(|&x| x as usize).sum::<usize>();
        if at + len > b.len() {
            return b;
        }
        at += len;
    }
    &b[..at]
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

    /// A voice's samples, interleaved, its frame count long at most (`None`: a `Raw` resource, or data that does not
    /// decode to the sound's channels).
    pub fn decode(&self, v: &Voice) -> Option<Vec<i16>> {
        let o = self.object(v.resource)?;
        let (ch, mut pcm) = match codec(o) {
            Codec::Ima => decode_ima(o, Some(v.frames as usize)).ok()?,
            _ => decode(o).ok()?,
        };
        pcm.truncate(v.frames as usize * ch as usize);
        (ch == v.channels).then_some(pcm)
    }

    /// The streamed loop a play event starts (a sound of type 6, an ambience's): the stream's id, its tracks' rate and
    /// the gains on the way (dB). `None` for any other event.
    pub fn stream_loop(&self, event: u32) -> Option<StreamLoop> {
        let e = self.object(event).filter(|_| event >> 28 == 1)?;
        if u32_at(e, 8)? != 1 {
            return None;
        }
        let id = u32_at(e, 0xc)?;
        let o = self.object(id).filter(|_| id >> 28 == 2)?;
        if u32_at(o, 8)? != 6 {
            return None;
        }
        let stream = u32_at(o, 0x50).filter(|s| s >> 28 == 5)?;
        let rate = u32_at(o, 0x84).filter(|r| (4000..=96000).contains(r))?;
        Some(StreamLoop { stream, rate, gain_db: fixed(u32_at(o, 0x18)?) })
    }
}

/// See `Bank::stream_loop`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StreamLoop {
    pub stream: u32,
    pub rate: u32,
    pub gain_db: f32,
}

/// CRC32("SoundAmbienceSet").
pub const CLASS_SOUND_AMBIENCE_SET: u32 = 0x3cfd0bbb;
/// CRC32("AssassinAmbienceRegionCellData").
pub const CLASS_AMBIENCE_REGION: u32 = 0xf02fbd93;
/// The class of a sound reference inside other objects (name not found).
const CLASS_SOUND_REF: u32 = 0xbb8e3eec;

/// The sound references in an object body: (event BAO id, `SoundBao` object id), in order.
pub fn sound_refs(body: &[u8]) -> Vec<(u32, u32)> {
    (0..body.len().saturating_sub(11))
        .filter(|&k| u32_at(body, k) == Some(CLASS_SOUND_REF))
        .filter_map(|k| Some((u32_at(body, k + 4)?, u32_at(body, k + 8)?)))
        .filter(|(bao, _)| bao >> 28 == 1)
        .collect()
}

/// An `AssassinAmbienceRegionCellData` body's `SoundAmbienceSet` (its object id).
pub fn ambience_region_set(body: &[u8]) -> Option<u32> {
    u32_at(body, 8)
}

/// A city's ambience regions (see the module header).
#[derive(Clone, Debug)]
pub struct Ambience {
    pub layout: crate::world::GridLayout,
    pub levels: u32,
    /// (grid cell over all levels, the region's `SoundAmbienceSet` object id).
    pub regions: Vec<(usize, u32)>,
    /// `SoundAmbienceSet` object id -> (name, the event BAO ids of its sound references).
    pub sets: HashMap<u32, (String, Vec<u32>)>,
}

impl Ambience {
    /// The regions whose cell holds game-space point (x, y): (set, the cell's grid level), each set once at its finest.
    /// (A region is linked from the cells its entity is in, a big one from a coarse cell: the cell is not its area,
    /// which is not read.)
    pub fn sets_at(&self, x: f32, y: f32) -> Vec<(u32, u32)> {
        let mut out: Vec<(u32, u32)> = vec![];
        for &(cell, set) in &self.regions {
            let Some((l, c, r)) = self.layout.cell(cell, self.levels) else { continue };
            let (min, max) = self.layout.rect(l, c, r);
            if !((min[0]..max[0]).contains(&x) && (min[1]..max[1]).contains(&y)) {
                continue;
            }
            match out.iter_mut().find(|o| o.0 == set) {
                Some(o) => o.1 = o.1.min(l),
                None => out.push((set, l)),
            }
        }
        out
    }
}

/// A city's ambience from its forge: the grid from its world file (`world_file`, the city's name), the regions and
/// sets from every `Cell<k>_DataBlock`; the `SoundBao`s of the cells holding either go into `bank`. Reads (and
/// decompresses) every cell: seconds, for a thread.
pub fn load_ambience(forge: &crate::Forge, world_file: &str, bank: &mut Bank) -> Result<Ambience> {
    let e = forge.entries.iter().find(|e| e.name.eq_ignore_ascii_case(world_file)).with_context(|| format!("no world file {world_file}"))?;
    let data = forge.read(e)?;
    let objs = crate::parse_objects(&data)?;
    let world = objs.iter().find(|o| o.class == crate::world::CLASS_WORLD).context("no World")?;
    let layout = crate::world::grid_layout(world.body)?;
    let part = objs.iter().find(|o| o.class == crate::world::CLASS_GRID_PARTITION).context("no GridPartition")?;
    let levels = crate::world::grid_levels(part.body, &layout)?;
    let mut regions = vec![];
    let mut sets = HashMap::new();
    for e in &forge.entries {
        let Some(cell) = e.name.strip_prefix("Cell").and_then(|n| n.strip_suffix("_DataBlock")).and_then(|n| n.parse::<usize>().ok()) else { continue };
        let Ok(data) = forge.read(e) else { continue };
        let Ok(objs) = crate::parse_objects(&data) else { continue };
        let mut keep = false;
        for o in &objs {
            match o.class {
                CLASS_AMBIENCE_REGION => {
                    if let Some(set) = ambience_region_set(o.body) {
                        regions.push((cell, set));
                    }
                    keep = true;
                }
                CLASS_SOUND_AMBIENCE_SET => {
                    sets.entry(o.id).or_insert_with(|| (o.name.clone(), sound_refs(o.body).into_iter().map(|r| r.0).collect()));
                    keep = true;
                }
                _ => {}
            }
        }
        if keep {
            for o in objs.iter().filter(|o| o.class == CLASS_SOUND_BAO) {
                let _ = bank.add(o.id, o.body);
            }
        }
    }
    Ok(Ambience { layout, levels, regions, sets })
}

/// The name of the streamed data file holding stream `id` (in `DataPC_StreamedSounds.forge`).
pub fn stream_file_name(id: u32) -> String {
    format!("Common_BAO_{id:#010x}")
}

/// Interleaved tracks as stereo: a lone track to both sides, two as left and right, four (the ambience's: front left,
/// front right, back left, back right, as quad sound is usually laid out; not checked) folded front over back.
pub fn tracks_to_stereo(tracks: u16, samples: &[i16]) -> Vec<i16> {
    let t = tracks.max(1) as usize;
    samples
        .chunks_exact(t)
        .flat_map(|f| match t {
            1 => [f[0], f[0]],
            2 | 3 => [f[0], f[1]],
            _ => {
                let mix = |a: i16, b: i16| ((a as i32 + b as i32) / 2) as i16;
                [mix(f[0], f[2]), mix(f[1], f[3])]
            }
        })
        .collect()
}

/// CRC32("CollisionMaterial"): a collision surface's kind (`shape::MeshShape::materials` name them per triangle).
pub const CLASS_COLLISION_MATERIAL: u32 = 0x74f7311d;
/// The footstep switches' cases (the events' switch 0) run 0 ..= 24.
pub const SOUND_MATERIALS: u32 = 25;

/// A `CollisionMaterial` body's sound material (+0 u32: `Stone_Clean` 4, `Stone_Dirt` 5, `Marble` 6, `Wood_Plank` 9,
/// `Grass_Short` 14, `Water_Shallow` 19, `Metal_Grill` 24; `Default` holds 4001, no material): the value of the
/// footstep events' switch 0. `None` past the switches' cases.
pub fn collision_sound_material(body: &[u8]) -> Option<u32> {
    u32_at(body, 0).filter(|&m| m < SOUND_MATERIALS)
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
    fn ambience_parts_read() {
        assert_eq!(crc32fast::hash(b"SoundAmbienceSet"), CLASS_SOUND_AMBIENCE_SET);
        assert_eq!(crc32fast::hash(b"AssassinAmbienceRegionCellData"), CLASS_AMBIENCE_REGION);
        let mut b = vec![1, 2, 3];
        for v in [CLASS_SOUND_REF, 0x1055_0013, 0x0bc6_6999, CLASS_SOUND_REF, 0x2000_0001, 7] {
            b.extend(v.to_le_bytes());
        }
        assert_eq!(sound_refs(&b), vec![(0x1055_0013, 0x0bc6_6999)], "event references only");
        assert_eq!(tracks_to_stereo(4, &[2, 4, 6, 8, 1, 1, 1, 1]), vec![4, 6, 1, 1]);
        assert_eq!(tracks_to_stereo(1, &[5, 6]), vec![5, 5, 6, 6]);
        assert_eq!(stream_file_name(0x5118_7015), "Common_BAO_0x51187015");
    }

    #[test]
    fn collision_materials_give_their_sound_material() {
        assert_eq!(crc32fast::hash(b"CollisionMaterial"), CLASS_COLLISION_MATERIAL);
        assert_eq!(collision_sound_material(&[4, 0, 0, 0, 0]), Some(4));
        assert_eq!(collision_sound_material(&4001u32.to_le_bytes()), None);
        assert_eq!(collision_sound_material(&[4, 0]), None);
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
    fn stereo_tracks_and_ogg_track_headers() {
        // one stereo IMA track: a PCM frame, then a byte a frame (high nibble left)
        let mut o = vec![0; 4];
        for v in [0x0004_0008u32, 0x25, 1, 1, 12, 0x80, 0x200, 28] {
            o.extend(v.to_le_bytes());
        }
        let mut h = [0u8; IMA_HEADER];
        h[0] = 5;
        h[1..8].copy_from_slice(&[0xdb, 0x74, 0x11, 0x6c, 0x5e, 0x44, 0x47]);
        h[12] = 1;
        h[14] = 1;
        o.extend(h);
        for v in [3u32, 0, 4 + 1] {
            o.extend(v.to_le_bytes());
        }
        o.extend([1, 0, 2, 0, 0x77]);
        assert_eq!(decode_tracks(&o).unwrap(), (2, vec![1, 2, 13, 13]), "code 7 from each header predictor (0)");
        // (an IMA header with a time in it is still IMA)
        assert_eq!(codec(&ima_object(true, &[1, 2], None, &[0x77])), Codec::Ima);
        let mut t = ima_object(false, &[0], None, &[]);
        t[5..8].copy_from_slice(&[0xdb, 0x74, 0x11]);
        assert_eq!(codec(&t), Codec::Ima);
        t[22] = 89;
        assert_eq!(codec(&t), Codec::Raw, "a step index past the table is not a header");

        // an Ogg track header: whole pages kept, a cut page kept for the blocks to finish, padding dropped
        let page = |body: usize| {
            let mut p = b"OggS".to_vec();
            p.resize(26, 0);
            p.push(1);
            p.push(body as u8);
            p.resize(28 + body, 7);
            p
        };
        let mut head = page(10);
        head.extend(page(20));
        let whole = head.len();
        head.resize(whole + 9, 0);
        assert_eq!(ogg_pages(&head).len(), whole);
        let mut cut = page(10);
        cut.extend(&page(200)[..40]);
        assert_eq!(ogg_pages(&cut).len(), cut.len());
        assert!(decode_ogg(&head).is_err(), "not Vorbis");
        assert!(!complete(
            &{
                let mut x = vec![0; 4];
                x.extend(b"OggS  ");
                x.extend(100_000i64.to_le_bytes());
                x
            },
            10
        ));
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
