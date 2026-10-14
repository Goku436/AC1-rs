//! The level editor (the ` key, beside 1): blocks, platforms, beams, posts, haystacks, walls, ladders and swing bars
//! placed in the test level or on empty ground, built with the test level's own builders (`level_file`), saved to and
//! loaded from `levels/<name>.level` beside the executable.
//!
//! In the editor the free camera flies (as P's: WASD, Space/E up, Ctrl/Q down, Shift fast, Alt slow, the mouse looks)
//! and the crosshair aims: the piece to place is outlined where it points (snapped to a 0.25 m grid, N toggles it).
//! 1-8 or the wheel pick the kind; the left button places one (and selects it), the right button selects the piece
//! under the crosshair (or none). The selection: V carries it with the crosshair (the left button drops it), the
//! arrows move it a step across the ground (by the camera's way), Page Up / Page Down up and down, R turns it 90
//! degrees (Shift the other way), T 15, U/J make it wider / narrower, I/K taller / lower, O/L deeper / shallower, C
//! puts hold rows up a box's or wall's faces (or takes them off), B duplicates it beside itself, Delete or X deletes
//! it. Z undoes (64 steps). Y saves (type a name, Enter), H loads one (by name; a level on another base starts the
//! game again in it). Every change rebuilds the level's gameplay data at once (the collision, holds, perches, bars,
//! ladders and haystacks: milliseconds). ` again (or P) leaves the editor and puts Altaïr on the ground under the
//! camera, facing its way, to play; ` again goes back to editing.
//!
//! Online (`net`): each game builds its own level, so both players start the same file (`AC1_LEVEL=<file>.level`,
//! the same file copied to both machines); only the players' poses go over the network.

use crate::level::Level;
use crate::level_file::{Base, Kind, LevelFile, Piece};
use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::prelude::*;

/// The grid pieces snap to, and the step of a move or resize without it (m).
const GRID: f32 = 0.25;
const FINE: f32 = 0.05;
/// Undo steps kept.
const UNDO: usize = 64;
/// The crosshair reaches this far (m).
const REACH: f32 = 300.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Prompt {
    Save,
    Load,
}

/// How much of the level is the base's (what a rebuild keeps).
#[derive(Default, Clone, Copy)]
struct Counts {
    tris: usize,
    ledges: usize,
    perches: usize,
    bars: usize,
    ladders: usize,
    haystacks: usize,
}

#[derive(Resource)]
pub struct Editor {
    pub on: bool,
    base: Base,
    pieces: Vec<Piece>,
    counts: Counts,
    /// A city: no editing.
    city: bool,
    kind: usize,
    /// The yaw a new piece is placed at (R / T with nothing selected).
    yaw: f32,
    selected: Option<usize>,
    carrying: bool,
    snap: bool,
    undo: Vec<Vec<Piece>>,
    /// The level's name (its file's stem), and whether it changed since saved.
    name: String,
    changed: bool,
    spawn: Option<Vec3>,
    prompt: Option<(Prompt, String)>,
    note: String,
    /// The level's gameplay data and meshes want rebuilding.
    dirty: bool,
    /// Leaving: put Altaïr on the ground under this point, facing this yaw (after the rebuild).
    leave: Option<(Vec3, f32)>,
    /// Where the crosshair points, snapped: a new piece goes there.
    aim: Option<Vec3>,
    look: Option<Look>,
    /// Turn the editor on at the first frame (`AC1_EDITOR=1`).
    auto: bool,
}

/// The pieces' materials.
struct Look {
    walls: Handle<StandardMaterial>,
    strips: Handle<StandardMaterial>,
    hay: Handle<StandardMaterial>,
    wood: Handle<StandardMaterial>,
}

/// A mesh drawn for a piece (all despawned and drawn again at a rebuild).
#[derive(Component)]
pub struct PieceMesh;

#[derive(Component)]
struct Panel;

#[derive(Component)]
struct Crosshair;

/// The panel's text and visibility.
type PanelText = (&'static mut Text, &'static mut Visibility);

impl Editor {
    fn push_undo(&mut self) {
        self.undo.push(self.pieces.clone());
        if self.undo.len() > UNDO {
            self.undo.remove(0);
        }
        self.changed = true;
        self.dirty = true;
    }

    fn step(&self) -> f32 {
        if self.snap { GRID } else { FINE }
    }

    fn kind(&self) -> Kind {
        Kind::ALL[self.kind]
    }
}

pub struct EditorPlugin;

impl Plugin for EditorPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreUpdate, input.after(bevy::ui::UiSystems::Focus))
            .add_systems(Update, apply.before(crate::player_input))
            .add_systems(Update, draw.after(crate::camera_follow));
    }
}

/// After the level is built (`level::spawn_level`) and before the player is placed (`setup`): what the base holds,
/// and the pieces of the level the game started in (`level_file::start_file`), built.
pub fn start(
    mut commands: Commands,
    mut level: ResMut<Level>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    old: Query<Entity, With<PieceMesh>>,
) {
    let counts = Counts {
        tris: level.tris.len(),
        ledges: level.ledges.len(),
        perches: level.perches.len(),
        bars: level.bars.len(),
        ladders: level.ladders.len(),
        haystacks: level.haystacks.len(),
    };
    let mat = |mats: &mut Assets<StandardMaterial>, c: Color| mats.add(StandardMaterial { base_color: c, perceptual_roughness: 0.9, ..default() });
    let look = Look {
        walls: mat(&mut mats, Color::srgb(0.78, 0.7, 0.58)),
        strips: mat(&mut mats, Color::srgb(0.55, 0.5, 0.42)),
        hay: mat(&mut mats, Color::srgb(0.86, 0.72, 0.33)),
        wood: mat(&mut mats, Color::srgb(0.42, 0.3, 0.18)),
    };
    let file = crate::level_file::start_file();
    let (name, f) = match file {
        Some((path, f)) => (path.as_ref().and_then(|p| p.file_stem()).map_or("untitled".into(), |s| s.to_string_lossy().into_owned()), f.clone()),
        None => ("untitled".into(), LevelFile::default()),
    };
    if let Some(s) = f.spawn {
        level.spawn = Some(s);
    }
    let mut ed = Editor {
        on: false,
        base: f.base,
        pieces: f.pieces,
        counts,
        city: level.city,
        kind: 0,
        yaw: 0.0,
        selected: None,
        carrying: false,
        snap: true,
        undo: vec![],
        name,
        changed: false,
        spawn: f.spawn,
        prompt: None,
        note: String::new(),
        dirty: false,
        leave: None,
        aim: None,
        look: Some(look),
        auto: std::env::var("AC1_EDITOR").is_ok(),
    };
    if !ed.pieces.is_empty() {
        rebuild(&mut ed, &mut level, &mut commands, &mut meshes, &old);
    }
    commands.spawn((
        Panel,
        Text::new(""),
        TextFont { font_size: bevy::text::FontSize::Px(14.0), ..default() },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
        Node { position_type: PositionType::Absolute, left: px(6), bottom: px(4), padding: UiRect::all(px(6)), max_width: px(760), ..default() },
        Visibility::Hidden,
        GlobalZIndex(5),
    ));
    commands.spawn((
        Crosshair,
        Text::new("+"),
        TextFont { font_size: bevy::text::FontSize::Px(22.0), ..default() },
        TextColor(Color::srgb(1.0, 0.95, 0.6)),
        Node {
            position_type: PositionType::Absolute,
            left: percent(50),
            top: percent(50),
            margin: UiRect { left: px(-6), top: px(-13), ..default() },
            ..default()
        },
        Visibility::Hidden,
        GlobalZIndex(5),
    ));
    commands.insert_resource(ed);
}

/// Put the level back to its base and build every piece into it again (the carried one left out: it is outlined).
fn rebuild(ed: &mut Editor, level: &mut Level, commands: &mut Commands, meshes: &mut Assets<Mesh>, old: &Query<Entity, With<PieceMesh>>) {
    let begun = std::time::Instant::now();
    for e in old {
        commands.entity(e).despawn();
    }
    let c = ed.counts;
    level.tris.truncate(c.tris);
    level.ledges.truncate(c.ledges);
    level.perches.truncate(c.perches);
    level.bars.truncate(c.bars);
    level.ladders.truncate(c.ladders);
    level.haystacks.truncate(c.haystacks);
    let Some(look) = &ed.look else { return };
    for (i, p) in ed.pieces.iter().enumerate() {
        if ed.carrying && ed.selected == Some(i) {
            continue;
        }
        let built = crate::level_file::build(p, level);
        let decor = if p.kind == Kind::Haystack { &look.hay } else { &look.wood };
        for (b, mat, solid) in [(built.walls, &look.walls, true), (built.strips, &look.strips, true), (built.decor, decor, false)] {
            if b.tris.is_empty() {
                continue;
            }
            let (mesh, tris) = b.mesh();
            if solid {
                level.tris.extend(tris);
            }
            commands.spawn((PieceMesh, Mesh3d(meshes.add(mesh)), MeshMaterial3d(mat.clone())));
        }
    }
    level.build_grid();
    ed.dirty = false;
    info!(
        "editor: {} pieces built, {} triangles, {} holds in the level, in {:.1} ms",
        ed.pieces.len(),
        level.tris.len(),
        level.ledges.len(),
        begun.elapsed().as_secs_f32() * 1000.0
    );
}

/// The editor's keys and clicks (`PreUpdate`): what it takes the game does not see.
#[allow(clippy::too_many_arguments)]
fn input(
    mut ed: Option<ResMut<Editor>>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut scroll: ResMut<AccumulatedMouseScroll>,
    mut typed: MessageReader<KeyboardInput>,
    mut cam: ResMut<crate::OrbitCam>,
    level: Res<Level>,
    menu: Res<crate::menu::Menu>,
    player: Query<&Transform, With<crate::character::Player>>,
    cursor: Query<&bevy::window::CursorOptions, With<bevy::window::PrimaryWindow>>,
    mut exit: MessageWriter<AppExit>,
) {
    let typed: Vec<KeyboardInput> = typed.read().filter(|k| k.state == ButtonState::Pressed).cloned().collect();
    let Some(ed) = ed.as_mut() else { return };
    if menu.open.is_some() {
        return;
    }
    let toggle = keys.just_pressed(KeyCode::Backquote) || std::mem::take(&mut ed.auto);
    if !ed.on {
        if toggle {
            if ed.city {
                warn!("editor: not in a city (the test level, AC1_LEVEL=empty or a saved .level)");
                return;
            }
            ed.on = true;
            ed.note = "editing".into();
            cam.free = Some(cam.free.unwrap_or(cam.eye));
            info!("editor: on");
        }
        return;
    }
    // Typing a name.
    if let Some((which, mut text)) = ed.prompt.take() {
        let mut done = None;
        for k in &typed {
            match &k.logical_key {
                Key::Enter => done = Some(true),
                Key::Escape => done = Some(false),
                Key::Backspace => {
                    text.pop();
                }
                _ => text.extend(k.text.as_deref().unwrap_or("").chars().filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-').take(40)),
            }
        }
        keys.reset_all();
        mouse.reset_all();
        scroll.delta = Vec2::ZERO;
        match done {
            Some(true) if !text.is_empty() => match which {
                Prompt::Save => save(ed, &text, player.single().ok().map(|t| t.translation)),
                Prompt::Load => load(ed, &text, &mut exit),
            },
            Some(_) => ed.note = "cancelled".into(),
            None => ed.prompt = Some((which, text)),
        }
        return;
    }
    if toggle || cam.free.is_none() {
        // Leave: play from the ground under the camera.
        let eye = cam.free.unwrap_or(cam.eye);
        ed.on = false;
        if ed.carrying {
            ed.carrying = false;
            ed.dirty = true;
        }
        ed.leave = Some((eye, cam.yaw));
        cam.free = None;
        cam.at = None;
        info!("editor: off, playing from {eye:.2}");
        return;
    }
    let rot = Quat::from_euler(EulerRot::YXZ, cam.yaw, cam.pitch, 0.0);
    let (eye, dir) = (cam.free.unwrap_or(cam.eye), rot * Vec3::NEG_Z);
    let step = ed.step();
    let snap = |v: f32, on: bool| if on { (v / GRID).round() * GRID } else { v };
    // The crosshair's point: what it meets in the level (a wall's face: out from it), else the ground plane.
    ed.aim = level
        .raycast(eye, dir, REACH)
        .map(|h| {
            if h.normal.y.abs() < 0.5 {
                let out = h.normal.with_y(0.0).normalize_or_zero();
                let p = h.point + out * 0.3;
                level.ground(p, 0.05, 50.0).map_or(p, |g| g.point)
            } else {
                h.point
            }
        })
        .or_else(|| (dir.y < -1e-3).then(|| eye + dir * (-eye.y / dir.y)))
        .map(|p| Vec3::new(snap(p.x, ed.snap), (p.y * 100.0).round() / 100.0, snap(p.z, ed.snap)));
    let grabbed = cursor.single().is_ok_and(|c| !c.visible);
    // The kind: 1-8, the wheel (the digits are the weapons' keys: not passed on).
    for (i, k) in [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4, KeyCode::Digit5, KeyCode::Digit6, KeyCode::Digit7, KeyCode::Digit8]
        .into_iter()
        .enumerate()
    {
        if keys.just_pressed(k) {
            ed.kind = i;
            keys.clear_just_pressed(k);
        }
    }
    if scroll.delta.y != 0.0 {
        let n = Kind::ALL.len() as i32;
        ed.kind = (ed.kind as i32 - scroll.delta.y.signum() as i32).rem_euclid(n) as usize;
        scroll.delta = Vec2::ZERO;
    }
    if keys.just_pressed(KeyCode::KeyN) {
        ed.snap ^= true;
    }
    if keys.just_pressed(KeyCode::KeyZ) {
        match ed.undo.pop() {
            Some(p) => {
                ed.pieces = p;
                ed.selected = None;
                ed.carrying = false;
                ed.dirty = true;
                ed.changed = true;
                ed.note = "undone".into();
            }
            None => ed.note = "nothing to undo".into(),
        }
    }
    if keys.just_pressed(KeyCode::KeyY) {
        ed.prompt = Some((Prompt::Save, if ed.name == "untitled" { String::new() } else { ed.name.clone() }));
        return;
    }
    if keys.just_pressed(KeyCode::KeyH) {
        ed.prompt = Some((Prompt::Load, String::new()));
        return;
    }
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let turn = if keys.just_pressed(KeyCode::KeyR) {
        90.0
    } else if keys.just_pressed(KeyCode::KeyT) {
        15.0
    } else {
        0.0
    } * if shift { -1.0 } else { 1.0 };
    // The buttons (taken from the game: the left is the weapon hand).
    if grabbed && mouse.just_pressed(MouseButton::Left) {
        if ed.carrying {
            ed.carrying = false;
            ed.dirty = true;
            ed.note = "dropped".into();
        } else if let Some(at) = ed.aim {
            ed.push_undo();
            let mut p = Piece::new(ed.kind(), at);
            p.yaw = ed.yaw;
            ed.pieces.push(p);
            ed.selected = Some(ed.pieces.len() - 1);
            ed.note = format!("placed a {}", p.kind.word());
        }
    }
    if grabbed && mouse.just_pressed(MouseButton::Right) && !ed.carrying {
        ed.selected = ed.pieces.iter().enumerate().filter_map(|(i, p)| p.ray_hit(eye, dir).map(|t| (i, t))).min_by(|a, b| a.1.total_cmp(&b.1)).map(|(i, _)| i);
        ed.note = ed.selected.map_or("nothing selected".into(), |i| format!("selected #{i}"));
    }
    if grabbed {
        mouse.clear_just_pressed(MouseButton::Left);
        mouse.clear_just_pressed(MouseButton::Right);
    }
    let Some(i) = ed.selected.filter(|&i| i < ed.pieces.len()) else {
        ed.selected = None;
        if turn != 0.0 {
            ed.yaw = (ed.yaw + turn).rem_euclid(360.0);
        }
        return;
    };
    if ed.carrying {
        if let Some(at) = ed.aim {
            ed.pieces[i].pos = at;
        }
    } else if keys.just_pressed(KeyCode::KeyV) {
        ed.push_undo();
        ed.carrying = true;
        ed.note = "carrying: the left button drops it".into();
    }
    // A change to the selection (undo kept first).
    let mut next = ed.pieces[i];
    let fwd = Vec3::new(-cam.yaw.sin(), 0.0, -cam.yaw.cos());
    let along = if fwd.x.abs() > fwd.z.abs() { Vec3::X * fwd.x.signum() } else { Vec3::Z * fwd.z.signum() };
    let side = Vec3::new(-along.z, 0.0, along.x);
    for (k, d) in [
        (KeyCode::ArrowUp, along),
        (KeyCode::ArrowDown, -along),
        (KeyCode::ArrowRight, side),
        (KeyCode::ArrowLeft, -side),
        (KeyCode::PageUp, Vec3::Y),
        (KeyCode::PageDown, Vec3::NEG_Y),
    ] {
        if keys.just_pressed(k) {
            next.pos += d * step;
        }
    }
    if turn != 0.0 {
        next.yaw = (next.yaw + turn).rem_euclid(360.0);
        ed.yaw = next.yaw;
    }
    for (k, axis, by) in [
        (KeyCode::KeyU, 0, 1.0),
        (KeyCode::KeyJ, 0, -1.0),
        (KeyCode::KeyI, 1, 1.0),
        (KeyCode::KeyK, 1, -1.0),
        (KeyCode::KeyO, 2, 1.0),
        (KeyCode::KeyL, 2, -1.0),
    ] {
        if keys.just_pressed(k) {
            next.size[axis] = (next.size[axis] + by * step).clamp(0.1, 200.0);
        }
    }
    if keys.just_pressed(KeyCode::KeyC) && next.kind.takes_holds() {
        next.holds ^= true;
    }
    if next != ed.pieces[i] {
        ed.push_undo();
        ed.pieces[i] = next;
        ed.note = format!("#{i} changed");
    }
    if keys.just_pressed(KeyCode::KeyB) {
        ed.push_undo();
        let mut copy = ed.pieces[i];
        let (lo, hi) = copy.local_bounds();
        copy.pos += copy.rotation() * Vec3::X * (hi.x - lo.x);
        ed.pieces.push(copy);
        ed.selected = Some(ed.pieces.len() - 1);
        ed.note = format!("duplicated #{i}");
    }
    if keys.just_pressed(KeyCode::Delete) || keys.just_pressed(KeyCode::KeyX) {
        ed.push_undo();
        ed.pieces.remove(i);
        ed.selected = None;
        ed.carrying = false;
        ed.note = format!("deleted #{i}");
    }
}

fn save(ed: &mut Editor, name: &str, at: Option<Vec3>) {
    let dir = crate::level_file::levels_dir();
    let path = dir.join(format!("{name}.level"));
    // (Altaïr starts where he stands now.)
    ed.spawn = at.or(ed.spawn);
    let file = LevelFile { base: ed.base, spawn: ed.spawn, pieces: ed.pieces.clone() };
    match std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&path, file.to_text())) {
        Ok(()) => {
            ed.name = name.to_string();
            ed.changed = false;
            ed.note = format!("saved {}", path.display());
            info!("editor: saved {} ({} pieces)", path.display(), ed.pieces.len());
        }
        Err(e) => ed.note = format!("could not save {}: {e}", path.display()),
    }
}

fn load(ed: &mut Editor, name: &str, exit: &mut MessageWriter<AppExit>) {
    let path = crate::level_file::resolve(name);
    let file = match LevelFile::load(&path) {
        Ok(f) => f,
        Err(e) => {
            ed.note = e;
            return;
        }
    };
    if file.base != ed.base {
        // (Another base: the game starts again in it, as the map menu does.)
        let Ok(exe) = std::env::current_exe() else { return };
        let mut c = std::process::Command::new(exe);
        c.args(std::env::args_os().skip(1)).env("AC1_LEVEL", std::path::absolute(&path).unwrap_or(path.clone())).env_remove("AC1_LEVEL_FILE");
        c.env_remove("AC1_START").env_remove("AC1_FREECAM");
        match c.spawn() {
            Ok(_) => {
                info!("editor: {} is on the {} base: starting again in it", path.display(), file.base.word());
                exit.write(AppExit::Success);
            }
            Err(e) => ed.note = format!("could not start the game again: {e}"),
        }
        return;
    }
    ed.push_undo();
    ed.pieces = file.pieces;
    ed.spawn = file.spawn.or(ed.spawn);
    ed.selected = None;
    ed.carrying = false;
    ed.name = name.to_string();
    ed.changed = false;
    ed.note = format!("loaded {}", path.display());
}

/// Rebuild what changed, and after leaving put Altaïr down (`Update`, before the player's input).
fn apply(
    mut commands: Commands,
    ed: Option<ResMut<Editor>>,
    mut level: ResMut<Level>,
    mut meshes: ResMut<Assets<Mesh>>,
    old: Query<Entity, With<PieceMesh>>,
    mut q: Query<(&mut Transform, &mut crate::character::Controller, &mut crate::character::Character), With<crate::character::Player>>,
) {
    let Some(mut ed) = ed else { return };
    if ed.dirty {
        rebuild(&mut ed, &mut level, &mut commands, &mut meshes, &old);
    }
    let Some((eye, yaw)) = ed.leave.take() else { return };
    let Ok((mut tf, mut ctl, mut ch)) = q.single_mut() else { return };
    let at = level.ground(eye, 0.0, 600.0).map_or(Vec3::new(eye.x, 0.0, eye.z), |g| g.point);
    tf.translation = at;
    tf.rotation = Quat::from_rotation_y(yaw);
    // (Fresh, as a control run starts.)
    ch.wall = None;
    ch.velocity = Vec3::ZERO;
    ch.fall_v = 0.0;
    ch.edge_lock = None;
    ch.exit_fade = None;
    ch.ragdoll = None;
    *ctl = crate::character::Controller::default();
}

/// The crosshair, the outlines (the piece to place, the selection) and the panel.
fn draw(
    ed: Option<Res<Editor>>,
    overlay: Res<crate::Overlay>,
    mut gizmos: Gizmos,
    mut panel: Query<PanelText, (With<Panel>, Without<Crosshair>)>,
    mut cross: Query<&mut Visibility, (With<Crosshair>, Without<Panel>)>,
) {
    let Some(ed) = ed else { return };
    let show = ed.on && !overlay.hidden;
    if let Ok(mut v) = cross.single_mut() {
        v.set_if_neq(if show { Visibility::Visible } else { Visibility::Hidden });
    }
    let Ok((mut text, mut vis)) = panel.single_mut() else { return };
    vis.set_if_neq(if show { Visibility::Visible } else { Visibility::Hidden });
    if !ed.on {
        return;
    }
    let outline = |gizmos: &mut Gizmos, p: &Piece, c: Color| {
        let (lo, hi) = p.local_bounds();
        let tf = Transform { translation: p.world_point((lo + hi) * 0.5), rotation: p.rotation(), scale: (hi - lo).max(Vec3::splat(0.02)) };
        gizmos.cube(tf, c);
        gizmos.line(p.pos, p.world_point(Vec3::NEG_Z * ((hi.z - lo.z) * 0.5 + 0.4)), c);
    };
    let sel = ed.selected.and_then(|i| ed.pieces.get(i));
    if let Some(p) = sel {
        outline(&mut gizmos, p, if ed.carrying { Color::srgb(0.3, 1.0, 0.4) } else { Color::srgb(1.0, 0.85, 0.2) });
    }
    if !ed.carrying
        && let Some(at) = ed.aim
    {
        let mut ghost = Piece::new(ed.kind(), at);
        ghost.yaw = ed.yaw;
        outline(&mut gizmos, &ghost, Color::srgba(0.6, 0.85, 1.0, 0.8));
    }
    let kinds: Vec<String> =
        Kind::ALL.iter().enumerate().map(|(i, k)| if i == ed.kind { format!("[{} {}]", i + 1, k.word()) } else { format!("{} {}", i + 1, k.word()) }).collect();
    let mut t = format!(
        "LEVEL EDITOR: {}{} ({} pieces, {} base)   grid {} (N)\n{}\n",
        ed.name,
        if ed.changed { " *" } else { "" },
        ed.pieces.len(),
        ed.base.word(),
        if ed.snap { "0.25 m" } else { "off" },
        kinds.join("  ")
    );
    t += &match sel {
        Some(p) => format!(
            "selected #{}: {} at ({:.2}, {:.2}, {:.2}), {:.0} deg, {:.2} x {:.2} x {:.2} m{}\n",
            ed.selected.unwrap_or(0),
            p.kind.word(),
            p.pos.x,
            p.pos.y,
            p.pos.z,
            p.yaw,
            p.size.x,
            p.size.y,
            p.size.z,
            if p.holds { ", holds up its faces" } else { "" }
        ),
        None => "nothing selected\n".into(),
    };
    t += "1-8 / wheel: piece   left button: place   right button: select\n\
          V carry (left button drops)   B duplicate   Del / X delete   Z undo\n\
          arrows, PgUp / PgDn: move   R / Shift+R: turn 90   T / Shift+T: turn 15\n\
          U/J wider/narrower   I/K taller/lower   O/L deeper/shallower   C holds up the faces\n\
          Y save as   H load   ` (or P) play from the ground under the camera\n\
          fly: WASD, Space/E up, Ctrl/Q down, Shift fast, Alt slow; click to take the mouse";
    if let Some((which, typed)) = &ed.prompt {
        let files: Vec<String> = std::fs::read_dir(crate::level_file::levels_dir())
            .map(|d| {
                d.filter_map(|e| Some(e.ok()?.path()))
                    .filter(|p| p.extension().is_some_and(|x| x == "level"))
                    .filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
                    .collect()
            })
            .unwrap_or_default();
        t += &format!(
            "\n\n{} {}_   (Enter: ok, Esc: cancel)\nin {}: {}",
            if *which == Prompt::Save { "SAVE AS:" } else { "LOAD:" },
            typed,
            crate::level_file::levels_dir().display(),
            if files.is_empty() { "none yet".into() } else { files.join(", ") }
        );
    }
    if !ed.note.is_empty() {
        t += &format!("\n> {}", ed.note);
    }
    if text.0 != t {
        text.0 = t;
    }
}
