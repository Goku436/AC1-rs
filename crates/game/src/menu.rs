//! The in-game menus, drawn as the F2 map menu is: F8 the graphics settings (`settings`), F10 online (`net`: hosting
//! on a port, joining an address, the replay recorder). Keyboard: Up / Down pick a row, Left / Right change it, Enter
//! presses a button or edits a text field (typing, Backspace, Enter keeps it, Esc drops it), Esc or the menu's key
//! closes. Mouse: the cursor is freed while a menu is open; a click changes a row (the right button backwards), the
//! wheel too. While a menu is open the game gets no keys or buttons; closing it gives the cursor back to the camera if
//! it had it. Every change is saved to the settings file at once.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use crate::settings::{AntiAlias, DetailLevel, FRAME_CAPS, Preset, RESOLUTIONS, Settings, Shadows, TEXTURES, VIEWS, WindowMode, word};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Which {
    Graphics,
    Online,
}

#[derive(Resource, Default)]
pub struct Menu {
    pub open: Option<Which>,
    row: usize,
    /// A text field being typed into, and its text so far.
    edit: Option<(Id, String)>,
    /// The camera had the cursor as the menu opened.
    held: bool,
    /// The last thing to tell (a save, an error).
    note: String,
    /// The next frame starts the game again (the graphics menu's Restart).
    restart: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Id {
    Preset,
    Window,
    Resolution,
    Vsync,
    FrameCap,
    Detail,
    Textures,
    Shadows,
    AntiAlias,
    Fog,
    View,
    Restart,
    Hosting,
    HostPort,
    JoinIp,
    JoinPort,
    Join,
    Leave,
    Record,
    Close,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Choice,
    Text,
    Button,
}

struct Row {
    id: Id,
    label: String,
    value: String,
    kind: Kind,
}

/// The most rows a menu shows.
const ROWS: usize = 14;

/// The menu's panel, and its rows (by index).
#[derive(Component)]
pub struct Panel;
#[derive(Component)]
pub struct RowNode(usize);
/// The panel's texts: its title, a row's label and value, the status line.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    Title,
    Label(usize),
    Value(usize),
    Status,
}

const SELECTED: Color = Color::srgba(0.85, 0.65, 0.25, 0.35);
const HOVERED: Color = Color::srgba(1.0, 1.0, 1.0, 0.08);

/// The panel (hidden), made once at start. (`AC1_MENU=graphics|online` opens one from the start, for screenshots.)
pub fn spawn(mut commands: Commands, mut menu: ResMut<Menu>) {
    menu.open = match std::env::var("AC1_MENU").as_deref() {
        Ok("graphics") => Some(Which::Graphics),
        Ok("online") => Some(Which::Online),
        _ => None,
    };
    let font = |px: f32| TextFont { font_size: bevy::text::FontSize::Px(px), ..default() };
    commands
        .spawn((
            Panel,
            Node {
                position_type: PositionType::Absolute,
                left: percent(28),
                top: percent(19),
                width: px(620),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(px(16)),
                row_gap: px(2),
                ..default()
            },
            BackgroundColor(Color::srgba(0.03, 0.03, 0.04, 0.9)),
            Visibility::Hidden,
            GlobalZIndex(11),
        ))
        .with_children(|p| {
            p.spawn((Part::Title, Text::new(""), font(24.0), Node { margin: UiRect::bottom(px(10)), ..default() }));
            for i in 0..ROWS {
                p.spawn((
                    RowNode(i),
                    Button,
                    Node { padding: UiRect::axes(px(8), px(3)), justify_content: JustifyContent::SpaceBetween, column_gap: px(24), ..default() },
                    BackgroundColor(Color::NONE),
                ))
                .with_children(|r| {
                    r.spawn((Part::Label(i), Text::new(""), font(19.0)));
                    r.spawn((Part::Value(i), Text::new(""), font(19.0), TextColor(Color::srgb(1.0, 0.85, 0.55))));
                });
            }
            p.spawn((Part::Status, Text::new(""), font(16.0), TextColor(Color::srgb(0.8, 0.85, 0.9)), Node { margin: UiRect::top(px(10)), ..default() }));
        });
}

fn on_off(b: bool) -> String {
    if b { "On".into() } else { "Off".into() }
}

/// The city detail and texture size the game loaded with differ from the settings: a restart would change them.
fn restart_needed(s: &Settings) -> Option<String> {
    let (was, now) = (crate::settings::started().city_detail(), s.city_detail());
    let mut what = vec![];
    if was.max_texture != now.max_texture {
        what.push("texture size");
    }
    if was.full != now.full {
        what.push("detail full");
    }
    (!what.is_empty()).then(|| what.join(", "))
}

fn rows(which: Which, s: &Settings, net: Option<&crate::net::Net>, recording: bool, edit: &Option<(Id, String)>) -> Vec<Row> {
    let row = |id, label: &str, value: String, kind| Row { id, label: label.into(), value, kind };
    // (A field being typed into shows the text so far, with a caret.)
    let text = |id: Id, v: String| match edit {
        Some((e, t)) if *e == id => format!("[{t}_]"),
        _ => format!("[{v}]"),
    };
    let mut out = vec![];
    match which {
        Which::Graphics => {
            out.push(row(Id::Preset, "Preset", word(s.preset()), Kind::Choice));
            let mode = match s.window {
                WindowMode::Windowed => "Windowed",
                WindowMode::Borderless => "Borderless fullscreen",
                WindowMode::Fullscreen => "Fullscreen",
            };
            out.push(row(Id::Window, "Window", mode.into(), Kind::Choice));
            let windowed_only = if s.window == WindowMode::Windowed { "" } else { "  (windowed)" };
            out.push(row(Id::Resolution, "Resolution", format!("{} x {}{windowed_only}", s.resolution.0, s.resolution.1), Kind::Choice));
            out.push(row(Id::Vsync, "VSync", on_off(s.vsync), Kind::Choice));
            out.push(row(Id::FrameCap, "Frame cap", if s.frame_cap == 0 { "Off".into() } else { format!("{} fps", s.frame_cap) }, Kind::Choice));
            out.push(row(Id::Detail, "City detail (draw distance, LOD)", word(s.detail), Kind::Choice));
            out.push(row(Id::Textures, "Texture size (restart)", format!("{} px", s.textures), Kind::Choice));
            out.push(row(Id::Shadows, "Shadows", word(s.shadows), Kind::Choice));
            let aa = match s.aa {
                AntiAlias::Off => "Off",
                AntiAlias::Fxaa => "FXAA",
                AntiAlias::Msaa2 => "MSAA 2x",
                AntiAlias::Msaa4 => "MSAA 4x",
            };
            out.push(row(Id::AntiAlias, "Anti-aliasing", aa.into(), Kind::Choice));
            out.push(row(Id::Fog, "Fog", on_off(s.fog), Kind::Choice));
            out.push(row(Id::View, "View distance", format!("{} m", s.view), Kind::Choice));
            if let Some(what) = restart_needed(s) {
                out.push(row(Id::Restart, &format!("Restart now (for {what})"), String::new(), Kind::Button));
            }
        }
        Which::Online => {
            let hosting = net.is_some_and(|n| n.hosting());
            out.push(row(Id::Hosting, "Hosting", on_off(hosting), Kind::Choice));
            if hosting {
                let port = net.map_or(s.host_port, |n| n.port());
                out.push(row(Id::HostPort, "Hosting port", text(Id::HostPort, port.to_string()), Kind::Text));
            } else {
                out.push(row(Id::JoinIp, "Join IP", text(Id::JoinIp, s.join_ip.clone()), Kind::Text));
                out.push(row(Id::JoinPort, "Join port", text(Id::JoinPort, s.join_port.to_string()), Kind::Text));
                out.push(row(Id::Join, "Join game", String::new(), Kind::Button));
                if net.is_some() {
                    out.push(row(Id::Leave, "Leave", String::new(), Kind::Button));
                }
            }
            out.push(row(Id::Record, "Record replay", on_off(recording), Kind::Choice));
        }
    }
    out.push(row(Id::Close, "Close", String::new(), Kind::Button));
    out
}

/// Step through `list` from `now` (`dir` +1 or -1), wrapping; a value not in it starts from the first.
fn step<T: PartialEq + Copy>(list: &[T], now: T, dir: i32) -> T {
    let i = list.iter().position(|v| *v == now).map_or(0, |i| (i as i32 + dir).rem_euclid(list.len() as i32) as usize);
    list[i]
}

/// Change a choice row (`dir` +1 or -1) in the settings.
fn change(id: Id, s: &mut Settings, dir: i32) {
    match id {
        Id::Preset => {
            let p = step(&Preset::ALL, s.preset(), dir);
            s.set_preset(p);
        }
        Id::Window => s.window = step(&[WindowMode::Windowed, WindowMode::Borderless, WindowMode::Fullscreen], s.window, dir),
        Id::Resolution => s.resolution = step(&RESOLUTIONS, s.resolution, dir),
        Id::Vsync => s.vsync ^= true,
        Id::FrameCap => s.frame_cap = step(&FRAME_CAPS, s.frame_cap, dir),
        Id::Detail => s.detail = step(&[DetailLevel::Low, DetailLevel::Medium, DetailLevel::High, DetailLevel::Ultra, DetailLevel::Full], s.detail, dir),
        Id::Textures => s.textures = step(&TEXTURES, s.textures, dir),
        Id::Shadows => s.shadows = step(&[Shadows::Off, Shadows::Low, Shadows::Medium, Shadows::High], s.shadows, dir),
        Id::AntiAlias => s.aa = step(&[AntiAlias::Off, AntiAlias::Fxaa, AntiAlias::Msaa2, AntiAlias::Msaa4], s.aa, dir),
        Id::Fog => s.fog ^= true,
        Id::View => s.view = step(&VIEWS, s.view, dir),
        _ => {}
    }
}

/// What may be typed into a field, and how long it may get.
fn accepts(id: Id, c: char, len: usize) -> bool {
    match id {
        Id::HostPort | Id::JoinPort => c.is_ascii_digit() && len < 5,
        _ => (c.is_ascii_alphanumeric() || ".-:".contains(c)) && len < 64,
    }
}

/// Keys typed into a field's `text` (characters it takes, Backspace); Enter ends it (`Some(true)`: keep), Esc too
/// (`Some(false)`: drop). Keys after the end are left.
fn type_into(id: Id, text: &mut String, typed: &[KeyboardInput]) -> Option<bool> {
    for k in typed {
        match &k.logical_key {
            Key::Enter => return Some(true),
            Key::Escape => return Some(false),
            Key::Backspace => {
                text.pop();
            }
            _ => {
                for c in k.text.as_deref().unwrap_or("").chars() {
                    if accepts(id, c, text.len()) {
                        text.push(c);
                    }
                }
            }
        }
    }
    None
}

/// A port as typed.
fn port(t: &str) -> Result<u16, String> {
    t.trim().parse::<u16>().ok().filter(|p| *p > 0).ok_or_else(|| format!("{t:?} is no port (1-65535)"))
}

/// The menus' keys and clicks (`PreUpdate`, after the UI has seen the mouse): open, close, pick, change, type; then
/// the game is given no input while a menu is open.
#[allow(clippy::too_many_arguments)]
pub fn input(
    mut commands: Commands,
    mut menu: ResMut<Menu>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut scroll: ResMut<AccumulatedMouseScroll>,
    motion: Res<AccumulatedMouseMotion>,
    mut typed: MessageReader<KeyboardInput>,
    mut settings: ResMut<Settings>,
    net: Option<Res<crate::net::Net>>,
    recorder: Option<ResMut<crate::replay::Recorder>>,
    rows_ui: Query<(&RowNode, &Interaction)>,
    mut cursor: Query<&mut CursorOptions, With<PrimaryWindow>>,
    mut overlay: ResMut<crate::Overlay>,
) {
    let typed: Vec<KeyboardInput> = typed.read().filter(|k| k.state == ButtonState::Pressed).cloned().collect();
    let was_open = menu.open.is_some();
    // F8 / F10 open their menu (or close it); Esc closes (or drops the text being typed).
    for (key, which) in [(KeyCode::F8, Which::Graphics), (KeyCode::F10, Which::Online)] {
        if keys.just_pressed(key) {
            menu.open = if menu.open == Some(which) { None } else { Some(which) };
            menu.row = 0;
            menu.edit = None;
            menu.note.clear();
        }
    }
    let Some(which) = menu.open else {
        if was_open {
            close(&mut menu, &mut cursor);
            consume(&mut keys, &mut mouse, &mut scroll);
        }
        return;
    };
    if !was_open {
        // (Opening: the cursor is freed for the mouse, the map menu shut.)
        overlay.menu = false;
        if let Ok(mut c) = cursor.single_mut() {
            menu.held = c.grab_mode != CursorGrabMode::None;
            c.grab_mode = CursorGrabMode::None;
            c.visible = true;
        }
    }
    let before = settings.clone();
    let recording = recorder.as_ref().is_some_and(|r| r.on());
    let list = rows(which, &settings, net.as_deref(), recording, &menu.edit);
    menu.row = menu.row.min(list.len() - 1);
    // What is asked of a row this frame: a change (+1 / -1) or a press.
    let mut act: Option<(usize, i32)> = None;
    if let Some((id, mut text)) = menu.edit.take() {
        // Typing into a field.
        match type_into(id, &mut text, &typed) {
            Some(true) => {
                let r = match id {
                    Id::HostPort => port(&text).map(|p| settings.host_port = p),
                    Id::JoinPort => port(&text).map(|p| settings.join_port = p),
                    _ if text.trim().is_empty() => Err("an IP address or a name, as 192.168.1.20".into()),
                    _ => {
                        settings.join_ip = text.trim().to_string();
                        Ok(())
                    }
                };
                match r {
                    Err(e) => menu.note = e,
                    // (A new hosting port while hosting: listen on it now.)
                    Ok(()) if id == Id::HostPort && net.as_ref().is_some_and(|n| n.hosting() && n.port() != settings.host_port) => {
                        start(&mut commands, &mut menu, crate::net::host(settings.host_port))
                    }
                    Ok(()) => {}
                }
            }
            Some(false) => {}
            None => menu.edit = Some((id, text)),
        }
    } else if keys.just_pressed(KeyCode::Escape) {
        menu.open = None;
    } else {
        if keys.just_pressed(KeyCode::ArrowDown) || keys.just_pressed(KeyCode::Tab) {
            menu.row = (menu.row + 1) % list.len();
        }
        if keys.just_pressed(KeyCode::ArrowUp) {
            menu.row = (menu.row + list.len() - 1) % list.len();
        }
        if keys.just_pressed(KeyCode::ArrowRight) {
            act = Some((menu.row, 1));
        }
        if keys.just_pressed(KeyCode::ArrowLeft) {
            act = Some((menu.row, -1));
        }
        if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter) {
            act = Some((menu.row, 1));
        }
        // The mouse: the row under it is picked as it moves; a click (or the wheel) changes it.
        for (r, i) in &rows_ui {
            if r.0 < list.len() && *i != Interaction::None {
                if motion.delta != Vec2::ZERO {
                    menu.row = r.0;
                }
                if mouse.just_pressed(MouseButton::Left) {
                    act = Some((r.0, 1));
                } else if mouse.just_pressed(MouseButton::Right) {
                    act = Some((r.0, -1));
                } else if scroll.delta.y != 0.0 && list[r.0].kind == Kind::Choice {
                    act = Some((r.0, if scroll.delta.y > 0.0 { 1 } else { -1 }));
                }
            }
        }
    }
    if let Some((i, dir)) = act {
        menu.row = i;
        let r = &list[i];
        match (r.kind, r.id) {
            (Kind::Choice, Id::Hosting) => {
                if net.as_ref().is_some_and(|n| n.hosting()) {
                    commands.remove_resource::<crate::net::Net>();
                    menu.note = "hosting stopped".into();
                } else {
                    start(&mut commands, &mut menu, crate::net::host(settings.host_port));
                }
            }
            (Kind::Choice, Id::Record) => match recorder {
                Some(mut r) if r.on() => r.stop(),
                Some(mut r) => r.start(),
                None => commands.insert_resource(crate::replay::Recorder::new(std::env::var("AC1_LEVEL").unwrap_or_default())),
            },
            (Kind::Choice, id) => change(id, &mut settings, dir),
            (Kind::Text, id) => {
                let now = match id {
                    Id::HostPort => net.as_ref().map_or(settings.host_port, |n| n.port()).to_string(),
                    Id::JoinPort => settings.join_port.to_string(),
                    _ => settings.join_ip.clone(),
                };
                // (Opened with Enter: what was typed after it in the same frame goes in.)
                let mut now = now;
                let after = typed.iter().position(|k| k.logical_key == Key::Enter).map_or(&[][..], |i| &typed[i + 1..]);
                if type_into(id, &mut now, after).is_none() {
                    menu.edit = Some((id, now));
                }
            }
            (_, Id::Join) => {
                let to = format!("{}:{}", settings.join_ip, settings.join_port);
                start(&mut commands, &mut menu, crate::net::join(&to));
            }
            (_, Id::Leave) => {
                commands.remove_resource::<crate::net::Net>();
                menu.note = "left the game".into();
            }
            (_, Id::Restart) => menu.restart = true,
            (_, Id::Close) => menu.open = None,
            _ => {}
        }
    }
    if *settings != before && Settings::kept() {
        match settings.save() {
            Ok(p) => menu.note = format!("saved ({})", p.file_name().map_or(p.display().to_string(), |n| n.to_string_lossy().into())),
            Err(e) => menu.note = e,
        }
    }
    if menu.open.is_none() {
        close(&mut menu, &mut cursor);
    }
    consume(&mut keys, &mut mouse, &mut scroll);
}

/// Start a session (replacing any), or tell why not.
fn start(commands: &mut Commands, menu: &mut Menu, n: Result<crate::net::Net, String>) {
    match n {
        Ok(n) => {
            info!("net: {}", n.status());
            menu.note.clear();
            commands.insert_resource(n);
        }
        Err(e) => {
            warn!("net: {e}");
            menu.note = e;
        }
    }
}

fn close(menu: &mut Menu, cursor: &mut Query<&mut CursorOptions, With<PrimaryWindow>>) {
    menu.edit = None;
    if std::mem::take(&mut menu.held)
        && let Ok(mut c) = cursor.single_mut()
    {
        c.grab_mode = CursorGrabMode::Locked;
        c.visible = false;
    }
}

/// The game gets no keys, buttons or wheel this frame.
fn consume(keys: &mut ButtonInput<KeyCode>, mouse: &mut ButtonInput<MouseButton>, scroll: &mut AccumulatedMouseScroll) {
    keys.reset_all();
    mouse.reset_all();
    scroll.delta = Vec2::ZERO;
}

/// Draw the open menu: its title, rows (the picked one lit) and status line.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    menu: Res<Menu>,
    settings: Res<Settings>,
    net: Option<Res<crate::net::Net>>,
    recorder: Option<Res<crate::replay::Recorder>>,
    mut panel: Query<&mut Visibility, With<Panel>>,
    mut nodes: Query<(&RowNode, &mut Node, &mut BackgroundColor, &Interaction)>,
    mut texts: Query<(&Part, &mut Text)>,
) {
    let Ok(mut vis) = panel.single_mut() else { return };
    let Some(which) = menu.open else {
        vis.set_if_neq(Visibility::Hidden);
        return;
    };
    vis.set_if_neq(Visibility::Visible);
    let list = rows(which, &settings, net.as_deref(), recorder.as_ref().is_some_and(|r| r.on()), &menu.edit);
    let (name, key) = match which {
        Which::Graphics => ("Graphics", "F8"),
        Which::Online => ("Online", "F10"),
    };
    for (r, mut node, mut bg, i) in &mut nodes {
        let display = if r.0 < list.len() { Display::Flex } else { Display::None };
        if node.display != display {
            node.display = display;
        }
        let c = if r.0 == menu.row {
            SELECTED
        } else if *i != Interaction::None {
            HOVERED
        } else {
            Color::NONE
        };
        if bg.0 != c {
            bg.0 = c;
        }
    }
    for (part, mut t) in &mut texts {
        let s = match *part {
            Part::Title => format!("{name}  ({key} or Esc closes)"),
            Part::Label(i) => match list.get(i) {
                Some(row) if row.kind == Kind::Button => format!("[ {} ]", row.label),
                Some(row) => row.label.clone(),
                None => continue,
            },
            Part::Value(i) => match list.get(i) {
                Some(row) if row.kind == Kind::Choice => format!("<  {}  >", row.value),
                Some(row) => row.value.clone(),
                None => continue,
            },
            Part::Status => {
                let mut s = match which {
                    Which::Graphics => {
                        "Up/Down pick, Left/Right or click change, Enter press. Low suits slow PCs;\ntexture size takes a restart, the rest is live."
                            .to_string()
                    }
                    Which::Online => format!(
                        "Up/Down pick, Enter or click to type, Enter keeps, Esc drops.\n{}",
                        net.as_ref().map_or("offline (0 peers)".to_string(), |n| n.status())
                    ),
                };
                if !menu.note.is_empty() {
                    s += &format!("\n{}", menu.note);
                }
                s
            }
        };
        if t.0 != s {
            t.0 = s;
        }
    }
}

/// The graphics menu's Restart: the game starts again where he stands, in this level, with the saved settings.
pub fn restart(mut menu: ResMut<Menu>, player: Query<&Transform, With<crate::character::Player>>, mut exit: MessageWriter<AppExit>) {
    if !std::mem::take(&mut menu.restart) {
        return;
    }
    let Ok(exe) = std::env::current_exe() else { return };
    let mut c = std::process::Command::new(exe);
    c.args(std::env::args_os().skip(1)).env_remove("AC1_FREECAM");
    if let Ok(tf) = player.single() {
        let p = tf.translation;
        let yaw = tf.rotation.to_euler(EulerRot::YXZ).0.to_degrees();
        c.env("AC1_START", format!("{:.2},{:.2},{yaw:.1},{:.2}", p.x, p.z, p.y + 0.5));
    }
    match c.spawn() {
        Ok(_) => {
            info!("graphics menu: starting again");
            exit.write(AppExit::Success);
        }
        Err(e) => menu.note = format!("could not start the game again: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choices_step_round_and_presets_follow() {
        let mut s = Settings::default();
        change(Id::Preset, &mut s, 1);
        assert_eq!(s.preset(), Preset::High);
        change(Id::Preset, &mut s, -1);
        change(Id::Preset, &mut s, -1);
        assert_eq!(s.preset(), Preset::Low);
        change(Id::Preset, &mut s, -1);
        assert_eq!(s.preset(), Preset::Ultra);
        change(Id::Shadows, &mut s, -1);
        assert_eq!(s.preset(), Preset::Custom);
        assert_eq!(step(&VIEWS, 777, 1), VIEWS[0]);
    }

    #[test]
    fn fields_take_what_fits() {
        assert!(accepts(Id::HostPort, '7', 4) && !accepts(Id::HostPort, '7', 5) && !accepts(Id::HostPort, '.', 0));
        assert!(accepts(Id::JoinIp, '.', 3) && accepts(Id::JoinIp, 'a', 0) && !accepts(Id::JoinIp, ' ', 0));
        assert_eq!(port("7777"), Ok(7777));
        assert!(port("0").is_err() && port("70000").is_err() && port("").is_err());
    }
}
