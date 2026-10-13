//! The level cache (`src/level_cache.rs`) against a fresh build: each city built with `AC1_CACHE_CHECK=1` into an empty
//! cache directory (its gameplay data written and read back, byte for byte), built again and compared with the file the
//! first run saved (another process: the build gives the same bytes every time), then loaded from the cache. Needs the
//! game installed (`AC1_GAME_DIR` or the default Steam path) and runs only with `AC1_SCENARIOS=1`, as the scenarios do
//! (headless, about a minute); otherwise it passes without running.
//!
//!     AC1_SCENARIOS=1 cargo test --release -p ac1 --test level_cache -- --nocapture

use std::path::PathBuf;
use std::process::Command;

const DEFAULT_GAME_DIR: &str = "P:/SteamLibrary/steamapps/common/Assassins Creed";

fn game_dir() -> Option<PathBuf> {
    let d = PathBuf::from(std::env::var("AC1_GAME_DIR").unwrap_or_else(|_| DEFAULT_GAME_DIR.into()));
    d.join("DataPC.forge").exists().then_some(d)
}

/// One headless run of the game in `city` with the cache in `dir`, its log.
fn run(city: &str, dir: &std::path::Path, check: bool) -> String {
    let mut c = Command::new(env!("CARGO_BIN_EXE_ac1"));
    c.env("NO_COLOR", "1")
        .env("RUST_LOG", "ac1=info,wgpu=error")
        .env("AC1_HEADLESS", "1")
        .env("AC1_MUTE", "1")
        .env("AC1_NO_CROWD", "1")
        .env("AC1_SHOT", "-")
        .env("AC1_SHOT_SECS", "0.1")
        .env("AC1_LEVEL", city)
        .env("AC1_CACHE_DIR", dir)
        .env_remove("AC1_NO_CACHE");
    if check {
        c.env("AC1_CACHE_CHECK", "1");
    }
    let out = c.output().expect("run the game");
    String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr)
}

#[test]
fn level_cache_matches_a_fresh_build() {
    if std::env::var("AC1_SCENARIOS").is_err() {
        eprintln!("level cache check skipped: set AC1_SCENARIOS=1 to run it");
        return;
    }
    let Some(game) = game_dir() else {
        eprintln!("level cache check skipped: the game is not installed (AC1_GAME_DIR)");
        return;
    };
    eprintln!("level cache check on {}", game.display());
    let mut failed = vec![];
    for city in ["damascus", "masyaf"] {
        let dir = std::env::temp_dir().join(format!("ac1-level-cache-test-{}-{city}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let steps = [
            (true, "level cache check: read back identical", "the first build, read back"),
            (true, "level cache check: the cache file is identical to the fresh build", "a second build against the first's file"),
            (false, "(gameplay data from the cache)", "loaded from the cache"),
        ];
        for (check, want, what) in steps {
            let log = run(city, &dir, check);
            let ok = log.contains(want) && !log.contains("DIFFERENT") && !log.contains("panicked");
            let lines: Vec<&str> = log.lines().filter(|l| l.contains("level cache") || l.contains("ready in")).collect();
            eprintln!("{city}, {what}: {}\n  {}", if ok { "ok" } else { "FAILED" }, lines.join("\n  "));
            if !ok {
                failed.push(format!("{city}: {what}"));
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
    assert!(failed.is_empty(), "level cache: {failed:?}");
}
