//! AC1's move graph, indexed by clip: what the `ActionBlock`s of an `ActionKit` say a clip is for and what may
//! follow it. An action is a chain of items played in order; an item is one clip or a blend of several (variants
//! mixed by weight); an item's transitions name, per way out, a connector action and a destination action (by
//! action key). So after a clip may come: the next item of its action, or the first item of a connector or
//! destination its item lists. (Checked on `HumanGround`'s run stop: on to walking or jogging through
//! `runstop_tr_walk/jog`, or to standing through `runstop_tr_h_wait`.)
//!
//! Built from parsed blocks and the clip names of their data file; no game data is needed to test it.

use crate::action::{ActionBlock, Slot};
use std::collections::{HashMap, HashSet};

/// Where a clip sits in the graph.
#[derive(Debug, Clone)]
pub struct Place {
    /// The block's name (`HumanGround`, `HumanLedge`, ...), the action's key, and the item's index in it.
    pub block: String,
    pub action: u32,
    pub item: usize,
}

/// An item: its clips (names), blend-in time (s), and its ways out (connector, destination action keys).
type ItemNode = (Vec<String>, f32, Vec<(u32, u32)>);

#[derive(Debug, Clone, Default)]
struct ActionNode {
    items: Vec<ItemNode>,
}

#[derive(Debug, Default)]
pub struct MoveGraph {
    actions: HashMap<u32, ActionNode>,
    places: HashMap<String, Vec<Place>>,
    /// Actions some transition leads to (the rest only the game's code starts).
    targeted: HashSet<u32>,
}

impl MoveGraph {
    /// Index `blocks` (name, parsed block); `clip_name` names a clip object id.
    pub fn new<'a>(blocks: impl IntoIterator<Item = (&'a str, &'a ActionBlock)>, clip_name: impl Fn(u32) -> Option<String>) -> Self {
        let mut g = MoveGraph::default();
        for (block, b) in blocks {
            for a in b.actions.iter().filter_map(Slot::inline) {
                let mut node = ActionNode::default();
                for (k, item) in a.items.iter().enumerate() {
                    let clips: Vec<String> = item.clips.iter().filter_map(|&c| clip_name(c)).collect();
                    let next: Vec<(u32, u32)> = item.transitions.iter().filter_map(Slot::inline).map(|t| (t.action, t.action2)).collect();
                    g.targeted.extend(next.iter().flat_map(|&(c, d)| [c, d]));
                    for c in &clips {
                        g.places.entry(c.clone()).or_default().push(Place { block: block.to_string(), action: a.key, item: k });
                    }
                    node.items.push((clips, item.blend.times[0], next));
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

    /// The blend-in time (s) of the item a clip is in (its first listing).
    pub fn blend_in(&self, clip: &str) -> Option<f32> {
        let p = self.places(clip).first()?;
        self.actions.get(&p.action)?.items.get(p.item).map(|i| i.1)
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
            for key in item.2.iter().flat_map(|&(c, d)| [c, d]) {
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
    use crate::action::{Action, Blend, Item, Transition};

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
        assert_eq!(g.blend_in("stop_a"), Some(0.2));
        assert_eq!(g.places("stop_tr_wait")[0].block, "HumanGround");
    }
}
