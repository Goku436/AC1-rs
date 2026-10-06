//! Action blocks (class `ActionBlock`, ef82fce4) and the action kit (`ActionKit`, 195b695e): AC1's move
//! graph data. `Human_ActionKit` (in DataPC.forge, data file `Game Fix`) lists 51 blocks (`HumanGround`,
//! `HumanInAir`, `HumanWalling`, `HumanLedge`, `HumanClimb`, `HumanClimb_Jumps`, `HumanNarrowObject`, fights...).
//!
//! A block is a list of actions. An action is a list of items played one after another (an entry `_a` then
//! its `_b`); an item lists one or more clips. Several clips in one item are a blend space: variants of
//! one move (`hangwall_tr_hangknee` straight, `_45_in_`, `_30_out_`; `passover` 030cm, 100cm) mixed by
//! weight, the item's `weights` giving the default mix. Which action plays when is decided in code (not
//! here); the block gives what the action is made of: clips, blend times, feet, how its root moves, and
//! transitions to other actions.
//!
//! Serialization (learned from the game's class descriptors and its generated deserializers; numbers are
//! little-endian, nothing is aligned):
//! - an owned object slot: u8 kind, 0 = inline (u32 id, u32 class, body), 2 = a reference (u32 id), 3 = none
//! - an inline object field: u32 id, u32 class, body (no kind byte); array elements of inline objects the same
//! - an array: u32 count, then the elements; a link to another object: u32 id
//! - `ActionBlock`: [Action slot], u32 BodyPartTemplate link, AssociatedActionGroup slot
//! - `Action`: u32 key, BodyPartChannel slot, 2 x ActionTransition slot, 5 x u8 flags `?`, u32 torso
//!   constraint mode, u32 `?`, u32 `?`, AssociatedActionGroup slot, [ActionItem slot]
//! - `ActionItem`: [u32 clip link], [ActionTransition slot], ActionBlend (inline), u32 displacement mode,
//!   u32 feet at start, u32 feet at end, 12 x u8 flags `?`, f32 `?`, u32 `?`, u8 `?`, [f32 variant weight]
//! - `ActionTransition`: ActionBlend (inline), u32 action link, u32 `?`, ActionBlend (inline), u32 action
//!   link, u32 `?`
//! - `ActionBlend` (its own deserializer): u32 blend type, 3 x u8 flags, u32 B-position mode, u32
//!   displacement source mode, u32 actuator mode, u8 `?`, 3 x f32 times (s), u8 `?`, then an
//!   `ActionBlendFrankenstein` slot (always none in AC1's data; parsing one is an error)
//! - `AssociatedActionGroup`: [inline AssociatedAction: u32 action link, f32 weight, u32 `?`], 7 x u8 `?`
//!
//! Every action block in the install (300 data files' worth) parses to its exact last byte.

use anyhow::{Result, bail, ensure};

pub const CLASS_ACTION_BLOCK: u32 = 0xef82fce4;
pub const CLASS_ACTION_KIT: u32 = 0x195b695e;
const CLASS_ACTION: u32 = 0x406089a4;
const CLASS_ITEM: u32 = 0x80e50e4a;
const CLASS_TRANSITION: u32 = 0x46ed6df7;
const CLASS_BLEND: u32 = 0xc7041aee;
const CLASS_GROUP: u32 = 0x1e6ddbe2;
const CLASS_ASSOCIATED: u32 = 0x356e00e2;

/// An owned object field: inline or a reference to an object elsewhere.
#[derive(Debug, Clone)]
pub enum Slot<T> {
    Inline(T),
    Ref(u32),
}

impl<T> Slot<T> {
    pub fn inline(&self) -> Option<&T> {
        match self {
            Slot::Inline(t) => Some(t),
            Slot::Ref(_) => None,
        }
    }
}

/// Where a move's root motion comes from (`ACTDisplacementMode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Displacement {
    /// The clip's own root motion.
    #[default]
    Anim,
    /// Physics moves the body (falls).
    Physics,
    /// The game steers it (to a target: entries onto holds).
    Ai,
    Other(u32),
}

/// Which foot leads at a clip's start or end (`ACTFeetPosition`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Feet {
    #[default]
    NotSet,
    Parallel,
    RightAhead,
    LeftAhead,
    Other(u32),
}

#[derive(Debug, Clone, Default)]
pub struct Blend {
    /// `ACTBlendType`.
    pub kind: u32,
    pub flags: [u8; 3],
    /// B-position, displacement source and actuator modes.
    pub modes: [u32; 3],
    /// Times (s); the first is the blend into the move (0.2 is the common one).
    pub times: [f32; 3],
}

#[derive(Debug, Clone, Default)]
pub struct Transition {
    pub blend: Blend,
    pub action: u32,
    pub value: u32,
    pub blend2: Blend,
    pub action2: u32,
    pub value2: u32,
}

#[derive(Debug, Clone, Default)]
pub struct Item {
    pub id: u32,
    /// Clip object ids: one, or the variants of a blend space.
    pub clips: Vec<u32>,
    pub transitions: Vec<Slot<Transition>>,
    pub blend: Blend,
    pub displacement: Displacement,
    pub feet: (Feet, Feet),
    pub flags: [u8; 12],
    /// One weight per clip: the blend space's default mix (or other per-clip values).
    pub weights: Vec<f32>,
}

#[derive(Debug, Clone, Default)]
pub struct Action {
    pub id: u32,
    pub key: u32,
    pub transitions: [Option<Slot<Transition>>; 2],
    pub flags: [u8; 5],
    pub torso: u32,
    /// Actions played alongside this one, with weights.
    pub associated: Vec<(u32, f32)>,
    pub items: Vec<Item>,
}

#[derive(Debug, Clone, Default)]
pub struct ActionBlock {
    pub actions: Vec<Slot<Action>>,
    pub template: u32,
}

/// The kit: its blocks (object ids) and anim set layers.
#[derive(Debug, Clone)]
pub struct ActionKit {
    pub blocks: Vec<u32>,
    pub anim_sets: Vec<u32>,
}

struct R<'a> {
    d: &'a [u8],
    p: usize,
}

impl R<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8]> {
        ensure!(self.p + n <= self.d.len(), "action data ends at {:#x} (wanted {n} bytes)", self.p);
        let s = &self.d[self.p..self.p + n];
        self.p += n;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into()?))
    }
    fn f32(&mut self) -> Result<f32> {
        Ok(f32::from_bits(self.u32()?))
    }
    fn count(&mut self) -> Result<usize> {
        let at = self.p;
        let n = self.u32()? as usize;
        ensure!(n <= self.d.len(), "bad count {n} at {at:#x}");
        Ok(n)
    }
    fn bytes<const N: usize>(&mut self) -> Result<[u8; N]> {
        Ok(self.take(N)?.try_into()?)
    }
    /// An inline object's header: its id, checking its class.
    fn header(&mut self, class: u32) -> Result<u32> {
        let id = self.u32()?;
        let at = self.p;
        let c = self.u32()?;
        ensure!(c == class, "class {c:08x} at {at:#x}, wanted {class:08x}");
        Ok(id)
    }
    /// An owned object slot.
    fn slot<T>(&mut self, class: u32, body: impl FnOnce(&mut Self, u32) -> Result<T>) -> Result<Option<Slot<T>>> {
        let at = self.p;
        match self.u8()? {
            3 => Ok(None),
            2 => Ok(Some(Slot::Ref(self.u32()?))),
            0 => {
                let id = self.header(class)?;
                Ok(Some(Slot::Inline(body(self, id)?)))
            }
            k => bail!("slot kind {k} at {at:#x}"),
        }
    }
    fn blend(&mut self) -> Result<Blend> {
        let kind = self.u32()?;
        let flags = self.bytes()?;
        let modes = [self.u32()?, self.u32()?, self.u32()?];
        self.u8()?;
        let times = [self.f32()?, self.f32()?, self.f32()?];
        self.u8()?;
        let at = self.p;
        if self.u8()? != 3 {
            bail!("ActionBlendFrankenstein at {at:#x} (not in AC1's data)");
        }
        Ok(Blend { kind, flags, modes, times })
    }
    fn inline_blend(&mut self) -> Result<Blend> {
        self.header(CLASS_BLEND)?;
        self.blend()
    }
    fn transition(&mut self, _id: u32) -> Result<Transition> {
        let blend = self.inline_blend()?;
        let (action, value) = (self.u32()?, self.u32()?);
        let blend2 = self.inline_blend()?;
        let (action2, value2) = (self.u32()?, self.u32()?);
        Ok(Transition { blend, action, value, blend2, action2, value2 })
    }
    fn group(&mut self, _id: u32) -> Result<Vec<(u32, f32)>> {
        let mut v = vec![];
        for _ in 0..self.count()? {
            self.header(CLASS_ASSOCIATED)?;
            let (action, weight) = (self.u32()?, self.f32()?);
            self.u32()?;
            v.push((action, weight));
        }
        self.take(7)?;
        Ok(v)
    }
    fn item(&mut self, id: u32) -> Result<Item> {
        let clips = (0..self.count()?).map(|_| self.u32()).collect::<Result<_>>()?;
        let mut transitions = vec![];
        for _ in 0..self.count()? {
            transitions.extend(self.slot(CLASS_TRANSITION, Self::transition)?);
        }
        let blend = self.inline_blend()?;
        let displacement = match self.u32()? {
            0 => Displacement::Anim,
            1 => Displacement::Physics,
            2 => Displacement::Ai,
            n => Displacement::Other(n),
        };
        let feet = (feet(self.u32()?), feet(self.u32()?));
        let flags = self.bytes()?;
        self.f32()?;
        self.u32()?;
        self.u8()?;
        let weights = (0..self.count()?).map(|_| self.f32()).collect::<Result<_>>()?;
        Ok(Item { id, clips, transitions, blend, displacement, feet, flags, weights })
    }
    fn action(&mut self, id: u32) -> Result<Action> {
        let key = self.u32()?;
        self.slot(0, |_, _| Ok(()))?; // BodyPartChannel: always a reference
        let transitions = [self.slot(CLASS_TRANSITION, Self::transition)?, self.slot(CLASS_TRANSITION, Self::transition)?];
        let flags = self.bytes()?;
        let torso = self.u32()?;
        self.u32()?;
        self.u32()?;
        let associated = self.slot(CLASS_GROUP, Self::group)?.and_then(|s| s.inline().cloned()).unwrap_or_default();
        let mut items = vec![];
        for _ in 0..self.count()? {
            if let Some(Slot::Inline(i)) = self.slot(CLASS_ITEM, Self::item)? {
                items.push(i);
            }
        }
        Ok(Action { id, key, transitions, flags, torso, associated, items })
    }
}

fn feet(n: u32) -> Feet {
    match n {
        0 => Feet::NotSet,
        1 => Feet::Parallel,
        2 => Feet::RightAhead,
        3 => Feet::LeftAhead,
        n => Feet::Other(n),
    }
}

/// Parse an `ActionBlock` object's body (after its header).
pub fn parse_block(body: &[u8]) -> Result<ActionBlock> {
    let mut r = R { d: body, p: 0 };
    let mut actions = vec![];
    for _ in 0..r.count()? {
        actions.extend(r.slot(CLASS_ACTION, R::action)?);
    }
    let template = r.u32()?;
    r.slot(CLASS_GROUP, R::group)?;
    ensure!(r.p == body.len(), "{} bytes left at {:#x}", body.len() - r.p, r.p);
    Ok(ActionBlock { actions, template })
}

/// Parse an `ActionKit` object's body.
pub fn parse_kit(body: &[u8]) -> Result<ActionKit> {
    let mut r = R { d: body, p: 0 };
    let blocks = (0..r.count()?).map(|_| r.u32()).collect::<Result<_>>()?;
    r.slot(CLASS_ACTION, |_, _| Ok(()))?;
    r.u32()?;
    r.slot(CLASS_GROUP, R::group)?;
    let anim_sets = (0..r.count()?).map(|_| r.u32()).collect::<Result<_>>()?;
    ensure!(r.p == body.len(), "{} bytes left at {:#x}", body.len() - r.p, r.p);
    Ok(ActionKit { blocks, anim_sets })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blend(time: f32) -> Vec<u8> {
        let mut v = CLASS_BLEND.to_le_bytes().to_vec();
        v.splice(0..0, 0u32.to_le_bytes());
        v.extend(2u32.to_le_bytes());
        v.extend([0, 1, 0]);
        for m in [0u32, 1, 1] {
            v.extend(m.to_le_bytes());
        }
        v.push(0);
        for t in [time, 0.0, 0.0] {
            v.extend(t.to_le_bytes());
        }
        v.extend([0, 3]);
        v
    }

    /// A block with one action of one two-clip item (a blend space), built by hand.
    #[test]
    fn parses_a_block() {
        let mut d = 1u32.to_le_bytes().to_vec(); // one action
        d.push(0);
        d.extend(7u32.to_le_bytes());
        d.extend(CLASS_ACTION.to_le_bytes());
        d.extend(0x47u32.to_le_bytes()); // key
        d.push(2); // BodyPartChannel ref
        d.extend(9u32.to_le_bytes());
        d.extend([3, 3]); // no transitions
        d.extend([0, 0, 0, 1, 0]);
        for v in [0u32, 1, 0] {
            d.extend(v.to_le_bytes());
        }
        d.push(3); // no group
        d.extend(1u32.to_le_bytes()); // one item
        d.push(0);
        d.extend(8u32.to_le_bytes());
        d.extend(CLASS_ITEM.to_le_bytes());
        d.extend(2u32.to_le_bytes());
        d.extend(100u32.to_le_bytes());
        d.extend(101u32.to_le_bytes());
        d.extend(0u32.to_le_bytes()); // no transitions
        d.extend(blend(0.2));
        for v in [2u32, 1, 2] {
            d.extend(v.to_le_bytes());
        }
        d.extend([0u8; 12]);
        d.extend(1.0f32.to_le_bytes());
        d.extend(0u32.to_le_bytes());
        d.push(1);
        d.extend(2u32.to_le_bytes());
        d.extend(1.0f32.to_le_bytes());
        d.extend(0.0f32.to_le_bytes());
        d.extend(0x15cc7bu32.to_le_bytes()); // template
        d.push(3);
        let b = parse_block(&d).unwrap();
        let Slot::Inline(a) = &b.actions[0] else { panic!() };
        assert_eq!(a.key, 0x47);
        let i = &a.items[0];
        assert_eq!(i.clips, vec![100, 101]);
        assert_eq!(i.displacement, Displacement::Ai);
        assert_eq!(i.feet, (Feet::Parallel, Feet::RightAhead));
        assert_eq!(i.weights, vec![1.0, 0.0]);
        assert!((i.blend.times[0] - 0.2).abs() < 1e-6);
        assert_eq!(b.template, 0x15cc7b);
        // Truncated data is an error with an offset, not a panic.
        assert!(parse_block(&d[..d.len() - 3]).is_err());
    }
}
