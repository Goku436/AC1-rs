//! AC1's fight settings read from the install (skipped when it is not found; `AC1_GAME_DIR` overrides the path).

use forge::fight::{self, character, source};
use std::path::PathBuf;

fn game() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var("AC1_GAME_DIR").unwrap_or_else(|_| "P:/SteamLibrary/steamapps/common/Assassins Creed".into()));
    let found = dir.join("DataPC.forge").is_file();
    if !found {
        eprintln!("skipped: no install at {dir:?}");
    }
    found.then_some(dir)
}

#[test]
fn damage_and_fight_manager() {
    let Some(game) = game() else { return };
    let f = forge::Forge::open(game.join("DataPC.forge")).unwrap();
    let s = fight::read_settings(&f).unwrap();
    assert_eq!(s.damage.rows.len(), character::COUNT);
    // A throwing knife kills the militia outright; the bosses' rows (Garnier, 6) take 20 of it.
    assert_eq!(s.damage.get(character::MILITIA, source::THROWING_KNIFE), 255);
    assert_eq!(s.damage.get(6, source::THROWING_KNIFE), 20);
    assert!(s.damage.get(character::PLAYER_SWORD, source::QUICK_ATK) > 0);
    assert_eq!(s.blocks.len(), 7);
    // (The fight manager ramps up over the memory blocks.)
    assert!(s.blocks[6][0].engaged[0] > s.blocks[0][0].engaged[0]);
    for (k, b) in s.blocks.iter().enumerate() {
        eprintln!("memory block {}: {:?}", k + 1, b[0]);
    }
}
