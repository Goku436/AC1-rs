//! skelcheck <file.forge>: parse every skeleton.
fn main() -> anyhow::Result<()> {
    let f = forge::Forge::open(std::env::args().nth(1).unwrap())?;
    let (mut ok, mut bad) = (0, 0);
    for e in &f.entries {
        let Ok(d) = f.read(e) else { continue };
        let Ok(objs) = forge::parse_objects(&d) else { continue };
        for o in objs.iter().filter(|o| o.class == forge::skeleton::CLASS_SKELETON) {
            match forge::skeleton::parse_skeleton(o.body) {
                Ok(s) => {
                    ok += 1;
                    if o.name == "UCMA_Altair" {
                        println!("{} bones, {} named", s.bones.len(), s.bones.iter().filter(|b| b.name.is_some()).count());
                    }
                }
                Err(err) => {
                    bad += 1;
                    if bad < 5 {
                        println!("FAIL {}: {err:#}", o.name);
                    }
                }
            }
        }
    }
    println!("ok {ok} bad {bad}");
    Ok(())
}
