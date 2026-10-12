//! animcheck <game dir>: parse every animation in every archive; report failures and codec usage, and how many
//! clips' event tracks parse (`anim::parse_events`).
use std::collections::BTreeMap;
fn main() -> anyhow::Result<()> {
    let dir = std::env::args().nth(1).unwrap();
    let (mut ok, mut bad) = (0, 0);
    let mut errs: BTreeMap<String, (usize, String)> = BTreeMap::new();
    let mut unknown: BTreeMap<u8, usize> = BTreeMap::new();
    let (mut ev_ok, mut ev_keys) = (0, 0);
    let mut ev_errs: BTreeMap<String, (usize, String)> = BTreeMap::new();
    for p in
        std::fs::read_dir(&dir)?.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "forge") && !p.to_string_lossy().contains("Sound"))
    {
        let f = forge::Forge::open(&p)?;
        for e in &f.entries {
            let Ok(d) = f.read(e) else { continue };
            let Ok(objs) = forge::parse_objects(&d) else { continue };
            for o in objs.iter().filter(|o| o.class == forge::anim::CLASS_ANIMATION) {
                match forge::anim::parse_animation(o.bytes) {
                    Ok(a) => {
                        ok += 1;
                        match forge::anim::parse_events(o.body) {
                            Ok(e) => {
                                ev_ok += 1;
                                ev_keys += e.len();
                            }
                            Err(err) => {
                                let k: String = err.to_string().split(" at 0x").next().unwrap_or("").chars().take(40).collect();
                                ev_errs.entry(k).or_insert((0, o.name.clone())).0 += 1;
                            }
                        }
                        for t in &a.tracks {
                            if let forge::anim::Channel::Unknown(k) = t.channel {
                                *unknown.entry(k).or_default() += 1;
                            }
                        }
                    }
                    Err(err) => {
                        bad += 1;
                        let k = err.to_string();
                        let k: String = k.chars().take(30).collect();
                        errs.entry(k).or_insert((0, o.name.clone())).0 += 1;
                    }
                }
            }
        }
    }
    println!("ok {ok} bad {bad} undecoded track types {unknown:x?}");
    println!("event tracks parse in {ev_ok}/{ok} clips ({ev_keys} keys)");
    for (k, (n, ex)) in ev_errs {
        println!("{n:>6} events: {k}  e.g. {ex}");
    }
    for (k, (n, ex)) in errs {
        println!("{n:>6} {k}  e.g. {ex}");
    }
    Ok(())
}
