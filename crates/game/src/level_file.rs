//! Editor levels (`editor`): pieces placed on a base (the test level, or empty ground), saved as a plain text file.
//! Every piece is built with the test level's own builders (`level::building`, the cuboids, perches, bars, ladders
//! and haystacks as `level::spawn_level` makes them), in the piece's own frame, then turned by its yaw about the
//! vertical and moved to its place: climbed, jumped onto and walked along as the test level's.
//!
//! The file (`levels/<name>.level` beside the executable; `AC1_LEVEL=<file>.level` starts in one), one item a line,
//! `#` starts a comment, Bevy space (Y up, metres, degrees):
//!
//! ```text
//! ac1-level 1
//! base empty                      # or `test`: the pieces stand in the test level
//! spawn 0 0 0                     # optional: where Altaïr starts (without AC1_START)
//! box 0 0 6 0 6 5.99 2 holds      # <kind> x y z yaw width height depth [holds]
//! ```
//!
//! `x y z` is the middle of the piece's foot (its base), `yaw` turns it about the vertical (positive from +X toward
//! -Z, as Bevy's), width runs along its own X, depth along its own Z, height up. The kinds:
//! - `box`: a building (`level::building`): a hold along every top edge; `holds` adds rows every 0.6 m from 1.79 m up
//!   all four faces, to climb.
//! - `platform`: a plain block to stand on (no holds).
//! - `beam`: a beam `width` long, its top `height` up, `depth` wide (0.2 m thick), a perch along its top.
//! - `post`: a post `height` tall, a perch on its top.
//! - `haystack`: hay to dive into, `width` across (square, not turned: AC1's footprint test is axis-aligned here), `height` tall.
//! - `wall`: a plain wall (a passover wall at 1.4 m); `holds` makes it a building with hold rows up its faces.
//! - `ladder`: a ladder `height` tall, its front to the piece's -Z (stand it 0.08 m out from a wall's face).
//! - `bar`: a swing bar `width` long, `height` up, on two uprights just past its ends.

use crate::level::{Builder, HayStack, Ladder, Ledge, Level, Line, Tri};
use bevy::prelude::*;
use std::path::{Path, PathBuf};

/// The first line of a level file.
pub const MAGIC: &str = "ac1-level 1";

/// What the pieces stand in.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Base {
    /// The test level (`level::spawn_level`).
    #[default]
    Test,
    /// Flat ground and nothing else.
    Empty,
}

impl Base {
    pub fn word(self) -> &'static str {
        match self {
            Base::Test => "test",
            Base::Empty => "empty",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Box,
    Platform,
    Beam,
    Post,
    Haystack,
    Wall,
    Ladder,
    Bar,
}

impl Kind {
    /// In the editor's order (its keys 1-8).
    pub const ALL: [Kind; 8] = [Kind::Box, Kind::Platform, Kind::Beam, Kind::Post, Kind::Haystack, Kind::Wall, Kind::Ladder, Kind::Bar];

    pub fn word(self) -> &'static str {
        match self {
            Kind::Box => "box",
            Kind::Platform => "platform",
            Kind::Beam => "beam",
            Kind::Post => "post",
            Kind::Haystack => "haystack",
            Kind::Wall => "wall",
            Kind::Ladder => "ladder",
            Kind::Bar => "bar",
        }
    }

    pub fn parse(w: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|k| k.word() == w)
    }

    /// A new piece's size (width, height, depth; m): the test level's own (a 3.59 m building, AC1's rows; a 2 m
    /// platform; a 0.3 m beam 2 m up; a 0.4 m post; its haystacks; a 1.4 m passover wall; a 5 m ladder; a 4.4 m bar).
    pub fn default_size(self) -> Vec3 {
        match self {
            Kind::Box => Vec3::new(4.0, 1.79 + 0.6 * 3.0, 4.0),
            Kind::Platform => Vec3::new(3.0, 2.0, 4.0),
            Kind::Beam => Vec3::new(4.0, 2.0, 0.3),
            Kind::Post => Vec3::new(0.4, 2.0, 0.4),
            Kind::Haystack => Vec3::new(2.6, 1.7, 2.6),
            Kind::Wall => Vec3::new(4.0, 1.4, 0.3),
            Kind::Ladder => Vec3::new(0.5, 5.0, 0.1),
            Kind::Bar => Vec3::new(3.2, 4.4, 0.2),
        }
    }

    /// Whether `holds` means anything for it.
    pub fn takes_holds(self) -> bool {
        matches!(self, Kind::Box | Kind::Wall)
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Piece {
    pub kind: Kind,
    /// The middle of its foot.
    pub pos: Vec3,
    /// Degrees about the vertical.
    pub yaw: f32,
    /// Width (own X), height, depth (own Z).
    pub size: Vec3,
    pub holds: bool,
}

impl Piece {
    pub fn new(kind: Kind, pos: Vec3) -> Piece {
        Piece { kind, pos, yaw: 0.0, size: kind.default_size(), holds: kind == Kind::Box }
    }

    pub fn rotation(&self) -> Quat {
        Quat::from_rotation_y(self.yaw.to_radians())
    }

    /// Its own frame to the world.
    pub fn world_point(&self, p: Vec3) -> Vec3 {
        self.pos + self.rotation() * p
    }

    /// Its bounds in its own frame (min, max): what is drawn round it and picked.
    pub fn local_bounds(&self) -> (Vec3, Vec3) {
        let s = self.size;
        let (hw, hd) = (s.x * 0.5, s.z * 0.5);
        match self.kind {
            Kind::Beam => (Vec3::new(-hw, s.y - 0.2, -hd), Vec3::new(hw, s.y, hd)),
            Kind::Haystack => (Vec3::new(-hw, 0.0, -hw), Vec3::new(hw, s.y, hw)),
            Kind::Ladder => (Vec3::new(-0.3, 0.0, -0.05), Vec3::new(0.3, s.y + 0.9, 0.05)),
            Kind::Bar => (Vec3::new(-hw - 0.2, 0.0, -0.1), Vec3::new(hw + 0.2, s.y + 0.1, 0.1)),
            _ => (Vec3::new(-hw, 0.0, -hd), Vec3::new(hw, s.y, hd)),
        }
    }

    /// Where a ray from `o` along unit `d` meets its bounds (the distance), if it does.
    pub fn ray_hit(&self, o: Vec3, d: Vec3) -> Option<f32> {
        let inv = self.rotation().inverse();
        let (o, d) = (inv * (o - self.pos), inv * d);
        let (lo, hi) = self.local_bounds();
        let (mut t0, mut t1) = (0.0f32, f32::MAX);
        for i in 0..3 {
            if d[i].abs() < 1e-6 {
                if o[i] < lo[i] || o[i] > hi[i] {
                    return None;
                }
                continue;
            }
            let (a, b) = ((lo[i] - o[i]) / d[i], (hi[i] - o[i]) / d[i]);
            t0 = t0.max(a.min(b));
            t1 = t1.min(a.max(b));
        }
        (t0 <= t1).then_some(t0)
    }
}

/// A piece built: what collides (`walls`, and the hold strips `strips`, as the test level's), what is only drawn
/// (`decor`: hay, ladder rails and rungs, the bar), all in the world.
#[derive(Default)]
pub struct Built {
    pub walls: Builder,
    pub strips: Builder,
    pub decor: Builder,
}

/// Build `p` into `level` (its holds, perches, bars, ladders and haystacks; its triangles are the caller's to add,
/// from what is returned, as `spawn_level`'s `add` does).
pub fn build(p: &Piece, level: &mut Level) -> Built {
    let mut b = Built::default();
    let mut own = Level::default();
    let s = p.size;
    let (hw, hd) = (s.x * 0.5, s.z * 0.5);
    match p.kind {
        Kind::Box => {
            let faces = if p.holds { vec![Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z] } else { vec![] };
            crate::level::building(&mut b.walls, &mut b.strips, &mut own, Vec3::new(-hw, 0.0, -hd), Vec3::new(hw, s.y, hd), &faces);
        }
        Kind::Wall if p.holds => {
            let faces = [Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z];
            crate::level::building(&mut b.walls, &mut b.strips, &mut own, Vec3::new(-hw, 0.0, -hd), Vec3::new(hw, s.y, hd), &faces);
        }
        Kind::Platform | Kind::Wall => b.walls.cuboid(Vec3::new(-hw, 0.0, -hd), Vec3::new(hw, s.y, hd)),
        Kind::Beam => {
            b.walls.cuboid(Vec3::new(-hw, s.y - 0.2, -hd), Vec3::new(hw, s.y, hd));
            own.perches.push(Line { a: Vec3::new(-hw, s.y, 0.0), b: Vec3::new(hw, s.y, 0.0) });
        }
        Kind::Post => {
            b.walls.cuboid(Vec3::new(-hw, 0.0, -hd), Vec3::new(hw, s.y, hd));
            own.perches.push(Line { a: Vec3::new(0.0, s.y, 0.0), b: Vec3::new(0.0, s.y, 0.0) });
        }
        Kind::Haystack => {
            b.decor.cuboid(Vec3::new(-hw, 0.0, -hw), Vec3::new(hw, s.y, hw));
            own.haystacks.push(HayStack { centre: Vec3::ZERO, half: hw, height: s.y, facing: Vec3::ZERO, own: None });
        }
        Kind::Ladder => {
            let h = s.y + 0.9;
            for x in [-0.25, 0.25] {
                b.decor.cuboid(Vec3::new(x - 0.025, 0.0, -0.025), Vec3::new(x + 0.025, h, 0.025));
            }
            let mut y = 0.28;
            while y < h - 0.1 {
                b.decor.cuboid(Vec3::new(-0.25, y - 0.02, -0.02), Vec3::new(0.25, y + 0.02, 0.02));
                y += 0.25;
            }
            own.ladders.push(Ladder::upright(Vec3::ZERO, s.y, Vec3::NEG_Z));
        }
        Kind::Bar => {
            for x in [-hw - 0.1, hw + 0.1] {
                b.walls.cuboid(Vec3::new(x - 0.1, 0.0, -0.1), Vec3::new(x + 0.1, s.y + 0.1, 0.1));
            }
            b.decor.cuboid(Vec3::new(-hw, s.y - 0.04, -0.04), Vec3::new(hw, s.y + 0.04, 0.04));
            own.bars.push(Line { a: Vec3::new(-hw, s.y, 0.0), b: Vec3::new(hw, s.y, 0.0) });
        }
    }
    let rot = p.rotation();
    let at = |v: Vec3| p.pos + rot * v;
    for bld in [&mut b.walls, &mut b.strips, &mut b.decor] {
        for t in &mut bld.tris {
            *t = Tri { a: at(t.a), b: at(t.b), c: at(t.c) };
        }
    }
    level.ledges.extend(own.ledges.iter().map(|l| Ledge { a: at(l.a), b: at(l.b), out: rot * l.out }));
    level.perches.extend(own.perches.iter().map(|l| Line { a: at(l.a), b: at(l.b) }));
    level.bars.extend(own.bars.iter().map(|l| Line { a: at(l.a), b: at(l.b) }));
    level.ladders.extend(own.ladders.iter().map(|l| Ladder { base: at(l.base), top: p.pos.y + l.top, out: rot * l.out, lean: rot * l.lean }));
    level.haystacks.extend(own.haystacks.iter().map(|h| HayStack { centre: at(h.centre), ..*h }));
    b
}

/// A level file's contents.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct LevelFile {
    pub base: Base,
    pub spawn: Option<Vec3>,
    pub pieces: Vec<Piece>,
}

/// A number as written: to the millimetre, no trailing zeros.
fn num(v: f32) -> String {
    let s = format!("{:.3}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.into() }
}

impl LevelFile {
    pub fn to_text(&self) -> String {
        let mut out = format!("{MAGIC}\n# ac1-rs editor level: <kind> x y z yaw width height depth [holds] (Bevy space, m, degrees)\n");
        out += &format!("base {}\n", self.base.word());
        if let Some(s) = self.spawn {
            out += &format!("spawn {} {} {}\n", num(s.x), num(s.y), num(s.z));
        }
        for p in &self.pieces {
            let [x, y, z] = p.pos.to_array().map(num);
            let [w, h, d] = p.size.to_array().map(num);
            out += &format!("{} {x} {y} {z} {} {w} {h} {d}{}\n", p.kind.word(), num(p.yaw), if p.holds { " holds" } else { "" });
        }
        out
    }

    /// Read a level file; an error names the line.
    pub fn parse(text: &str) -> Result<LevelFile, String> {
        let mut file = LevelFile::default();
        let mut magic = false;
        for (i, raw) in text.lines().enumerate() {
            let line = raw.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let at = |e: &str| format!("line {}: {e} ({raw:?})", i + 1);
            if !magic {
                if line != MAGIC {
                    return Err(at(&format!("not an ac1-rs level (want `{MAGIC}` first)")));
                }
                magic = true;
                continue;
            }
            let words: Vec<&str> = line.split_whitespace().collect();
            let nums = |w: &[&str]| -> Result<Vec<f32>, String> {
                w.iter().map(|v| v.parse::<f32>().ok().filter(|f| f.is_finite()).ok_or_else(|| at(&format!("{v:?} is not a number")))).collect()
            };
            match words[0] {
                "base" => {
                    file.base = match words.get(1).copied() {
                        Some("test") => Base::Test,
                        Some("empty") => Base::Empty,
                        _ => return Err(at("want `base test` or `base empty`")),
                    }
                }
                "spawn" => {
                    let v = nums(&words[1..])?;
                    if v.len() != 3 {
                        return Err(at("want `spawn x y z`"));
                    }
                    file.spawn = Some(Vec3::new(v[0], v[1], v[2]));
                }
                w => {
                    let kind = Kind::parse(w).ok_or_else(|| at(&format!("unknown piece {w:?}")))?;
                    let holds = words.last() == Some(&"holds");
                    let v = nums(&words[1..words.len() - holds as usize])?;
                    if v.len() != 7 {
                        return Err(at("want `<kind> x y z yaw width height depth [holds]`"));
                    }
                    let size = Vec3::new(v[4], v[5], v[6]);
                    if size.min_element() <= 0.0 || size.max_element() > 500.0 {
                        return Err(at("a size must be over 0 and at most 500 m"));
                    }
                    file.pieces.push(Piece { kind, pos: Vec3::new(v[0], v[1], v[2]), yaw: v[3], size, holds });
                }
            }
        }
        if !magic {
            return Err("empty file".into());
        }
        Ok(file)
    }

    pub fn load(path: &Path) -> Result<LevelFile, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        LevelFile::parse(&text).map_err(|e| format!("{}: {e}", path.display()))
    }
}

/// `levels/` beside the executable.
pub fn levels_dir() -> PathBuf {
    std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join("levels"))).unwrap_or_else(|| PathBuf::from("levels"))
}

/// Is `AC1_LEVEL` a saved level (a `.level` file) or `empty`, not a city?
pub fn is_editor_level(v: &str) -> bool {
    v.eq_ignore_ascii_case("empty") || v.to_ascii_lowercase().ends_with(".level")
}

/// A level's file from what was asked for: the path as given (from the working directory), else in `levels/`; a bare
/// name gets `.level`.
pub fn resolve(name: &str) -> PathBuf {
    let file = if name.to_ascii_lowercase().ends_with(".level") { name.to_string() } else { format!("{name}.level") };
    let given = PathBuf::from(&file);
    if given.exists() || given.is_absolute() { given } else { levels_dir().join(file) }
}

/// The level the game started in (`AC1_LEVEL_FILE`, which `main` sets from an `AC1_LEVEL` that `is_editor_level`):
/// its path (None for `empty`) and contents. A file that can't be read is logged and starts empty ground.
pub fn start_file() -> Option<&'static (Option<PathBuf>, LevelFile)> {
    static START: std::sync::OnceLock<Option<(Option<PathBuf>, LevelFile)>> = std::sync::OnceLock::new();
    START
        .get_or_init(|| {
            let v = std::env::var("AC1_LEVEL_FILE").ok()?;
            if v.eq_ignore_ascii_case("empty") {
                return Some((None, LevelFile { base: Base::Empty, ..default() }));
            }
            let path = resolve(&v);
            match LevelFile::load(&path) {
                Ok(f) => {
                    info!("editor level {}: {} pieces on the {} base", path.display(), f.pieces.len(), f.base.word());
                    Some((Some(path), f))
                }
                Err(e) => {
                    error!("editor level: {e}; starting on empty ground");
                    Some((None, LevelFile { base: Base::Empty, ..default() }))
                }
            }
        })
        .as_ref()
}

/// The base the game started on.
pub fn start_base() -> Base {
    start_file().map_or(Base::Test, |(_, f)| f.base)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> LevelFile {
        LevelFile {
            base: Base::Empty,
            spawn: Some(Vec3::new(1.5, 0.0, -2.25)),
            pieces: vec![
                Piece { kind: Kind::Box, pos: Vec3::new(0.0, 0.0, 6.0), yaw: 0.0, size: Vec3::new(6.0, 5.99, 2.0), holds: true },
                Piece { kind: Kind::Beam, pos: Vec3::new(-3.25, 0.5, 1.0), yaw: 15.0, size: Vec3::new(4.0, 2.0, 0.3), holds: false },
                Piece { kind: Kind::Ladder, pos: Vec3::new(2.0, 0.0, 4.92), yaw: -90.0, size: Vec3::new(0.5, 5.0, 0.1), holds: false },
                Piece::new(Kind::Bar, Vec3::new(10.0, 0.0, 0.0)),
                Piece::new(Kind::Haystack, Vec3::new(-10.0, 0.0, 0.0)),
            ],
        }
    }

    #[test]
    fn a_level_saved_and_read_back_is_the_same() {
        let f = sample();
        let text = f.to_text();
        assert!(text.starts_with(MAGIC));
        assert_eq!(LevelFile::parse(&text), Ok(f.clone()));
        // (And through a file.)
        let path = std::env::temp_dir().join(format!("ac1-level-test-{}.level", std::process::id()));
        std::fs::write(&path, &text).unwrap();
        let back = LevelFile::load(&path);
        let _ = std::fs::remove_file(&path);
        assert_eq!(back, Ok(f));
    }

    #[test]
    fn bad_lines_are_errors_with_their_line() {
        assert!(LevelFile::parse("").is_err());
        assert!(LevelFile::parse("box 0 0 0 0 1 1 1").unwrap_err().contains("line 1"));
        let e = LevelFile::parse(&format!("{MAGIC}\nbase empty\nbox 0 0 zero 0 1 1 1\n")).unwrap_err();
        assert!(e.contains("line 3") && e.contains("zero"), "{e}");
        assert!(LevelFile::parse(&format!("{MAGIC}\ntower 0 0 0 0 1 1 1\n")).unwrap_err().contains("unknown piece"));
        assert!(LevelFile::parse(&format!("{MAGIC}\nbox 0 0 0 0 1 -1 1\n")).is_err());
        assert!(LevelFile::parse(&format!("{MAGIC}\nbox 0 0 0 0 1 1\n")).is_err());
        // (Comments and blank lines are fine.)
        let f = LevelFile::parse(&format!("# made by hand\n{MAGIC}\n\npost 1 0 2 0 0.4 2 0.4 # a post\n")).unwrap();
        assert_eq!(f.base, Base::Test);
        assert_eq!(f.pieces.len(), 1);
    }

    #[test]
    fn a_box_with_holds_has_the_test_levels_rows_turned_with_it() {
        let mut level = Level::default();
        let p = Piece { kind: Kind::Box, pos: Vec3::new(10.0, 1.0, 0.0), yaw: 90.0, size: Vec3::new(4.0, 3.59, 2.0), holds: true };
        let b = build(&p, &mut level);
        assert_eq!(b.walls.tris.len(), 12);
        // Rows at 1.79, 2.39, 2.99 and the top edge at 3.59 on each of the four faces, over the piece's foot.
        assert_eq!(level.ledges.len(), 16);
        let mut ys: Vec<i32> = level.ledges.iter().map(|l| ((l.a.y - 1.0) * 100.0).round() as i32).collect();
        ys.sort();
        ys.dedup();
        assert_eq!(ys, vec![179, 239, 299, 359]);
        // Turned 90 degrees, its own -Z face (4 m wide, at z -1) faces world -X, 1 m (+ the 0.06 m grab line) out.
        let front: Vec<&Ledge> = level.ledges.iter().filter(|l| l.out.distance(Vec3::NEG_X) < 1e-4).collect();
        assert_eq!(front.len(), 4);
        for l in front {
            assert!((l.a.x - (10.0 - 1.06)).abs() < 1e-4, "{l:?}");
            assert!((l.a.distance(l.b) - (4.0 - 0.16)).abs() < 1e-4);
        }
        // Every triangle is inside its turned bounds.
        for t in b.walls.tris.iter().chain(&b.strips.tris) {
            for v in [t.a, t.b, t.c] {
                assert!((v.x - 10.0).abs() <= 1.2 && v.z.abs() <= 2.13 && v.y >= 1.0 - 0.08 && v.y <= 4.6, "{v}");
            }
        }
    }

    #[test]
    fn pieces_give_their_perches_bars_ladders_and_hay() {
        let mut level = Level::default();
        for k in Kind::ALL {
            build(&Piece::new(k, Vec3::new(0.0, 0.5, 0.0)), &mut level);
        }
        assert_eq!(level.perches.len(), 2);
        assert_eq!(level.bars.len(), 1);
        assert_eq!(level.ladders.len(), 1);
        assert!((level.ladders[0].top - 5.5).abs() < 1e-5);
        assert_eq!(level.haystacks.len(), 1);
        assert!((level.haystacks[0].half - 1.3).abs() < 1e-5);
        // A box's top edges.
        assert_eq!(level.ledges.len(), 4 * 4);
    }

    #[test]
    fn a_ray_picks_a_turned_piece() {
        let p = Piece { kind: Kind::Wall, pos: Vec3::new(0.0, 0.0, -5.0), yaw: 45.0, size: Vec3::new(4.0, 2.0, 0.3), holds: false };
        let t = p.ray_hit(Vec3::new(0.0, 1.0, 0.0), Vec3::NEG_Z).expect("hits");
        assert!(t > 4.5 && t < 5.0, "{t}");
        assert!(p.ray_hit(Vec3::new(0.0, 3.0, 0.0), Vec3::NEG_Z).is_none());
        assert!(p.ray_hit(Vec3::new(0.0, 1.0, 0.0), Vec3::Z).is_none());
    }
}
