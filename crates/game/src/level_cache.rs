//! A city's gameplay data saved on disk after its first load (`spawn_level`): the collision triangles and their sound
//! materials, the holds, AC1's authored guidance, ladders, swing bars, kiosk pieces, beams and perches, haystacks,
//! benches, viewpoints, the navigation mesh, the start and the crowd's loop: everything `Level` holds once the city is
//! built (the raycast grid is rebuilt from the triangles on load). Building it reads every data file of the city's
//! forge and probes hundreds of thousands of holds (Damascus: about 30 s); reading it back takes well under a second.
//!
//! It is derived from the user's own install and never shipped: it lives beside the executable
//! (`<exe dir>/ac1-cache/<city>-<variant>.level`, inside the git-ignored `target/`), or in `AC1_CACHE_DIR`.
//! `AC1_NO_CACHE=1` neither reads nor writes it. `AC1_CACHE_CHECK=1` builds the city afresh anyway and compares it, byte
//! for byte, with what the cache holds (logged as `level cache check: ...`).
//!
//! Layout (little-endian): `b"AC1LEVEL"`, u32 `FORMAT`, u64 key, u64 payload length, u64 payload checksum, then the
//! payload (`write_level`). The key hashes `CACHE_VERSION`, the source of the code deriving the data (`SOURCES`), the
//! city, the env switches that change what is built (`VARIANT_ENV`), and the size and modification time of every forge
//! the build reads. Anything that does not match (another key, a short or corrupt file, a bad checksum, a count past the
//! end) is a miss: the city is built again and the file rewritten; reading never panics.

use crate::level::{Authored, CityObject, HayStack, Kiosk, Ladder, Ledge, Level, Line, Tri};
use anyhow::{Result, bail, ensure};
use bevy::prelude::*;
use std::path::{Path, PathBuf};

/// Bump whenever the code that builds a city's gameplay data changes what it builds (collision, holds, guidance, beams,
/// perches, haystacks, ladders, bars, navigation, sound materials) in a file `SOURCES` does not list, or the payload's
/// layout changes. (Changes in the listed sources invalidate the cache on their own.)
pub const CACHE_VERSION: u32 = 1;

/// The payload layout's version (in the header).
const FORMAT: u32 = 1;

pub(crate) const MAGIC: &[u8; 8] = b"AC1LEVEL";

/// No coordinate of a city is this far out (m); a number past it in the payload is corrupt.
const MAX_COORD: f32 = 30_000.0;

/// A number of the payload as it may be: finite, within `MAX_COORD`.
fn in_range(v: f32) -> bool {
    v.is_finite() && v.abs() <= MAX_COORD
}

/// The code a city's gameplay data comes from: any change to it changes the key (a safety net under `CACHE_VERSION`).
const SOURCES: &[&str] = &[
    include_str!("city.rs"),
    include_str!("level.rs"),
    include_str!("level_cache.rs"),
    include_str!("nav.rs"),
    include_str!("beam_pairs.rs"),
    include_str!("../../forge/src/lib.rs"),
    include_str!("../../forge/src/read.rs"),
    include_str!("../../forge/src/datafile.rs"),
    include_str!("../../forge/src/guidance.rs"),
    include_str!("../../forge/src/index.rs"),
    include_str!("../../forge/src/mesh.rs"),
    include_str!("../../forge/src/navmesh.rs"),
    include_str!("../../forge/src/shape.rs"),
    include_str!("../../forge/src/skeleton.rs"),
    include_str!("../../forge/src/sound.rs"),
];

/// Env switches that change what a city's build gives: each value makes its own cache file. (`AC1_BIND_PROPS` leaves
/// skinned props in their bind pose, which moves their holds, beams and haystacks.)
pub(crate) const VARIANT_ENV: &[&str] = &["AC1_COLLISION", "AC1_GEOMETRY_HOLDS", "AC1_OLD_BEAMS", "AC1_NO_LIPS", "AC1_POSE_PROPS", "AC1_BIND_PROPS"];

/// Env switches whose output comes from building the city (debug meshes drawn, logs while placing): with one set the
/// cache is not used.
const BYPASS_ENV: &[&str] = &["AC1_NO_CACHE", "AC1_SHOW_COLLISION", "AC1_COLLISION_SWEEP", "AC1_MESHES_AT", "AC1_PROP_STATS"];

/// The forges a city's build reads (its own, the shared props', Masyaf's village collision, DataPC's Game Fix).
const FORGES: &[&str] = &["DataPC_Common.forge", "DataPC_Kingdom.forge", "DataPC.forge"];

/// A 64-bit hash (FNV-1a's offset, a multiply-rotate over 8-byte words): the key and the payload's checksum.
#[derive(Clone, Copy)]
pub struct Hash64(u64);

impl Default for Hash64 {
    fn default() -> Self {
        Hash64(0xcbf2_9ce4_8422_2325)
    }
}

impl Hash64 {
    pub fn bytes(mut self, b: &[u8]) -> Self {
        let mut words = b.chunks_exact(8);
        for w in &mut words {
            let mut a = [0u8; 8];
            a.copy_from_slice(w);
            self.0 = (self.0 ^ u64::from_le_bytes(a)).wrapping_mul(0x9e37_79b9_7f4a_7c15).rotate_left(29);
        }
        for &x in words.remainder() {
            self.0 = (self.0 ^ x as u64).wrapping_mul(0x0000_0100_0000_01b3);
        }
        // (The length too: "ab" + "c" and "a" + "bc" differ.)
        self.0 = (self.0 ^ b.len() as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
        self
    }
    pub fn finish(self) -> u64 {
        self.0 ^ (self.0 >> 31)
    }
}

/// Whether the cache is used this run (`BYPASS_ENV`).
pub fn enabled() -> bool {
    BYPASS_ENV.iter().all(|k| std::env::var(k).is_err())
}

/// `AC1_CACHE_CHECK=1`: build afresh and compare with the cache.
pub fn check() -> bool {
    std::env::var("AC1_CACHE_CHECK").is_ok()
}

fn dir() -> Option<PathBuf> {
    if let Ok(d) = std::env::var("AC1_CACHE_DIR") {
        return Some(PathBuf::from(d));
    }
    Some(std::env::current_exe().ok()?.parent()?.join("ac1-cache"))
}

/// What a kind of cache file is made from: its file extension, version, the code deriving it and the env switches that
/// change it (the level here; the drawn city, `render_cache`).
pub(crate) struct CacheKind {
    pub ext: &'static str,
    pub version: u32,
    pub sources: &'static [&'static str],
    pub variant_env: &'static [&'static str],
}

/// The city's gameplay data.
const LEVEL: CacheKind = CacheKind { ext: "level", version: CACHE_VERSION, sources: SOURCES, variant_env: VARIANT_ENV };

/// The cache file of `city` (lowercase) under the env switches set now, and its key over the game files.
pub fn locate(game_dir: &Path, city: &str) -> Option<(PathBuf, u64)> {
    locate_kind(game_dir, city, &LEVEL)
}

/// `locate` for a kind of cache file.
pub(crate) fn locate_kind(game_dir: &Path, city: &str, kind: &CacheKind) -> Option<(PathBuf, u64)> {
    let city = city.to_ascii_lowercase();
    let mut variant = Hash64::default();
    for k in kind.variant_env {
        variant = variant.bytes(k.as_bytes()).bytes(std::env::var(k).unwrap_or_else(|_| "\u{0}unset".into()).as_bytes());
    }
    let variant = variant.finish() as u32;
    let mut key = Hash64::default().bytes(kind.ext.as_bytes()).bytes(&kind.version.to_le_bytes()).bytes(&FORMAT.to_le_bytes()).bytes(city.as_bytes());
    for s in kind.sources {
        key = key.bytes(s.as_bytes());
    }
    key = key.bytes(&variant.to_le_bytes());
    let own = crate::city::forge_of(&city);
    for f in std::iter::once(own.as_str()).chain(FORGES.iter().copied()) {
        // (Size and modification time, to the nanosecond; a forge not there counts as one.)
        let (len, secs, nanos) = std::fs::metadata(game_dir.join(f))
            .ok()
            .map(|m| {
                let t = m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).unwrap_or_default();
                (m.len(), t.as_secs(), t.subsec_nanos())
            })
            .unwrap_or((u64::MAX, 0, 0));
        key = key.bytes(f.as_bytes()).bytes(&len.to_le_bytes()).bytes(&secs.to_le_bytes()).bytes(&nanos.to_le_bytes());
    }
    let safe: String = city.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    Some((dir()?.join(format!("{safe}-{variant:08x}.{}", kind.ext)), key.finish()))
}

/// The payload of the cache file at `path` if it is whole and was made under `key`.
pub fn read_payload(path: &Path, key: u64) -> Result<Vec<u8>> {
    read_payload_as(MAGIC, path, key)
}

/// `read_payload` of a file starting with `magic`.
pub(crate) fn read_payload_as(magic: &[u8; 8], path: &Path, key: u64) -> Result<Vec<u8>> {
    let mut d = std::fs::read(path)?;
    ensure!(d.len() >= 36 && &d[..8] == magic, "not a {} cache", String::from_utf8_lossy(magic));
    let u32_at = |o: usize| u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]]);
    let u64_at = |o: usize| u64::from(u32_at(o)) | (u64::from(u32_at(o + 4)) << 32);
    ensure!(u32_at(8) == FORMAT, "format {} (this build reads {FORMAT})", u32_at(8));
    ensure!(u64_at(12) == key, "made from other game files, code or settings");
    let (len, sum) = (u64_at(20), u64_at(28));
    ensure!(len == (d.len() - 36) as u64, "payload of {len} bytes, the file holds {}", d.len() - 36);
    // (The payload in place, not copied: hundreds of megabytes.)
    d.drain(..36);
    ensure!(Hash64::default().bytes(&d).finish() == sum, "checksum does not match");
    Ok(d)
}

/// Write `payload` under `key` to `path`: to a file of its own first, then renamed over it (several games loading the city
/// at once each write whole files; a reader never sees half of one).
pub(crate) fn write_payload_as(magic: &[u8; 8], path: &Path, key: u64, payload: &[u8]) -> Result<()> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)?;
        remove_stale_temporaries(d);
    }
    let mut out = Vec::with_capacity(payload.len() + 36);
    out.extend(magic);
    out.extend(FORMAT.to_le_bytes());
    out.extend(key.to_le_bytes());
    out.extend((payload.len() as u64).to_le_bytes());
    out.extend(Hash64::default().bytes(payload).finish().to_le_bytes());
    out.extend(payload);
    let tmp = path.with_extension(format!("tmp{}", std::process::id()));
    std::fs::write(&tmp, &out)?;
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        bail!("rename: {e}");
    }
    Ok(())
}

/// Half-written files (`*.tmp<pid>`) a game left when it was closed while writing, an hour old or more.
fn remove_stale_temporaries(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let stale = e.metadata().ok().and_then(|m| m.modified().ok()).and_then(|t| t.elapsed().ok()).is_some_and(|age| age.as_secs() > 3600);
        let temporary = e.path().extension().and_then(|x| x.to_str()).is_some_and(|x| x.starts_with("tmp"));
        if stale && temporary {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

/// Cache files being written on threads of their own (`write_in_background`), waited for as the game closes.
static WRITERS: std::sync::Mutex<Vec<std::thread::JoinHandle<()>>> = std::sync::Mutex::new(vec![]);

/// `write_payload_as` on a thread of its own (a city's cache file is hundreds of megabytes: seconds of the first load
/// on a busy disk), logged when done; `finish_writes` waits for it.
pub(crate) fn write_in_background(magic: &'static [u8; 8], path: PathBuf, key: u64, payload: Vec<u8>, what: String) {
    let job = std::thread::spawn(move || match write_payload_as(magic, &path, key, &payload) {
        Ok(()) => info!("{what} {}: saved", path.display()),
        Err(e) => warn!("{what} {}: not saved: {e:#}", path.display()),
    });
    if let Ok(mut w) = WRITERS.lock() {
        w.push(job);
    }
}

/// Wait for the cache files still being written (the game closing).
pub fn finish_writes() {
    let jobs = WRITERS.lock().map(|mut w| std::mem::take(&mut *w)).unwrap_or_default();
    for j in jobs {
        let _ = j.join();
    }
}

/// Timing of a load's phases (`load: <phase> <s>` lines; `AC1_FPS=1` logs the time to the first frame).
pub struct Lap(std::time::Instant);

impl Lap {
    pub fn start() -> Self {
        Lap(std::time::Instant::now())
    }
    /// Log the time since the last lap as `what`'s.
    pub fn lap(&mut self, what: &str) {
        info!("load: {what} {:.2} s", self.0.elapsed().as_secs_f32());
        self.0 = std::time::Instant::now();
    }
}

// ---- The payload ----

pub(crate) struct W(pub Vec<u8>);

impl W {
    pub fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    pub fn u32(&mut self, v: u32) {
        self.0.extend(v.to_le_bytes());
    }
    pub fn len(&mut self, n: usize) {
        self.0.extend((n as u64).to_le_bytes());
    }
    pub fn f32(&mut self, v: f32) {
        self.0.extend(v.to_bits().to_le_bytes());
    }
    pub fn v3(&mut self, v: Vec3) {
        self.f32(v.x);
        self.f32(v.y);
        self.f32(v.z);
    }
    pub fn bool(&mut self, v: bool) {
        self.u8(v as u8);
    }
    pub fn str(&mut self, s: &str) {
        self.len(s.len());
        self.0.extend(s.as_bytes());
    }
    /// A run of bytes: its length, then them.
    pub fn bytes(&mut self, b: &[u8]) {
        self.len(b.len());
        self.0.extend(b);
    }
    /// A run of u32s: their count, then them.
    pub fn u32s(&mut self, v: &[u32]) {
        self.len(v.len());
        self.0.reserve(v.len() * 4);
        for x in v {
            self.0.extend(x.to_le_bytes());
        }
    }
    pub fn vec<T>(&mut self, items: &[T], mut f: impl FnMut(&mut Self, &T)) {
        self.len(items.len());
        for t in items {
            f(self, t);
        }
    }
    pub fn opt<T>(&mut self, v: Option<&T>, f: impl FnOnce(&mut Self, &T)) {
        self.bool(v.is_some());
        if let Some(v) = v {
            f(self, v);
        }
    }
    fn ledge(&mut self, l: &Ledge) {
        self.v3(l.a);
        self.v3(l.b);
        self.v3(l.out);
    }
    fn line(&mut self, l: &Line) {
        self.v3(l.a);
        self.v3(l.b);
    }
}

/// A bounds-checked reader over the payload; every error names its offset.
pub(crate) struct R<'a> {
    pub d: &'a [u8],
    pub p: usize,
}

impl<'a> R<'a> {
    pub fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.p.checked_add(n).filter(|&e| e <= self.d.len());
        let Some(end) = end else { bail!("level cache ends early at {:#x} (wanted {n} bytes)", self.p) };
        let s = &self.d[self.p..end];
        self.p = end;
        Ok(s)
    }
    pub fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    pub fn u32(&mut self) -> Result<u32> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    pub fn u64(&mut self) -> Result<u64> {
        let b = self.take(8)?;
        let mut a = [0u8; 8];
        a.copy_from_slice(b);
        Ok(u64::from_le_bytes(a))
    }
    /// A count of items at least `min_size` bytes each: never more than the bytes left could hold (no huge allocation
    /// from a corrupt count).
    pub fn len(&mut self, min_size: usize) -> Result<usize> {
        let at = self.p;
        let n = self.u64()?;
        let left = (self.d.len() - self.p) as u64;
        if n.checked_mul(min_size.max(1) as u64).is_none_or(|b| b > left) {
            bail!("level cache count {n} at {at:#x} runs past its end");
        }
        Ok(n as usize)
    }
    /// A coordinate or size: finite and within `MAX_COORD` (a corrupt one could make the grids loop for ever).
    pub fn f32(&mut self) -> Result<f32> {
        let at = self.p;
        let v = f32::from_bits(self.u32()?);
        if !in_range(v) {
            bail!("level cache number {v} at {at:#x} is out of range");
        }
        Ok(v)
    }
    pub fn v3(&mut self) -> Result<Vec3> {
        Ok(Vec3::new(self.f32()?, self.f32()?, self.f32()?))
    }
    /// A number as written, whatever it is (an infinite distance, a box left empty at `f32::MAX`).
    pub fn f32_raw(&mut self) -> Result<f32> {
        Ok(f32::from_bits(self.u32()?))
    }
    /// A run of bytes (`W::bytes`).
    pub fn bytes(&mut self) -> Result<&'a [u8]> {
        let n = self.len(1)?;
        self.take(n)
    }
    /// A run of u32s (`W::u32s`).
    pub fn u32s(&mut self) -> Result<Vec<u32>> {
        let n = self.len(4)?;
        Ok(self.take(n * 4)?.chunks_exact(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect())
    }
    pub fn bool(&mut self) -> Result<bool> {
        let at = self.p;
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            v => bail!("level cache flag {v} at {at:#x}"),
        }
    }
    pub fn str(&mut self) -> Result<String> {
        let n = self.len(1)?;
        let at = self.p;
        String::from_utf8(self.take(n)?.to_vec()).map_err(|_| anyhow::anyhow!("level cache text at {at:#x} is not UTF-8"))
    }
    pub fn vec<T>(&mut self, min_size: usize, mut f: impl FnMut(&mut Self) -> Result<T>) -> Result<Vec<T>> {
        let n = self.len(min_size)?;
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            out.push(f(self)?);
        }
        Ok(out)
    }
    pub fn opt<T>(&mut self, f: impl FnOnce(&mut Self) -> Result<T>) -> Result<Option<T>> {
        Ok(if self.bool()? { Some(f(self)?) } else { None })
    }
    fn ledge(&mut self) -> Result<Ledge> {
        Ok(Ledge { a: self.v3()?, b: self.v3()?, out: self.v3()? })
    }
    /// An index below `n`.
    pub fn index(&mut self, n: usize) -> Result<usize> {
        let at = self.p;
        let i = self.u64()?;
        if i >= n as u64 {
            bail!("level cache index {i} at {at:#x} is past {n}");
        }
        Ok(i as usize)
    }
    /// Triangles, in one go (millions of them).
    fn tris(&mut self) -> Result<Vec<[Vec3; 3]>> {
        let n = self.len(36)?;
        let at = self.p;
        let b = self.take(n * 36)?;
        let mut bad = false;
        let f = |c: &[u8]| {
            let v = f32::from_le_bytes([c[0], c[1], c[2], c[3]]);
            bad |= !in_range(v);
            v
        };
        let mut f = f;
        let out: Vec<[Vec3; 3]> = b
            .chunks_exact(36)
            .map(|t| {
                let mut v = t.chunks_exact(4).map(&mut f);
                let mut p = || Vec3::new(v.next().unwrap_or(0.0), v.next().unwrap_or(0.0), v.next().unwrap_or(0.0));
                [p(), p(), p()]
            })
            .collect();
        if bad {
            bail!("level cache triangles at {at:#x} hold a number out of range");
        }
        Ok(out)
    }
    fn line(&mut self) -> Result<Line> {
        Ok(Line { a: self.v3()?, b: self.v3()? })
    }
}

fn sub_type(v: u32, at: usize) -> Result<forge::guidance::SubType> {
    forge::guidance::SubType::from_u32(v).ok_or_else(|| anyhow::anyhow!("level cache guidance kind {v} at {at:#x}"))
}

fn city_object(v: u8, at: usize) -> Result<CityObject> {
    Ok(match v {
        0 => CityObject::Hay,
        1 => CityObject::HidingSpot,
        2 => CityObject::Ladder,
        3 => CityObject::Bench,
        4 => CityObject::Pole,
        _ => bail!("level cache object kind {v} at {at:#x}"),
    })
}

fn city_object_u8(k: CityObject) -> u8 {
    match k {
        CityObject::Hay => 0,
        CityObject::HidingSpot => 1,
        CityObject::Ladder => 2,
        CityObject::Bench => 3,
        CityObject::Pole => 4,
    }
}

/// A built level's gameplay data as the cache's payload (every field of `Level` but the raycast grid, which
/// `build_grid` makes from the triangles; the navigation mesh as its triangles and their links).
pub fn write_level(level: &Level) -> Vec<u8> {
    // (Every field named, none skipped by `..`: a field added to `Level` stops the build here until it is saved too.)
    let Level {
        nav,
        tris,
        ledges,
        authored,
        authored_src,
        authored_src_at: _,
        authored_ladders,
        authored_holds,
        city_objects,
        hay_shapes,
        lip_candidates,
        interest,
        haystacks,
        benches,
        viewpoints,
        perches,
        bars,
        ladders,
        kiosks,
        grid: _,
        big: _,
        report_edges: _,
        report_grid: _,
        hold_grid: _,
        hold_grid_len: _,
        spawn,
        crowd_path,
        city,
        beam_tops,
        labels,
        tri_sound,
    } = level;
    let mut w = W(Vec::with_capacity(tris.len() * 36 + ledges.len() * 36 + authored.len() * 56 + (1 << 20)));
    w.opt(nav.as_ref(), |w, n| {
        w.vec(n.tris(), |w, t| t.iter().for_each(|v| w.v3(*v)));
        w.vec(n.links(), |w, l| {
            w.vec(l, |w, (j, mid)| {
                w.len(*j);
                w.v3(*mid);
            });
        });
    });
    w.vec(tris, |w, t| {
        w.v3(t.a);
        w.v3(t.b);
        w.v3(t.c);
    });
    w.vec(ledges, W::ledge);
    w.vec(authored, |w, e| {
        w.u32(e.kind as u32);
        w.v3(e.a);
        w.v3(e.b);
        w.v3(e.out);
        w.v3(e.axis);
        w.u32(e.src);
    });
    w.vec(authored_src, |w, s| w.str(s));
    w.bool(*authored_ladders);
    w.bool(*authored_holds);
    w.vec(city_objects, |w, (k, lo, hi, tf)| {
        w.u8(city_object_u8(*k));
        w.v3(*lo);
        w.v3(*hi);
        w.v3(tf.translation);
        let q = tf.rotation;
        [q.x, q.y, q.z, q.w].into_iter().for_each(|v| w.f32(v));
        w.v3(tf.scale);
    });
    w.vec(hay_shapes, |w, (a, b)| {
        w.v3(*a);
        w.v3(*b);
    });
    w.vec(lip_candidates, W::ledge);
    w.vec(interest, |w, v| w.v3(*v));
    w.vec(haystacks, |w, h| {
        w.v3(h.centre);
        w.f32(h.half);
        w.f32(h.height);
        w.v3(h.facing);
        w.opt(h.own.as_ref(), |w, (a, b)| {
            w.v3(*a);
            w.v3(*b);
        });
    });
    w.vec(benches, |w, (a, b, c)| {
        w.v3(*a);
        w.v3(*b);
        w.v3(*c);
    });
    w.vec(viewpoints, |w, v| w.v3(*v));
    w.vec(perches, W::line);
    w.vec(bars, W::line);
    w.vec(ladders, |w, l| {
        w.v3(l.base);
        w.f32(l.top);
        w.v3(l.out);
    });
    w.vec(kiosks, |w, k| {
        w.ledge(&k.edge);
        w.v3(k.axis);
    });
    w.opt(spawn.as_ref(), |w, v| w.v3(*v));
    w.opt(crowd_path.as_ref(), |w, p| w.vec(p, |w, v| w.v3(*v)));
    w.bool(*city);
    w.vec(beam_tops, W::line);
    w.vec(labels, |w, (s, v)| {
        w.str(s);
        w.v3(*v);
    });
    w.vec(tri_sound, |w, s| w.u8(*s));
    w.0
}

/// The level a payload holds (`write_level`), its raycast grid built.
pub fn read_level(d: &[u8]) -> Result<Level> {
    let mut r = R { d, p: 0 };
    let nav = r.opt(|r| {
        let tris = r.tris()?;
        let links = r.vec(8, |r| r.vec(20, |r| Ok((r.index(tris.len())?, r.v3()?))))?;
        ensure!(links.len() == tris.len(), "level cache: {} navigation triangles, links for {}", tris.len(), links.len());
        Ok(crate::nav::NavGraph::with_links(tris, links))
    })?;
    let tris = r.tris()?.into_iter().map(|[a, b, c]| Tri { a, b, c }).collect();
    let ledges = r.vec(36, R::ledge)?;
    let authored = r.vec(56, |r| {
        let at = r.p;
        let kind = sub_type(r.u32()?, at)?;
        Ok(Authored { kind, a: r.v3()?, b: r.v3()?, out: r.v3()?, axis: r.v3()?, src: r.u32()? })
    })?;
    let authored_src = r.vec(8, R::str)?;
    let authored_ladders = r.bool()?;
    let authored_holds = r.bool()?;
    let city_objects = r.vec(65, |r| {
        let at = r.p;
        let k = city_object(r.u8()?, at)?;
        let (lo, hi, translation) = (r.v3()?, r.v3()?, r.v3()?);
        let rotation = Quat::from_xyzw(r.f32()?, r.f32()?, r.f32()?, r.f32()?);
        let scale = r.v3()?;
        Ok((k, lo, hi, Transform { translation, rotation, scale }))
    })?;
    let hay_shapes = r.vec(24, |r| Ok((r.v3()?, r.v3()?)))?;
    let lip_candidates = r.vec(36, R::ledge)?;
    let interest = r.vec(12, R::v3)?;
    let haystacks = r.vec(33, |r| {
        let (centre, half, height, facing) = (r.v3()?, r.f32()?, r.f32()?, r.v3()?);
        let own = r.opt(|r| Ok((r.v3()?, r.v3()?)))?;
        Ok(HayStack { centre, half, height, facing, own })
    })?;
    let benches = r.vec(36, |r| Ok((r.v3()?, r.v3()?, r.v3()?)))?;
    let viewpoints = r.vec(12, R::v3)?;
    let perches = r.vec(24, R::line)?;
    let bars = r.vec(24, R::line)?;
    let ladders = r.vec(28, |r| Ok(Ladder { base: r.v3()?, top: r.f32()?, out: r.v3()? }))?;
    let kiosks = r.vec(48, |r| Ok(Kiosk { edge: r.ledge()?, axis: r.v3()? }))?;
    let spawn = r.opt(R::v3)?;
    let crowd_path = r.opt(|r| r.vec(12, R::v3))?;
    let city = r.bool()?;
    let beam_tops = r.vec(24, R::line)?;
    let labels = r.vec(20, |r| Ok((r.str()?, r.v3()?)))?;
    let tri_sound = r.vec(1, R::u8)?;
    ensure!(r.p == d.len(), "level cache has {} bytes past its end at {:#x}", d.len() - r.p, r.p);
    ensure!(authored.iter().all(|e| (e.src as usize) < authored_src.len()), "level cache: an authored edge's source is past the list");
    let authored_src_at = authored_src.iter().enumerate().map(|(i, s)| (s.clone(), i as u32)).collect();
    let mut level = Level {
        nav,
        tris,
        ledges,
        authored,
        authored_src,
        authored_src_at,
        authored_ladders,
        authored_holds,
        city_objects,
        hay_shapes,
        lip_candidates,
        interest,
        haystacks,
        benches,
        viewpoints,
        perches,
        bars,
        ladders,
        kiosks,
        spawn,
        crowd_path,
        city,
        beam_tops,
        labels,
        tri_sound,
        ..default()
    };
    level.build_grid();
    Ok(level)
}

/// What a built level holds, in a line: counts and the payload's hash (the cache check's log).
pub fn summary(level: &Level, payload: &[u8]) -> String {
    format!(
        "{} triangles, {} holds, {} authored edges, {} perches, {} bars, {} ladders, {} haystacks, {} nav triangles, {} bytes, hash {:016x}",
        level.tris.len(),
        level.ledges.len(),
        level.authored.len(),
        level.perches.len(),
        level.bars.len(),
        level.ladders.len(),
        level.haystacks.len(),
        level.nav.as_ref().map_or(0, |n| n.len()),
        payload.len(),
        Hash64::default().bytes(payload).finish()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Level {
        let v = |x: f32| Vec3::new(x, x * 2.0, -x);
        let mut level = Level {
            tris: (0..50).map(|i| Tri { a: v(i as f32), b: v(i as f32 + 1.0), c: v(i as f32 + 0.5) + Vec3::Y }).collect(),
            ledges: vec![Ledge { a: v(1.0), b: v(2.0), out: Vec3::X }],
            authored: vec![Authored { kind: forge::guidance::SubType::Kiosk, a: v(3.0), b: v(4.0), out: Vec3::Z, axis: Vec3::X, src: 0 }],
            authored_src: vec!["Kiosk\tStall".into()],
            authored_holds: true,
            city_objects: vec![(CityObject::Bench, v(1.0), v(2.0), Transform::from_xyz(1.0, 2.0, 3.0).with_rotation(Quat::from_rotation_y(0.7)))],
            haystacks: vec![HayStack { centre: v(5.0), half: 1.0, height: 1.5, facing: Vec3::X, own: Some((v(4.0), v(6.0))) }],
            perches: vec![Line { a: v(7.0), b: v(8.0) }],
            ladders: vec![Ladder { base: v(9.0), top: 12.0, out: -Vec3::X }],
            kiosks: vec![Kiosk { edge: Ledge { a: v(1.0), b: v(3.0), out: Vec3::Z }, axis: Vec3::X }],
            spawn: Some(v(0.5)),
            crowd_path: Some(vec![v(1.0), v(2.0)]),
            city: true,
            labels: vec![("a label".into(), v(3.0))],
            tri_sound: vec![1, 2, crate::level::NO_SOUND],
            nav: Some(crate::nav::NavGraph::new(vec![[Vec3::ZERO, Vec3::X, Vec3::Z], [Vec3::X, Vec3::X + Vec3::Z, Vec3::Z]])),
            ..default()
        };
        level.build_grid();
        level
    }

    #[test]
    fn round_trip_is_identical() {
        let level = sample();
        let bytes = write_level(&level);
        let back = read_level(&bytes).expect("reads back");
        assert_eq!(write_level(&back), bytes);
        assert_eq!(back.grid.len(), level.grid.len());
        assert_eq!(back.authored_src_at.get("Kiosk\tStall"), Some(&0));
        assert_eq!(back.nav.as_ref().map(|n| n.len()), Some(2));
    }

    #[test]
    fn short_or_corrupt_payloads_are_errors() {
        let bytes = write_level(&sample());
        for cut in [0, 1, 7, 8, bytes.len() / 2, bytes.len() - 1] {
            assert!(read_level(&bytes[..cut]).is_err(), "cut at {cut}");
        }
        // A count far past the end: an error, not an allocation.
        let mut huge = bytes.clone();
        huge[..9].copy_from_slice(&[1, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x7f]);
        assert!(read_level(&huge).is_err());
        // Bytes past the end.
        let mut long = bytes.clone();
        long.push(0);
        assert!(read_level(&long).is_err());
        // Every single flipped byte reads as an error or as some level; never a panic.
        for i in (0..bytes.len()).step_by(7) {
            let mut b = bytes.clone();
            b[i] ^= 0xa5;
            let _ = read_level(&b);
        }
    }

    #[test]
    fn files_are_checked() {
        let dir = std::env::temp_dir().join(format!("ac1-level-cache-test-{}", std::process::id()));
        let path = dir.join("x.level");
        let payload = write_level(&sample());
        write_payload_as(MAGIC, &path, 42, &payload).expect("writes");
        assert_eq!(read_payload(&path, 42).expect("reads"), payload);
        assert!(read_payload(&path, 43).is_err(), "another key");
        let mut file = std::fs::read(&path).expect("file");
        let last = file.len() - 1;
        file[last] ^= 1;
        std::fs::write(&path, &file).expect("rewrite");
        assert!(read_payload(&path, 42).is_err(), "checksum");
        std::fs::write(&path, &file[..20]).expect("rewrite");
        assert!(read_payload(&path, 42).is_err(), "short");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
