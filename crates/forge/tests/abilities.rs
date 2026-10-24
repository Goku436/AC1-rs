//! The ranks' ability sets, read from the install (skipped when it is not found; `AC1_GAME_DIR` overrides the path).

use forge::ability::{AbilitySet, bit, rank_set};
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
fn every_rank_allows_the_movement_and_grasp_comes_at_rank_4() {
    let Some(game) = game() else { return };
    let movement = [bit::JUMP, bit::PASS_OVER, bit::WALLING, bit::CLIMB, bit::LADDER, bit::LEAP_OF_FAITH, bit::LOOK_DOWN];
    for rank in 0..10 {
        let s = rank_set(&game, rank).unwrap();
        eprintln!("rank {rank}: {:09x}, max speed {}, high profile {}", s.flags, s.max_speed, s.high_profile);
        assert!(movement.iter().all(|&b| s.allows(b)), "rank {rank}: {s:?}");
        assert_eq!(s.max_speed, 4);
        assert!(s.high_profile);
        assert_eq!(s.allows(bit::GRASP), rank >= 4, "rank {rank}");
        // (The fight is there from the start, its counters come with the ranks.)
        assert!(s.allows(bit::FIGHT));
        assert_eq!(s.allows(bit::COUNTER_KILL), rank > 1, "rank {rank}");
    }
    assert_eq!(rank_set(&game, 9).unwrap(), AbilitySet::ALL);
}
