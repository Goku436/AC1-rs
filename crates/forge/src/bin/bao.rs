//! bao: the install's sound (see `forge::sound`).
//! - `bao scan <game dir>`: every `SoundBao` object of every forge and every streamed BAO file: how many parse, their
//!   kinds; per codec, how many distinct resources and streams decode, and decode to their end (headerless streams
//!   joined to their start); the failures by message.
//! - `bao list <forge file> <data file>`: the data file's BAOs (id, kind, type or codec, bytes).
//! - `bao wav <game dir> <bao id hex>...`: write a resource, a stream, or what an event plays, as WAV files to
//!   `out/sound/` (git-ignored).
//! - `bao raw <game dir> <bao id>...`: a resource's or stream's object bytes, to `out/sound/<id>.bin`.
//! - `bao clip <game dir> <clip name substring>`: the event keys of the matching clips in "Game Fix", and what Altaïr's
//!   contact table and the sound bank play for each.
//! - `bao tree <game dir> <event id>...`: an event's graph (switches and their cases, multis, plays, random
//!   containers, sounds).
//!
//! - `bao ambience <game dir> <City>`: the city's ambience regions, their sets and the loops they play.
//!
//! `wav`, `clip` and `tree` load `DataPC.forge`'s player sounds; `BAO_EXTRA="<forge file>:<data file>,..."` adds more.
use anyhow::{Context, Result, bail};
use forge::sound::{self, Bank, Codec, Kind};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

fn forges(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)?.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "forge")).collect();
    v.sort();
    Ok(v)
}

fn short(e: &anyhow::Error) -> String {
    let s = format!("{e:#}");
    // offsets vary: keep the message's words
    s.split(" at 0x").next().unwrap_or(&s).chars().take(60).collect()
}

/// The scan's counts: per (kind, codec) the distinct resources and streams, how many decode and decode to their end;
/// the failures by message (count, an example); the seconds decoded.
#[derive(Default)]
struct Tally {
    codecs: BTreeMap<(Kind, Codec), (usize, usize, usize)>,
    errs: BTreeMap<String, (usize, String)>,
    seconds: f64,
}

impl Tally {
    fn fail(&mut self, what: &str, e: &anyhow::Error, name: &str) {
        self.errs.entry(format!("{what}: {}", short(e))).or_insert((0, name.to_string())).0 += 1;
    }
    fn count(&mut self, kind: Kind, object: &[u8], name: &str) {
        let c = sound::codec(object);
        self.codecs.entry((kind, c)).or_default().0 += 1;
        if c == Codec::Raw {
            return;
        }
        match sound::decode(object) {
            Ok((ch, pcm)) => self.decoded(kind, c, object, ch, pcm.len()),
            Err(err) => self.fail(&format!("{kind:?} {c:?}"), &err, name),
        }
    }
    fn decoded(&mut self, kind: Kind, c: Codec, object: &[u8], ch: u16, samples: usize) {
        let frames = samples / ch.max(1) as usize;
        self.seconds += frames as f64 / sound::data_rate(object).unwrap_or(24000) as f64;
        let e = self.codecs.entry((kind, c)).or_default();
        e.1 += 1;
        if sound::complete(object, frames) {
            e.2 += 1;
        }
    }
}

fn scan(dir: &Path) -> Result<()> {
    let (mut objects, mut objects_ok) = (0, 0);
    let mut kinds: BTreeMap<Kind, usize> = BTreeMap::new();
    let (mut streams, mut streams_ok) = (0, 0);
    let mut t = Tally::default();
    // Every resource or stream once (cities repeat the shared ones).
    let mut seen: HashSet<(u32, usize)> = HashSet::new();
    // The streamed files first: the headerless ones are joined to their start below.
    let mut paths = forges(dir)?;
    paths.sort_by_key(|p| !p.to_string_lossy().contains("Streamed"));
    let mut rest: HashMap<u32, (usize, usize)> = HashMap::new();
    let mut opened = Vec::new();
    for (fi, path) in paths.iter().enumerate() {
        let f = forge::Forge::open(path)?;
        for (ei, e) in f.entries.iter().enumerate() {
            if e.name.contains("_BAO_0x") {
                streams += 1;
                let d = f.read(e)?;
                match sound::parse_bao(&d) {
                    Ok(b) => {
                        streams_ok += 1;
                        t.count(b.kind, b.object, &e.name);
                        if sound::codec(b.object) == Codec::Raw
                            && let Some(id) = e.name.rsplit("_BAO_0x").next().and_then(|h| u32::from_str_radix(h, 16).ok())
                        {
                            rest.insert(id & 0x0fff_ffff, (fi, ei));
                        }
                    }
                    Err(err) => t.fail("stream header", &err, &e.name),
                }
                continue;
            }
            let Ok(d) = f.read(e) else { continue };
            let Ok(objs) = forge::parse_objects(&d) else { continue };
            for o in objs.iter().filter(|o| o.class == sound::CLASS_SOUND_BAO) {
                objects += 1;
                let s = match sound::parse_sound_bao(o.body) {
                    Ok(s) => s,
                    Err(err) => {
                        t.fail("SoundBao", &err, &o.name);
                        continue;
                    }
                };
                objects_ok += 1;
                for (vi, (_, b)) in s.variants.iter().enumerate() {
                    *kinds.entry(b.kind).or_default() += 1;
                    if matches!(b.kind, Kind::Resource | Kind::Stream) && seen.insert((s.id, vi)) {
                        t.count(b.kind, b.object, &o.name);
                    }
                }
                // A headerless stream's start: the variant (language) it continues decodes joined to it.
                if s.id >> 28 == 3
                    && let Some((sf, se)) = rest.remove(&(s.id & 0x0fff_ffff))
                {
                    let Some(sf) = opened.get(sf) else { continue };
                    let sf: &forge::Forge = sf;
                    let d = sf.read(&sf.entries[se])?;
                    let stream = sound::parse_bao(&d)?.object;
                    let mut err = None;
                    let mut ok = false;
                    for (_, b) in &s.variants {
                        let joined = sound::join_stream(b.object, stream);
                        match sound::decode(&joined) {
                            Ok((ch, pcm)) if sound::complete(&joined, pcm.len() / ch.max(1) as usize) => {
                                t.decoded(Kind::Stream, Codec::Raw, &joined, ch, pcm.len());
                                ok = true;
                                break;
                            }
                            Ok(_) => err = Some(anyhow::anyhow!("joined: not decoded to the end")),
                            Err(x) => err = Some(x),
                        }
                    }
                    if !ok && let Some(x) = err {
                        t.fail("Stream Raw (joined)", &x, &sf.entries[se].name);
                    }
                }
            }
        }
        eprintln!("{}: {objects} SoundBaos so far", path.display());
        opened.push(f);
    }
    println!("SoundBao objects: {objects_ok}/{objects} parse; BAOs by kind: {kinds:?}");
    println!("streamed BAO files: {streams_ok}/{streams} parse");
    println!("distinct resources and streams by codec: count, decode, decode to the end (Raw streams: joined to their start)");
    for ((k, c), (n, ok, end)) in &t.codecs {
        println!("  {k:?} {c:?}: {n} {ok} {end}");
    }
    println!("about {:.0} h of sound", t.seconds / 3600.0);
    if !rest.is_empty() {
        let mut v: Vec<String> = rest.values().take(5).map(|&(f, e)| opened[f].entries[e].name.clone()).collect();
        v.sort();
        println!("{} headerless streams whose start (the resource with the same low bits) is in no forge, e.g. {v:?}", rest.len());
    }
    for (k, (n, ex)) in t.errs {
        println!("{n:>6} {k}  e.g. {ex}");
    }
    Ok(())
}

fn list(forge_file: &Path, data_file: &str) -> Result<()> {
    let f = forge::Forge::open(forge_file)?;
    let e = f.entries.iter().find(|e| e.name == data_file).with_context(|| format!("no data file {data_file}"))?;
    let d = f.read(e)?;
    for o in forge::parse_objects(&d)?.iter().filter(|o| o.class == sound::CLASS_SOUND_BAO) {
        match sound::parse_sound_bao(o.body) {
            Ok(s) => {
                let b = s.bao();
                let ty = match b.kind {
                    Kind::Event | Kind::Sound => format!("type {}", b.object.get(8).copied().unwrap_or(0)),
                    _ => format!("{:x?}", sound::codec(b.object)),
                };
                println!(
                    "{:08x} {:08x} {:<9} {:<16} {:>8} bytes {} variant(s) deps {:08x?}",
                    o.id,
                    s.id,
                    format!("{:?}", b.kind),
                    ty,
                    b.object.len(),
                    s.variants.len(),
                    s.deps
                );
            }
            Err(e) => println!("{:08x} {}: {e:#}", o.id, o.name),
        }
    }
    Ok(())
}

/// Every SoundBao of `DataPC.forge`'s files that hold the player's sounds, and Altaïr's contact table.
fn load_bank(dir: &Path) -> Result<(Bank, Option<sound::ContactTable>)> {
    let f = forge::Forge::open(dir.join("DataPC.forge"))?;
    let mut bank = Bank::default();
    let mut table = None;
    for name in ["Game Fix", "Game Bootstrap Settings", "Rank 9"] {
        let e = f.entries.iter().find(|e| e.name == name).with_context(|| format!("no data file {name}"))?;
        let d = f.read(e)?;
        bank.add_data_file(&d)?;
        if let Some(o) = forge::parse_objects(&d)?.iter().find(|o| o.class == sound::CLASS_CONTACT_TABLE && o.name == "ContactTable_AlTair") {
            table = Some(sound::parse_contact_table(o.body)?);
        }
    }
    // `BAO_EXTRA="<forge file>:<data file>,..."`: more data files' sounds (a city's world file).
    for spec in std::env::var("BAO_EXTRA").unwrap_or_default().split(',').filter(|s| !s.is_empty()) {
        let (forge_file, data_file) = spec.split_once(':').context("BAO_EXTRA: <forge file>:<data file>")?;
        let f = forge::Forge::open(dir.join(forge_file))?;
        let e = f.entries.iter().find(|e| e.name == data_file).with_context(|| format!("no data file {data_file}"))?;
        bank.add_data_file(&f.read(e)?)?;
    }
    Ok((bank, table))
}

fn write(name: &str, ch: u16, rate: u32, pcm: &[i16]) -> Result<()> {
    std::fs::create_dir_all("out/sound")?;
    let p = Path::new("out/sound").join(format!("{name}.wav"));
    std::fs::write(&p, sound::wav(ch, rate, pcm))?;
    println!("{}: {ch} ch, {rate} Hz, {:.2} s", p.display(), pcm.len() as f32 / ch as f32 / rate as f32);
    Ok(())
}

fn wav(dir: &Path, ids: &[String]) -> Result<()> {
    let (bank, _) = load_bank(dir)?;
    for id in ids {
        let id = u32::from_str_radix(id.trim_start_matches("0x"), 16)?;
        match id >> 28 {
            1 => {
                let mut pick = sound::Picker::new(id);
                for (k, v) in bank.resolve(id, &|_| None, &mut pick).iter().enumerate() {
                    match bank.decode(v) {
                        Some(pcm) => write(&format!("{id:08x}_{k}_{:08x}", v.resource), v.channels, v.rate, &pcm)?,
                        None => println!("{id:08x}: voice {:08x} not IMA", v.resource),
                    }
                }
            }
            3 => {
                let o = bank.object(id).context("no such resource")?;
                let (ch, pcm) = sound::decode(o)?;
                // (the rate is the sound object's; 24 kHz is the most common)
                write(&format!("{id:08x}"), ch, 24000, &pcm)?;
            }
            5 => {
                let mut found = false;
                for p in forges(dir)?.iter().filter(|p| p.to_string_lossy().contains("Streamed")) {
                    let f = forge::Forge::open(p)?;
                    if let Some(e) = f.entries.iter().find(|e| e.name.ends_with(&format!("_BAO_{id:#010x}"))) {
                        let d = f.read(e)?;
                        let (ch, pcm) = sound::decode(sound::parse_bao(&d)?.object)?;
                        write(&format!("{id:08x}"), ch, 24000, &pcm)?;
                        found = true;
                    }
                }
                if !found {
                    bail!("no streamed BAO {id:08x}");
                }
            }
            _ => bail!("{id:08x}: give an event (1...), a resource (3...) or a stream (5...)"),
        }
    }
    Ok(())
}

/// `bao raw <game dir> <bao id>...`: a resource's or a stream's object bytes to `out/sound/<id>.bin`.
fn raw(dir: &Path, ids: &[String]) -> Result<()> {
    let (bank, _) = load_bank(dir)?;
    std::fs::create_dir_all("out/sound")?;
    for id in ids {
        let id = u32::from_str_radix(id.trim_start_matches("0x"), 16)?;
        let bytes = match bank.object(id) {
            Some(o) => o.to_vec(),
            None => {
                let mut found = None;
                for p in forges(dir)?.iter().filter(|p| p.to_string_lossy().contains("Streamed")) {
                    let f = forge::Forge::open(p)?;
                    if let Some(e) = f.entries.iter().find(|e| e.name.ends_with(&format!("_BAO_{id:#010x}"))) {
                        found = Some(sound::parse_bao(&f.read(e)?)?.object.to_vec());
                    }
                }
                found.with_context(|| format!("no BAO {id:08x}"))?
            }
        };
        let p = Path::new("out/sound").join(format!("{id:08x}.bin"));
        std::fs::write(&p, &bytes)?;
        println!("{}: {} bytes", p.display(), bytes.len());
    }
    Ok(())
}

fn clip(dir: &Path, pat: &str) -> Result<()> {
    let (bank, table) = load_bank(dir)?;
    let f = forge::Forge::open(dir.join("DataPC.forge"))?;
    let e = f.entries.iter().find(|e| e.name == "Game Fix").context("no Game Fix")?;
    let d = f.read(e)?;
    let mut pick = sound::Picker::new(1);
    for o in forge::parse_objects(&d)?.iter().filter(|o| o.class == forge::anim::CLASS_ANIMATION && o.name.contains(pat)) {
        let a = forge::anim::parse_animation(o.bytes)?;
        let events = forge::anim::parse_events(o.body);
        println!(
            "{} {:.2} s: {}",
            o.name,
            a.duration,
            events.as_ref().map_or_else(|e| format!("events do not parse: {e:#}"), |e| format!("{} events", e.len()))
        );
        for ev in events.unwrap_or_default() {
            let (what, sb) = match ev.kind {
                forge::anim::EventKind::Contact { ty } => {
                    (format!("contact {}", forge::anim::CONTACT_HUMAN.get(ty as usize).copied().unwrap_or("?")), table.as_ref().and_then(|t| t.sound(None, ty)))
                }
                forge::anim::EventKind::Audio { sound } => ("audio".to_string(), Some(sound)),
                forge::anim::EventKind::Other(c) => (format!("other {c:08x?}"), None),
            };
            let event = sb.and_then(|s| bank.bao_of(s));
            let voices = event.map(|e| bank.resolve(e, &|_| None, &mut pick)).unwrap_or_default();
            let v: Vec<String> =
                voices.iter().map(|v| format!("{:08x} {:.0} dB x{:.2} {:.2} s", v.resource, v.gain_db, v.speed, v.frames as f32 / v.rate as f32)).collect();
            println!("  {:6.3} {what:<24} {:>10} -> {}", ev.time, event.map_or("-".into(), |e| format!("{e:08x}")), v.join(", "));
        }
    }
    Ok(())
}

/// An event's graph: switches with their variable and cases, multis, plays, containers, resources.
fn tree(bank: &Bank, id: u32, depth: usize) {
    let pad = "  ".repeat(depth + 1);
    let u = |o: &[u8], at: usize| o.get(at..at + 4).map_or(0, |b| u32::from_le_bytes(b.try_into().unwrap()));
    let Some(o) = bank.object(id).filter(|_| depth < 24) else {
        println!("{pad}{id:08x} (not loaded, or too deep)");
        return;
    };
    match (id >> 28, u(o, 8)) {
        (1, 1) => {
            println!("{pad}{id:08x} play, pitch range {:.3}", u(o, 0x10) as i32 as f32 / 65536.0);
            tree(bank, u(o, 0xc), depth + 1);
        }
        (1, 11) => {
            println!("{pad}{id:08x} switch on variable {}, default {:08x}", u(o, 0x10), u(o, 0x14));
            for j in 0..u(o, 0x1c).min(64) as usize {
                println!("{pad}  case {}:", u(o, 0x68 + 12 * j + 8));
                tree(bank, u(o, 0x68 + 12 * j), depth + 2);
            }
        }
        (1, 12) => {
            println!("{pad}{id:08x} multi");
            for j in 0..u(o, 0x10).min(64) as usize {
                tree(bank, u(o, 0x68 + 4 * j), depth + 1);
            }
        }
        (2, 1) => println!(
            "{pad}{id:08x} sound {:.0} dB: {:08x}, {} ch, {} Hz, {:.2} s",
            u(o, 0x18) as i32 as f32 / 65536.0,
            u(o, 0x20),
            u(o, 0x48),
            u(o, 0x4c),
            u(o, 0x54) as f32 / u(o, 0x4c).max(1) as f32
        ),
        (2, 4) => {
            println!("{pad}{id:08x} random of {} ({:.0} dB)", u(o, 0x20), u(o, 0x18) as i32 as f32 / 65536.0);
            for j in 0..u(o, 0x20).min(64) as usize {
                tree(bank, u(o, 0x84 + 16 * j), depth + 1);
            }
        }
        (k, t) => {
            println!("{pad}{id:08x} kind {k} type {t}");
            // `BAO_HEX=1`: the bytes of what is not read.
            if std::env::var("BAO_HEX").is_ok() {
                for (i, c) in o[..o.len().min(0x200)].chunks(16).enumerate() {
                    println!("{pad}  {:04x}: {}", i * 16, c.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" "));
                }
            }
        }
    }
}

/// A city's ambience regions, their sets and the loops they play.
fn ambience(dir: &Path, city: &str) -> Result<()> {
    let (mut bank, _) = load_bank(dir)?;
    let f = forge::Forge::open(dir.join(format!("DataPC_{city}.forge")))?;
    let amb = sound::load_ambience(&f, city, &mut bank)?;
    println!(
        "{} regions over {} cells, {} sets",
        amb.regions.len(),
        amb.regions.iter().map(|r| r.0).collect::<std::collections::HashSet<_>>().len(),
        amb.sets.len()
    );
    let mut used: Vec<u32> = amb.regions.iter().map(|r| r.1).collect();
    used.sort();
    used.dedup();
    for set in used {
        let Some((name, events)) = amb.sets.get(&set) else {
            println!("set {set:08x}: not found");
            continue;
        };
        let cells = amb.regions.iter().filter(|r| r.1 == set).count();
        let mut by_level = [0usize; 16];
        for r in amb.regions.iter().filter(|r| r.1 == set) {
            if let Some((l, ..)) = amb.layout.cell(r.0, amb.levels) {
                by_level[l as usize] += 1;
            }
        }
        println!("    cells by level {:?}", &by_level[..amb.levels as usize]);
        let loops: Vec<String> =
            events.iter().filter_map(|&e| bank.stream_loop(e)).map(|l| format!("{:08x} {} Hz {:.0} dB", l.stream, l.rate, l.gain_db)).collect();
        println!("{set:08x} {name} ({cells} cells): {} sounds, loops {}", events.len(), loops.join(", "));
    }
    Ok(())
}

fn main() -> Result<()> {
    let a: Vec<String> = std::env::args().collect();
    match a.get(1).map(String::as_str) {
        Some("scan") if a.len() == 3 => scan(Path::new(&a[2])),
        Some("list") if a.len() == 4 => list(Path::new(&a[2]), &a[3]),
        Some("wav") if a.len() >= 4 => wav(Path::new(&a[2]), &a[3..]),
        Some("raw") if a.len() >= 4 => raw(Path::new(&a[2]), &a[3..]),
        Some("clip") if a.len() == 4 => clip(Path::new(&a[2]), &a[3]),
        Some("ambience") if a.len() == 4 => ambience(Path::new(&a[2]), &a[3]),
        Some("tree") if a.len() >= 4 => {
            let (bank, _) = load_bank(Path::new(&a[2]))?;
            for id in &a[3..] {
                tree(&bank, u32::from_str_radix(id.trim_start_matches("0x"), 16)?, 0);
            }
            Ok(())
        }
        _ => bail!(
            "usage: bao scan <game dir> | list <forge file> <data file> | wav <game dir> <bao id>... | raw <game dir> <bao id>... | clip <game dir> <clip name> | tree <game dir> <event id>... | ambience <game dir> <City>"
        ),
    }
}
