//! Pose gallery (test world, west of the start): every loop Altaïr can hold, one figure each, in rows by
//! category, with the clip's name over it up close and the category's name over the row. Rows of wall and
//! hang poses stand against a wall and off the ground. Some moves get a row too, their clips in the order
//! they chain (onto a low obstacle).

/// A gallery row: its name, whether it hangs on a wall, and the rule on the clip name.
pub type Category = (&'static str, bool, fn(&str) -> bool);

/// Categories, in row order: (name, whether the row hangs on a wall, rule on the clip name).
pub const CATEGORIES: [Category; 11] = [
    ("Climbing (wall)", true, |n| n.starts_with("xx_climb_wait") || n.contains("ingredient_climb")),
    ("Hanging (ledge)", true, |n| n.contains("hangwall")),
    ("Hanging (free)", true, |n| n.contains("hangfree")),
    ("Ladder", true, |n| n.contains("ladder")),
    ("Beam and post", false, |n| n.contains("beam")),
    ("Swing bar", false, |n| n.contains("swing_cycle")),
    ("Lean and collide", false, |n| n.contains("lean_") || n.contains("collide")),
    ("Hay", false, |n| n.contains("haystack")),
    ("Standing", false, |n| n.contains("wait_hipm") || n.contains("idle_onspot") || n.contains("pray") || n.contains("listen") || n.contains("waithigh")),
    ("Sitting and crouching", false, |n| n.contains("sit") || n.contains("crouchwait")),
    ("Other", false, |_| true),
];

/// Moves (not loops) with a row of their own, in the order they chain.
pub const MOVES: [(&str, &[&str]); 5] = [
    (
        "Onto a low obstacle (in order): step up 50 cm, jump onto",
        &[
            "xx_h_collide_full_footl_050cm_a",
            "xx_h_collide_full_footl_050cm_b",
            "xx_h_collide_full_footl_050cm_to_freestep_050cm",
            "xx_h_collide_full_footl_050cm_to_freestep_050cm_tr_freestep_entry_footl",
            "xx_h_sprint_impultion_footl",
            "xx_h_air_front_050cm_footl_to_freestep",
            "xx_h_air_front_050cm_footl_to_freestep_tr_freestep_entry_footr",
        ],
    ),
    (
        "Ground moves",
        &[
            "xx_h_runstop_footl",
            "xx_h_sprintstop_footl",
            "xx_h_jogstop_footl",
            "xx_h_ledge_stop_start_footl",
            "xx_h_ledge_stop_end_footl",
            "xx_h_collide_full_hand_150cm",
            "xx_h_lean_040cm_twohand_150cm_to_left_jog",
            "xx_h_landing_damage_footl_roll",
            "xx_roll_hipm",
            "xx_h_wait_hipm_footl_to_h_waitturn_left_180_footl",
        ],
    ),
    (
        "Ledge: pull-down, one hand, back eject",
        &[
            "xx_l_ledge_stop_start_footl_pulldown_front_orientation",
            "xx_l_ledge_pulldown_soft_front",
            "xx_l_ledge_pulldown_soft_to_hangwall_straight_a",
            "xx_h_hangwall_onehand_to_hangknee_onehand_footl_a",
            "xx_h_hangknee_onehand_footl_to_freestep_entry_footl",
            "xx_h_hangwall_tr_rebound_footl",
            "xx_h_rebound_frontleft_front_300cm_footl_to_air",
            "xx_h_climbing_climb1m_tr_hangfree_left_3_c",
        ],
    ),
    (
        "Bench, hay, pickpocket",
        &[
            "xx_rest_sit_left_180_to_sitting",
            "xx_rest_sit_sitting_tr_sit_idle_040cm_01",
            "xx_rest_sit_idle_040cm_01_tr_l_wait_footl",
            "xx_h_air_to_haystack",
            "xx_l_haystack_hop_out",
            "xx_pickpocket_attempt_walk_footl",
            "xx_pickpocket_attempt_success_finish_footl",
        ],
    ),
    (
        "Getting up",
        &["xx_getup_fast_faceup", "xx_getup_fast_facedown", "xx_getup_fast_sit", "xx_getup_slow_faceup", "xx_getup_slow_facedown", "xx_getup_slow_sit"],
    ),
];

/// Combat is left out for now (sword, hidden blade, knives, fists, blocks and guards).
fn combat(n: &str) -> bool {
    n.starts_with("xx_h_light")
        || n.starts_with("xx_h_blade")
        || n.starts_with("xx_h_knife")
        || n.starts_with("xx_h_defence")
        || n.contains("unarmed")
        || n.contains("block_wait")
        || n.contains("sheath")
        || n.contains("assassin")
}

/// Figures per row at most.
pub const PER_ROW: usize = 10;

/// The poses: Altaïr's loops (`xx_` clips that wait, idle or cycle; not transitions, look-arounds, talk,
/// horse clips or the knife throws on the move), one foot of each left/right pair, by category.
pub fn poses(names: &[String]) -> Vec<(&'static str, bool, Vec<String>)> {
    let held = |n: &str| {
        n.starts_with("xx_")
            && ["wait", "idle", "loop", "cycle"].iter().any(|k| n.contains(k))
            && !["_tr_", "_to_", "lookaround", "talk", "horse", "mobile", "cyclebreaker", "_start", "Scene"].iter().any(|k| n.contains(k))
    };
    let all: Vec<&String> = names.iter().filter(|n| held(n) && !combat(n)).collect();
    let mut rows: Vec<(&'static str, bool, Vec<String>)> = CATEGORIES.iter().map(|(c, wall, _)| (*c, *wall, vec![])).collect();
    for n in all {
        // One of each left/right pair.
        if n.contains("footr") && names.contains(&n.replace("footr", "footl")) {
            continue;
        }
        let Some(row) = CATEGORIES.iter().position(|(_, _, rule)| rule(n)) else { continue };
        if rows[row].2.len() < PER_ROW {
            rows[row].2.push(n.clone());
        }
    }
    for (name, clips) in MOVES {
        rows.push((name, false, clips.iter().filter(|c| names.iter().any(|n| n == *c)).map(|c| c.to_string()).collect()));
    }
    rows.retain(|r| !r.2.is_empty());
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sorts_poses_into_rows() {
        let names: Vec<String> = [
            "xx_climb_wait_1m",
            "xx_h_hangwall_wait",
            "xx_h_light_wait_footl",
            "xx_h_light_wait_footr",
            "xx_l_wait_hipm_footl",
            "xx_h_wait_hipm_footl_tr_jog",
            "xx_horse_wait01",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let rows = poses(&names);
        let row = |c: &str| rows.iter().find(|r| r.0 == c).map(|r| r.2.clone()).unwrap_or_default();
        assert_eq!(row("Climbing (wall)"), ["xx_climb_wait_1m"]);
        assert_eq!(row("Hanging (ledge)"), ["xx_h_hangwall_wait"]);
        // Combat, transitions and horses left out.
        assert!(rows.iter().all(|r| !r.2.iter().any(|n| n.starts_with("xx_h_light"))));
        assert_eq!(row("Standing"), ["xx_l_wait_hipm_footl"]);
        assert!(rows.iter().all(|r| !r.2.iter().any(|n| n.contains("horse") || n.contains("_tr_"))));
    }
}
