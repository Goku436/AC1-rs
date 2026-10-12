//! AC1's move graph, indexed by clip: what the `ActionBlock`s of an `ActionKit` say a clip is for and what may
//! follow it. An action is a chain of items played in order; an item is one clip or a blend of several (variants
//! mixed by weight); an item's transitions name, per way out, a connector action and a destination action (by
//! action key). So after a clip may come: the next item of its action, or the first item of a connector or
//! destination its item lists. (Checked on `HumanGround`'s run stop: on to walking or jogging through
//! `runstop_tr_walk/jog`, or to standing through `runstop_tr_h_wait`.)
//!
//! Blends: each step carries AC1's whole `Blend` (kind, flags, modes, times), looked up as the game does (Banned445's
//! port of `sub_725950` and `0x773F90`): item k to item k+1 of one action takes item k+1's blend; a way out of an item
//! takes its transition's blend A into the connector (or straight into the destination when there is none), and the
//! connector's end goes on into the destination with blend B; an action the code starts takes the playing item's
//! way to it, else the playing action's out-transition, else the new action's in-transition, else the default
//! (ASTOPBROLL 0.2 s). A blend of kind NONE is a cut (audit ANIM-4, ANIM-5).
//!
//! Built from parsed blocks and the clip names of their data file; no game data is needed to test it.

use crate::action::{ActionBlock, Blend, Slot};
use std::collections::{HashMap, HashSet};

/// Where a clip sits in the graph.
#[derive(Debug, Clone)]
pub struct Place {
    /// The block's name (`HumanGround`, `HumanLedge`, ...), the action's key, and the item's index in it.
    pub block: String,
    pub action: u32,
    pub item: usize,
}

/// An item: its clips (names), its blend (from the item before it in its action), its ways out, and its gate word
/// (`Item::word`, see `Gate`).
type ItemNode = (Vec<String>, Blend, Vec<Way>, u32);

/// A way out of an item (`ActionTransition`): the connector action played first (0: none) with blend A, then the
/// destination action with blend B.
#[derive(Debug, Clone, Copy)]
struct Way {
    connector: u32,
    dest: u32,
    a: Blend,
    b: Blend,
}

/// An item's gate word, as the game reads it for the item playing (`ActionItem` +60, Banned445's trace; docs/PARKOUR.md
/// section 2; bits in `action::gate`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Gate(pub u32);

impl Gate {
    pub fn has(self, bit: u32) -> bool {
        self.0 & bit != 0
    }

    /// Whether the item may be left now: for moving (else standing), in high profile (else low); never while locked.
    pub fn may_leave(self, moving: bool, high: bool) -> bool {
        use crate::action::gate;
        let bit = match (moving, high) {
            (true, false) => gate::LEAVE_MOVE_LOW,
            (true, true) => gate::LEAVE_MOVE_HIGH,
            (false, false) => gate::LEAVE_STAND_LOW,
            (false, true) => gate::LEAVE_STAND_HIGH,
        };
        !self.has(gate::LOCKED) && self.has(bit)
    }
}

#[derive(Debug, Clone, Default)]
struct ActionNode {
    items: Vec<ItemNode>,
    /// The action's own in- and out-transitions' blend A (`Action` +16, +20).
    enter: Option<Blend>,
    leave: Option<Blend>,
    /// `Action` flag byte 2: no default transition into it (a cut when nothing else gives one).
    no_default: bool,
}

#[derive(Debug, Default)]
pub struct MoveGraph {
    actions: HashMap<u32, ActionNode>,
    places: HashMap<String, Vec<Place>>,
    /// Actions some transition leads to (the rest only the game's code starts).
    targeted: HashSet<u32>,
    /// (connector, destination) of every way with a connector: blend B, from the connector's end into the destination.
    after_connector: HashMap<(u32, u32), Blend>,
}

impl MoveGraph {
    /// Index `blocks` (name, parsed block); `clip_name` names a clip object id.
    pub fn new<'a>(blocks: impl IntoIterator<Item = (&'a str, &'a ActionBlock)>, clip_name: impl Fn(u32) -> Option<String>) -> Self {
        let mut g = MoveGraph::default();
        for (block, b) in blocks {
            for a in b.actions.iter().filter_map(Slot::inline) {
                let blend_a = |k: usize| a.transitions[k].as_ref().and_then(Slot::inline).map(|t| t.blend);
                let mut node = ActionNode { enter: blend_a(0), leave: blend_a(1), no_default: a.flags[2] != 0, ..Default::default() };
                for (k, item) in a.items.iter().enumerate() {
                    let clips: Vec<String> = item.clips.iter().filter_map(|&c| clip_name(c)).collect();
                    let next: Vec<Way> = item
                        .transitions
                        .iter()
                        .filter_map(Slot::inline)
                        .map(|t| Way { connector: t.action, dest: t.action2, a: t.blend, b: t.blend2 })
                        .collect();
                    g.targeted.extend(next.iter().flat_map(|w| [w.connector, w.dest]));
                    for w in next.iter().filter(|w| w.connector != 0 && w.dest != 0) {
                        g.after_connector.entry((w.connector, w.dest)).or_insert(w.b);
                    }
                    for c in &clips {
                        g.places.entry(c.clone()).or_default().push(Place { block: block.to_string(), action: a.key, item: k });
                    }
                    node.items.push((clips, item.blend, next, item.word()));
                }
                g.actions.insert(a.key, node);
            }
        }
        g
    }

    /// Every place a clip is listed (some clips are in several actions).
    pub fn places(&self, clip: &str) -> &[Place] {
        self.places.get(clip).map_or(&[], Vec::as_slice)
    }

    /// The blend of the item a clip is in (its first listing): the one from the item before it in its action.
    pub fn item_blend(&self, clip: &str) -> Option<Blend> {
        let p = self.places(clip).first()?;
        self.actions.get(&p.action)?.items.get(p.item).map(|i| i.1)
    }

    /// The gate word of the item a clip is in (its first listing).
    pub fn gate(&self, clip: &str) -> Option<Gate> {
        let p = self.places(clip).first()?;
        self.actions.get(&p.action)?.items.get(p.item).map(|i| Gate(i.3))
    }

    fn starts(&self, action: u32, clip: &str) -> bool {
        self.actions.get(&action).and_then(|x| x.items.first()).is_some_and(|i| i.0.iter().any(|n| n == clip))
    }

    /// The blend of the step from `a` to `b` where the graph lists it: `b`'s item blend when `b` is the next item of
    /// `a`'s action; a way out of `a`'s item: blend A into its connector, or into its destination when it has none
    /// (with a connector skipped, the default: Banned445's port, as A was the connector's own); the end of a connector
    /// into the destination a way pairs it with: that way's blend B.
    pub fn step_blend(&self, a: &str, b: &str) -> Option<Blend> {
        for p in self.places(a) {
            let Some(act) = self.actions.get(&p.action) else { continue };
            let Some(item) = act.items.get(p.item) else { continue };
            if let Some(next) = act.items.get(p.item + 1).filter(|i| i.0.iter().any(|n| n == b)) {
                return Some(next.1);
            }
            for w in &item.2 {
                if w.connector != 0 && self.starts(w.connector, b) {
                    return Some(w.a);
                }
                if self.starts(w.dest, b) {
                    return Some(if w.connector == 0 { w.a } else { Blend::DEFAULT });
                }
            }
            if p.item + 1 == act.items.len() {
                for to in self.places(b).iter().filter(|q| q.item == 0) {
                    if let Some(&blend) = self.after_connector.get(&(p.action, to.action)) {
                        return Some(blend);
                    }
                }
            }
        }
        None
    }

    /// The blend into `to`'s action when the game's code starts it with `from` playing (`sub_725950`, Banned445's
    /// port): `from`'s item's way to it (blend A when it has no connector, the default when a connector is skipped);
    /// else the playing action's out-transition; else the new action's in-transition; else the default transition
    /// (ASTOPBROLL 0.2 s), or a cut for an action flagged to have none. `None` when `to` is not in the graph.
    pub fn enter_blend(&self, from: Option<&str>, to: &str) -> Option<Blend> {
        let target = self.places(to).first()?;
        let new = self.actions.get(&target.action)?;
        let playing = from.and_then(|f| self.places(f).first());
        if let Some(p) = playing
            && let Some(act) = self.actions.get(&p.action)
        {
            if let Some(w) = act.items.get(p.item).and_then(|i| i.2.iter().find(|w| w.dest == target.action)) {
                return Some(if w.connector == 0 { w.a } else { Blend::DEFAULT });
            }
            if p.action != target.action
                && let Some(b) = act.leave
            {
                return Some(b);
            }
        }
        if let Some(b) = new.enter {
            return Some(b);
        }
        Some(if new.no_default { Blend::default() } else { Blend::DEFAULT })
    }

    /// The blend from `from` (playing, if known) into `to`: the step the graph lists, else the code's transition into
    /// `to`'s action (`enter_blend`). `None` when the graph has nothing for `to`.
    pub fn blend_into(&self, from: Option<&str>, to: &str) -> Option<Blend> {
        from.and_then(|f| self.step_blend(f, to)).or_else(|| self.enter_blend(from, to))
    }

    /// The clips that may follow `clip`: the next item of its action, and the first item of every connector and
    /// destination its item lists (all its listings).
    pub fn successors(&self, clip: &str) -> HashSet<&str> {
        let mut out: HashSet<&str> = HashSet::new();
        for p in self.places(clip) {
            let Some(a) = self.actions.get(&p.action) else { continue };
            if let Some(next) = a.items.get(p.item + 1) {
                out.extend(next.0.iter().map(String::as_str));
            }
            let Some(item) = a.items.get(p.item) else { continue };
            // (Through a connector, its own first item; the destination's too, for chains that skip it.)
            for key in item.2.iter().flat_map(|w| [w.connector, w.dest]) {
                if let Some(i) = self.actions.get(&key).and_then(|a| a.items.first()) {
                    out.extend(i.0.iter().map(String::as_str));
                }
            }
        }
        out
    }

    /// Whether the graph lets `b` follow `a`. Not judged (true): clips the graph doesn't list, and a clip whose
    /// every listing is the last item of its action with no ways out: the game's code picks what follows those
    /// (the wall run's `entry_a` and `entry_b` are separate actions, stepped through by its `WallingEntryA/B`).
    pub fn allows(&self, a: &str, b: &str) -> bool {
        if self.places(a).is_empty() || self.places(b).is_empty() || self.code_driven(a) || self.code_started(b) {
            return true;
        }
        self.successors(a).contains(b)
    }

    /// Every listing of `clip` starts an action no transition leads to: the game's code starts it, from wherever
    /// (a post jump's push off, `beam_impultionstraight_to_jumpstraight`, from the crouch).
    pub fn code_started(&self, clip: &str) -> bool {
        self.places(clip).iter().all(|p| p.item == 0 && !self.targeted.contains(&p.action))
    }

    /// Every listing of `clip` ends its action with no ways out listed.
    pub fn code_driven(&self, clip: &str) -> bool {
        self.places(clip).iter().all(|p| self.actions.get(&p.action).is_none_or(|a| p.item + 1 >= a.items.len() && a.items[p.item].2.is_empty()))
    }

    /// The clips that may lead to `clip` (the inverse of `successors`; a scan, for tools).
    pub fn predecessors(&self, clip: &str) -> Vec<&str> {
        let mut out: Vec<&str> = self.places.keys().map(String::as_str).filter(|a| self.successors(a).contains(clip)).collect();
        out.sort();
        out
    }

    /// The clips of an action's items, in order (the first of a blend space's variants are listed with it), by the
    /// action's key: the ids the game's code asks for (move tables, `0x019A05F1` ...).
    pub fn action_clips(&self, action: u32) -> Option<Vec<&[String]>> {
        self.actions.get(&action).map(|a| a.items.iter().map(|i| i.0.as_slice()).collect())
    }

    pub fn len(&self) -> usize {
        self.actions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::{Action, Blend, Item, Transition, blend_kind};

    fn fade(kind: u32, time: f32) -> Blend {
        Blend { kind, times: [time, 0.0, 0.0], ..Default::default() }
    }

    fn item(clips: &[u32], blend: f32, next: &[(u32, u32)]) -> Item {
        Item {
            clips: clips.to_vec(),
            blend: Blend { times: [blend, 0.0, 0.0], ..Default::default() },
            transitions: next.iter().map(|&(a, b)| Slot::Inline(Transition { action: a, action2: b, ..Default::default() })).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn follows_items_and_transitions() {
        // A stop (two items), its connector to standing and the stand loop.
        let stop = Action { key: 1, items: vec![item(&[10], 0.2, &[]), item(&[11], 0.0, &[(2, 3)])], ..Default::default() };
        let connector = Action { key: 2, items: vec![item(&[20], 0.0, &[])], ..Default::default() };
        let stand = Action { key: 3, items: vec![item(&[30], 0.0, &[])], ..Default::default() };
        let block = ActionBlock { actions: vec![Slot::Inline(stop), Slot::Inline(connector), Slot::Inline(stand)], ..Default::default() };
        let names: HashMap<u32, &str> = [(10, "stop_a"), (11, "stop_b"), (20, "stop_tr_wait"), (30, "wait")].into();
        let g = MoveGraph::new([("HumanGround", &block)], |id| names.get(&id).map(|s| s.to_string()));
        assert_eq!(g.len(), 3);
        assert!(g.allows("stop_a", "stop_b"));
        assert!(g.allows("stop_b", "stop_tr_wait"));
        assert!(g.allows("stop_b", "wait"));
        assert!(!g.allows("stop_a", "stop_tr_wait"));
        // The stop is started by code (no transition leads to it): any step into it is allowed.
        assert!(g.code_started("stop_a"));
        assert!(g.allows("unlisted", "wait"));
        // The stand loop ends its action with no ways out: the code decides, not judged.
        assert!(g.code_driven("wait"));
        assert!(g.allows("wait", "stop_a"));
        assert!(!g.code_started("wait"));
        assert_eq!(g.item_blend("stop_a").map(|b| b.time()), Some(0.2));
        assert_eq!(g.places("stop_tr_wait")[0].block, "HumanGround");
    }

    #[test]
    fn blends_as_the_game_looks_them_up() {
        // A run stop of two items (into the second: a cut), its way to standing through a connector (blend A a 0.1 s
        // fade, blend B a cut); the stand with an in-transition; a turn with none, flagged to have no default.
        let mut stop_b = item(&[11], 0.2, &[]);
        stop_b.transitions = vec![Slot::Inline(Transition {
            action: 2,
            action2: 3,
            blend: fade(blend_kind::ASTOP_BROLL, 0.1),
            blend2: fade(blend_kind::NONE, 0.2),
            ..Default::default()
        })];
        let stop = Action { key: 1, items: vec![item(&[10], 0.2, &[]), stop_b], ..Default::default() };
        let connector = Action { key: 2, items: vec![item(&[20], 0.2, &[])], ..Default::default() };
        let enter = Some(Slot::Inline(Transition { blend: fade(blend_kind::AROLL_BROLL, 0.06), ..Default::default() }));
        let stand = Action { key: 3, items: vec![item(&[30], 0.2, &[])], transitions: [enter, None], ..Default::default() };
        let turn = Action { key: 4, items: vec![item(&[40], 0.2, &[])], flags: [0, 0, 1, 0, 0], ..Default::default() };
        let other = Action { key: 5, items: vec![item(&[50], 0.2, &[])], ..Default::default() };
        let block = ActionBlock { actions: [stop, connector, stand, turn, other].into_iter().map(Slot::Inline).collect(), ..Default::default() };
        let names: HashMap<u32, &str> = [(10, "stop_a"), (11, "stop_b"), (20, "stop_tr_wait"), (30, "wait"), (40, "turn"), (50, "other")].into();
        let g = MoveGraph::new([("HumanGround", &block)], |id| names.get(&id).map(|s| s.to_string()));
        // Item to item: the next item's own blend (here a cut, though it carries 0.2 s).
        assert!(g.step_blend("stop_a", "stop_b").is_some_and(|b| b.is_cut()));
        // Into the connector: blend A; the connector on into the destination: blend B.
        assert_eq!(g.step_blend("stop_b", "stop_tr_wait"), Some(fade(blend_kind::ASTOP_BROLL, 0.1)));
        assert!(g.step_blend("stop_tr_wait", "wait").is_some_and(|b| b.is_cut()));
        // Skipping the connector: the default.
        assert_eq!(g.step_blend("stop_b", "wait"), Some(Blend::DEFAULT));
        assert_eq!(g.step_blend("stop_a", "wait"), None);
        // Started by code: the new action's in-transition; else the default; none for a flagged action.
        assert_eq!(g.enter_blend(Some("other"), "wait"), Some(fade(blend_kind::AROLL_BROLL, 0.06)));
        assert_eq!(g.enter_blend(None, "other"), Some(Blend::DEFAULT));
        assert!(g.enter_blend(None, "turn").is_some_and(|b| b.is_cut()));
        assert_eq!(g.enter_blend(None, "unlisted"), None);
        // The listed step first, then the code's transition.
        assert_eq!(g.blend_into(Some("stop_b"), "stop_tr_wait"), Some(fade(blend_kind::ASTOP_BROLL, 0.1)));
        assert_eq!(g.blend_into(Some("stop_a"), "other"), Some(Blend::DEFAULT));
    }
}
