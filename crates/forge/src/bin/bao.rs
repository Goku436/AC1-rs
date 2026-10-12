//! bao: the install's sound (see `forge::sound`).
//! - `bao scan <game dir>`: every `SoundBao` object of every forge and every streamed BAO file: how many parse, their
//!   kinds and codecs, how many IMA resources and streams decode; the failures by message.
//! - `bao list <forge file> <data file>`: the data file's BAOs (id, kind, type or codec, bytes).
//! - `bao wav <game dir> <bao id hex>...`: write a resource, a stream, or what an event plays, as WAV files to
//!   `out/sound/` (git-ignored).
//! - `bao clip <game dir> <clip name substring>`: the event keys of the matching clips in "Game Fix", and what Altaïr's
//!   contact table and the sound bank play for each.
//! - `bao tree <game dir> <event id>...`: an event's graph (switches and their cases, multis, plays, random
//!   containers, sounds).
use anyhow::{Context, Result, bail};
use forge::sound::{self, Bank, Codec, Kind};
use std::collections::BTreeMap;
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

fn scan(dir: &Path) -> Result<()> {
    let (mut objects, mut objects_ok) = (0, 0);
    let mut kinds: BTreeMap<Kind, usize> = BTreeMap::new();
    let mut codecs: BTreeMap<(Kind, Codec), usize> = BTreeMap::new();
    let (mut ima, mut ima_ok) = (0, 0);
    let (mut streams, mut streams_ok, mut stream_ima, mut stream_ima_ok) = (0, 0, 0, 0);
    let mut errs: BTreeMap<String, (usize, String)> = BTreeMap::new();
    let mut fail = |what: &str, e: &anyhow::Error, name: &str| errs.entry(format!("{what}: {}", short(e))).or_insert((0, name.to_string())).0 += 1;
    let mut seconds = 0.0f64;
    for path in forges(dir)? {
        let f = forge::Forge::open(&path)?;
        for e in &f.entries {
            if e.name.contains("_BAO_0x") {
                streams += 1;
                let d = f.read(e)?;
                match sound::parse_bao(&d) {
                    Ok(b) => {
                        streams_ok += 1;
                        let c = sound::codec(b.object);
                        *codecs.entry((b.kind, c)).or_default() += 1;
                        if matches!(c, Codec::Ima | Codec::Tracks) {
                            stream_ima += 1;
                            match sound::decode(b.object) {
                                Ok(_) => stream_ima_ok += 1,
                                Err(err) => fail("stream", &err, &e.name),
                            }
                        }
                    }
                    Err(err) => fail("stream header", &err, &e.name),
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
                        fail("SoundBao", &err, &o.name);
                        continue;
                    }
                };
                objects_ok += 1;
                for (_, b) in &s.variants {
                    *kinds.entry(b.kind).or_default() += 1;
                    if matches!(b.kind, Kind::Resource | Kind::Stream) {
                        *codecs.entry((b.kind, sound::codec(b.object))).or_default() += 1;
                    }
                    if b.kind == Kind::Resource && matches!(sound::codec(b.object), Codec::Ima | Codec::Tracks) {
                        ima += 1;
                        match sound::decode(b.object) {
                            Ok((ch, pcm)) => {
                                ima_ok += 1;
                                seconds += pcm.len() as f64 / ch as f64 / 24000.0;
                            }
                            Err(err) => fail("IMA", &err, &o.name),
                        }
                    }
                }
            }
        }
        eprintln!("{}: {objects} SoundBaos so far", path.display());
    }
    println!("SoundBao objects: {objects_ok}/{objects} parse");
    println!("BAOs by kind: {kinds:?}");
    println!("resources and streams by codec: {codecs:x?}");
    println!("IMA resources: {ima_ok}/{ima} decode (about {:.0} s at 24 kHz)", seconds);
    println!("streamed BAO files: {streams_ok}/{streams} parse; IMA ones {stream_ima_ok}/{stream_ima} decode");
    for (k, (n, ex)) in errs {
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
        (k, t) => println!("{pad}{id:08x} kind {k} type {t}"),
    }
}

fn main() -> Result<()> {
    let a: Vec<String> = std::env::args().collect();
    match a.get(1).map(String::as_str) {
        Some("scan") if a.len() == 3 => scan(Path::new(&a[2])),
        Some("list") if a.len() == 4 => list(Path::new(&a[2]), &a[3]),
        Some("wav") if a.len() >= 4 => wav(Path::new(&a[2]), &a[3..]),
        Some("clip") if a.len() == 4 => clip(Path::new(&a[2]), &a[3]),
        Some("tree") if a.len() >= 4 => {
            let (bank, _) = load_bank(Path::new(&a[2]))?;
            for id in &a[3..] {
                tree(&bank, u32::from_str_radix(id.trim_start_matches("0x"), 16)?, 0);
            }
            Ok(())
        }
        _ => bail!(
            "usage: bao scan <game dir> | list <forge file> <data file> | wav <game dir> <bao id>... | clip <game dir> <clip name> | tree <game dir> <event id>..."
        ),
    }
}
