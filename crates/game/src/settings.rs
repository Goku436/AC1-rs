//! The player's settings (the graphics menu, F8, and the online menu's addresses, F10), kept in `ac1-settings.txt`
//! next to the executable (`key = value` lines) and read at start.
//!
//! The environment still wins for test runs: `AC1_DETAIL` over the city detail and texture size, `AC1_WINDOW_SIZE`
//! over the resolution; `AC1_BACKGROUND` keeps a window. Scripted and headless runs (`AC1_HEADLESS`, `AC1_SHOT`,
//! `AC1_FRAMES`, `AC1_CONTROL`) and `AC1_NO_SETTINGS=1` do not read the file (`AC1_SETTINGS=<file>` reads that one, in
//! any run): they run on the defaults, which are the
//! game as it was before the menu (medium detail, 512 textures, AC1's shadows as set up, MSAA 4x, fog, 5 km).
//!
//! Live: window mode, resolution, vsync, frame cap, shadows, anti-aliasing, fog, view distance, and the city detail's
//! draw distance, LOD and vertex budget. On a restart (the menu's "Restart" row starts the game again where he
//! stands): the texture size (city textures are cut down as they load) and the detail `full` (no streaming).

use bevy::prelude::*;
use std::path::PathBuf;

/// The settings file's name (beside the executable).
pub const FILE: &str = "ac1-settings.txt";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowMode {
    Windowed,
    Borderless,
    Fullscreen,
}

/// City detail (`stream::Detail`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DetailLevel {
    Low,
    Medium,
    High,
    Ultra,
    Full,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shadows {
    Off,
    Low,
    Medium,
    High,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AntiAlias {
    Off,
    Fxaa,
    Msaa2,
    Msaa4,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    Low,
    Medium,
    High,
    Ultra,
    Custom,
}

#[derive(Resource, Clone, Debug, PartialEq)]
pub struct Settings {
    pub window: WindowMode,
    /// Windowed size (logical pixels).
    pub resolution: (u32, u32),
    pub vsync: bool,
    /// Frames a second at most (0: no cap).
    pub frame_cap: u32,
    pub detail: DetailLevel,
    /// The largest city texture side kept.
    pub textures: u32,
    pub shadows: Shadows,
    pub aa: AntiAlias,
    pub fog: bool,
    /// The camera's far plane (m); the fog closes in before it.
    pub view: u32,
    /// Online (F10): the port to host on, and the last address joined.
    pub host_port: u16,
    pub join_ip: String,
    pub join_port: u16,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            window: WindowMode::Windowed,
            resolution: (1280, 720),
            vsync: true,
            frame_cap: 0,
            detail: DetailLevel::Medium,
            textures: 512,
            shadows: Shadows::Medium,
            aa: AntiAlias::Msaa4,
            fog: true,
            view: 5000,
            host_port: 7777,
            join_ip: "127.0.0.1".into(),
            join_port: 7777,
        }
    }
}

pub const RESOLUTIONS: [(u32, u32); 6] = [(1280, 720), (1366, 768), (1600, 900), (1920, 1080), (2560, 1440), (3840, 2160)];
pub const FRAME_CAPS: [u32; 5] = [0, 30, 60, 120, 144];
pub const TEXTURES: [u32; 4] = [256, 512, 1024, 2048];
pub const VIEWS: [u32; 4] = [600, 1200, 2500, 5000];

impl Preset {
    pub const ALL: [Preset; 4] = [Preset::Low, Preset::Medium, Preset::High, Preset::Ultra];

    /// The quality settings a preset sets: (detail, textures, shadows, anti-aliasing, view distance). Medium is the
    /// game as it was before the menu.
    fn values(self) -> Option<(DetailLevel, u32, Shadows, AntiAlias, u32)> {
        Some(match self {
            Preset::Low => (DetailLevel::Low, 256, Shadows::Off, AntiAlias::Off, 1200),
            Preset::Medium => (DetailLevel::Medium, 512, Shadows::Medium, AntiAlias::Msaa4, 5000),
            Preset::High => (DetailLevel::High, 1024, Shadows::High, AntiAlias::Msaa4, 5000),
            Preset::Ultra => (DetailLevel::Ultra, 2048, Shadows::High, AntiAlias::Msaa4, 5000),
            Preset::Custom => return None,
        })
    }
}

impl Settings {
    /// The preset these settings match, or Custom.
    pub fn preset(&self) -> Preset {
        let now = (self.detail, self.textures, self.shadows, self.aa, self.view);
        Preset::ALL.into_iter().find(|p| p.values() == Some(now)).unwrap_or(Preset::Custom)
    }

    pub fn set_preset(&mut self, p: Preset) {
        if let Some((detail, textures, shadows, aa, view)) = p.values() {
            (self.detail, self.textures, self.shadows, self.aa, self.view) = (detail, textures, shadows, aa, view);
        }
    }

    /// The city detail to load with: `AC1_DETAIL` if set (its own texture size), else these settings.
    pub fn city_detail(&self) -> crate::stream::Detail {
        if let Some(d) = std::env::var("AC1_DETAIL").ok().and_then(|v| crate::stream::Detail::named(&v)) {
            return d;
        }
        crate::stream::Detail { max_texture: self.textures, ..self.detail.stream() }
    }

    /// As a settings file.
    pub fn to_text(&self) -> String {
        let mut s = String::from("# ac1-rs settings: F8 graphics, F10 online. Deleted, the defaults come back.\n");
        let mut put = |k: &str, v: String| s += &format!("{k} = {v}\n");
        put("window", word(self.window));
        put("resolution", format!("{}x{}", self.resolution.0, self.resolution.1));
        put("vsync", on_off(self.vsync).into());
        put("frame_cap", self.frame_cap.to_string());
        put("detail", word(self.detail));
        put("textures", self.textures.to_string());
        put("shadows", word(self.shadows));
        put("anti_alias", word(self.aa));
        put("fog", on_off(self.fog).into());
        put("view_distance", self.view.to_string());
        put("host_port", self.host_port.to_string());
        put("join_ip", self.join_ip.clone());
        put("join_port", self.join_port.to_string());
        s
    }

    /// From a settings file: lines it does not know, or values that do not read, keep the defaults.
    pub fn from_text(text: &str) -> Self {
        let mut s = Self::default();
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with('#') {
                continue;
            }
            let Some((k, v)) = line.split_once('=') else { continue };
            let (k, v) = (k.trim(), v.trim());
            let num = |v: &str| v.parse::<u32>().ok();
            match k {
                "window" => s.window = from_word(v, &[WindowMode::Windowed, WindowMode::Borderless, WindowMode::Fullscreen]).unwrap_or(s.window),
                "resolution" => {
                    if let Some((w, h)) = v.split_once('x').and_then(|(w, h)| Some((num(w)?, num(h)?))).filter(|(w, h)| *w >= 320 && *h >= 200) {
                        s.resolution = (w, h);
                    }
                }
                "vsync" => s.vsync = v != "off",
                "frame_cap" => s.frame_cap = num(v).unwrap_or(s.frame_cap),
                "detail" => {
                    s.detail =
                        from_word(v, &[DetailLevel::Low, DetailLevel::Medium, DetailLevel::High, DetailLevel::Ultra, DetailLevel::Full]).unwrap_or(s.detail)
                }
                "textures" => s.textures = num(v).filter(|t| TEXTURES.contains(t)).unwrap_or(s.textures),
                "shadows" => s.shadows = from_word(v, &[Shadows::Off, Shadows::Low, Shadows::Medium, Shadows::High]).unwrap_or(s.shadows),
                "anti_alias" => s.aa = from_word(v, &[AntiAlias::Off, AntiAlias::Fxaa, AntiAlias::Msaa2, AntiAlias::Msaa4]).unwrap_or(s.aa),
                "fog" => s.fog = v != "off",
                "view_distance" => s.view = num(v).filter(|d| (100..=20_000).contains(d)).unwrap_or(s.view),
                "host_port" => s.host_port = v.parse().ok().filter(|p| *p > 0).unwrap_or(s.host_port),
                "join_ip" if !v.is_empty() => s.join_ip = v.to_string(),
                "join_port" => s.join_port = v.parse().ok().filter(|p| *p > 0).unwrap_or(s.join_port),
                _ => {}
            }
        }
        s
    }

    /// Where the file is: beside the executable, or `AC1_SETTINGS=<file>`.
    pub fn path() -> Option<PathBuf> {
        if let Ok(p) = std::env::var("AC1_SETTINGS") {
            return Some(p.into());
        }
        Some(std::env::current_exe().ok()?.parent()?.join(FILE))
    }

    /// Does this run read and write the file (not a scripted or headless test run, unless `AC1_SETTINGS` names one)?
    pub fn kept() -> bool {
        if std::env::var("AC1_SETTINGS").is_ok() {
            return true;
        }
        ["AC1_NO_SETTINGS", "AC1_HEADLESS", "AC1_SHOT", "AC1_FRAMES", "AC1_CONTROL"].iter().all(|k| std::env::var(k).is_err())
    }

    /// The settings to start with: the file's, or the defaults.
    pub fn load() -> Self {
        if !Self::kept() {
            return Self::default();
        }
        Self::path().and_then(|p| std::fs::read_to_string(p).ok()).map_or_else(Self::default, |t| Self::from_text(&t))
    }

    pub fn save(&self) -> Result<PathBuf, String> {
        let p = Self::path().ok_or("no folder for the settings file")?;
        std::fs::write(&p, self.to_text()).map_err(|e| format!("cannot write {}: {e}", p.display()))?;
        Ok(p)
    }
}

/// The settings the game started with (the city reads its detail from them while it loads).
static STARTED: std::sync::OnceLock<Settings> = std::sync::OnceLock::new();

/// Load the settings once, at start.
pub fn started() -> &'static Settings {
    STARTED.get_or_init(Settings::load)
}

impl DetailLevel {
    pub fn stream(self) -> crate::stream::Detail {
        use crate::stream::Detail;
        match self {
            DetailLevel::Low => Detail::LOW,
            DetailLevel::Medium => Detail::MEDIUM,
            DetailLevel::High => Detail::HIGH,
            DetailLevel::Ultra => Detail::ULTRA,
            DetailLevel::Full => Detail::FULL,
        }
    }
}

fn on_off(b: bool) -> &'static str {
    if b { "on" } else { "off" }
}

/// An enum value as a lower-case word (its `Debug` name).
pub fn word<T: std::fmt::Debug>(v: T) -> String {
    format!("{v:?}").to_lowercase()
}

fn from_word<T: std::fmt::Debug + Copy>(w: &str, all: &[T]) -> Option<T> {
    all.iter().copied().find(|v| word(*v) == w.to_lowercase())
}

/// The window the settings ask for (before the env overrides `game_window` applies).
pub fn window_mode(m: WindowMode) -> bevy::window::WindowMode {
    use bevy::window::{MonitorSelection, VideoModeSelection};
    match m {
        WindowMode::Windowed => bevy::window::WindowMode::Windowed,
        WindowMode::Borderless => bevy::window::WindowMode::BorderlessFullscreen(MonitorSelection::Current),
        WindowMode::Fullscreen => bevy::window::WindowMode::Fullscreen(MonitorSelection::Current, VideoModeSelection::Current),
    }
}

/// Vsync on is Bevy's default (`Fifo`, as before the menu); off, no wait where the GPU allows it.
pub fn present_mode(vsync: bool) -> bevy::window::PresentMode {
    if vsync { bevy::window::PresentMode::Fifo } else { bevy::window::PresentMode::AutoNoVsync }
}

/// What a scene had before the settings touched it: the fog and each sun's cascades and the camera's shadow filter
/// (medium shadows and fog on give them back as made).
#[derive(Default)]
pub struct Original {
    fog: Option<DistanceFog>,
    cascades: Vec<(Entity, bevy::light::CascadeShadowConfig)>,
    filter: Option<bevy::light::ShadowFilteringMethod>,
    taken: bool,
}

/// The camera as `apply` reads it: its far plane, the fog and the shadow filter it was made with.
type SceneCamera = (Entity, &'static mut Projection, Option<&'static DistanceFog>, Option<&'static bevy::light::ShadowFilteringMethod>);

/// Apply the settings when they change (and once at start): the window, the camera's anti-aliasing, far plane and
/// fog, the sun's shadows, the city's streaming detail.
#[allow(clippy::too_many_arguments)]
pub fn apply(
    mut commands: Commands,
    settings: Res<Settings>,
    mut original: Local<Original>,
    mut windows: Query<&mut Window, With<bevy::window::PrimaryWindow>>,
    mut cams: Query<SceneCamera, With<Camera3d>>,
    mut suns: Query<(Entity, &mut DirectionalLight, &bevy::light::CascadeShadowConfig)>,
    mut shadow_map: ResMut<bevy::light::DirectionalLightShadowMap>,
    city: Option<ResMut<crate::stream::CityRender>>,
    mut last: Local<Option<Settings>>,
) {
    let Ok((cam, mut proj, fog, filter)) = cams.single_mut() else { return };
    if !original.taken {
        original.taken = true;
        original.fog = fog.cloned();
        original.filter = filter.copied();
        original.cascades = suns.iter().map(|(e, _, c)| (e, c.clone())).collect();
    }
    if last.as_ref() == Some(&*settings) {
        return;
    }
    let s = &*settings;
    let first = last.is_none();
    // The window (at start it was made as asked: `game_window`).
    if !first && let Ok(mut w) = windows.single_mut() {
        let mode = window_mode(s.window);
        if w.mode != mode {
            w.mode = mode;
        }
        if s.window == WindowMode::Windowed && (w.resolution.width() as u32, w.resolution.height() as u32) != s.resolution {
            w.resolution.set(s.resolution.0 as f32, s.resolution.1 as f32);
        }
        let present = present_mode(s.vsync);
        if w.present_mode != present {
            w.present_mode = present;
        }
    }
    // Anti-aliasing: MSAA, or FXAA after the frame.
    let mut c = commands.entity(cam);
    match s.aa {
        AntiAlias::Off => c.insert(Msaa::Off).remove::<bevy::anti_alias::fxaa::Fxaa>(),
        AntiAlias::Fxaa => c.insert((Msaa::Off, bevy::anti_alias::fxaa::Fxaa::default())),
        AntiAlias::Msaa2 => c.insert(Msaa::Sample2).remove::<bevy::anti_alias::fxaa::Fxaa>(),
        AntiAlias::Msaa4 => c.insert(Msaa::Sample4).remove::<bevy::anti_alias::fxaa::Fxaa>(),
    };
    // The far plane, and the fog closing in before it.
    if let Projection::Perspective(p) = &mut *proj
        && p.far != s.view as f32
    {
        p.far = s.view as f32;
    }
    match (&original.fog, s.fog) {
        (Some(f), true) => {
            let mut f = f.clone();
            if let FogFalloff::Linear { start, end } = &mut f.falloff {
                let k = (s.view as f32 * 0.9 / *end).min(1.0);
                (*start, *end) = (*start * k, *end * k);
            }
            c.insert(f);
        }
        (Some(_), false) => {
            c.remove::<DistanceFog>();
        }
        _ => {}
    }
    // Shadows: off; low a single 1024 cascade near him with hardware filtering; medium as set up; high 4096 maps.
    c.insert(if s.shadows == Shadows::Low { bevy::light::ShadowFilteringMethod::Hardware2x2 } else { original.filter.unwrap_or_default() });
    let size = match s.shadows {
        Shadows::Low => 1024,
        Shadows::High => 4096,
        _ => 2048,
    };
    if shadow_map.size != size {
        shadow_map.size = size;
    }
    for (e, mut sun, _) in &mut suns {
        let on = s.shadows != Shadows::Off;
        if sun.shadow_maps_enabled != on {
            sun.shadow_maps_enabled = on;
        }
        let made = original.cascades.iter().find(|(o, _)| *o == e).map(|(_, c)| c.clone());
        if s.shadows == Shadows::Low {
            commands.entity(e).insert(
                bevy::light::CascadeShadowConfigBuilder { num_cascades: 1, first_cascade_far_bound: 40.0, maximum_distance: 40.0, ..default() }.build(),
            );
        } else if let Some(c) = made {
            commands.entity(e).insert(c);
        }
    }
    // The city's streaming: its draw distance, LOD and budget now (its textures and `full` on a restart).
    if let Some(mut city) = city {
        city.set_detail(s.city_detail());
    }
    if !first {
        info!("settings: {:?} preset, applied", s.preset());
    }
    *last = Some(s.clone());
}

/// The frame cap: wait out the rest of the frame (`Last`).
pub fn cap_frames(settings: Res<Settings>, mut last: Local<Option<std::time::Instant>>) {
    let now = std::time::Instant::now();
    if settings.frame_cap > 0
        && let Some(prev) = *last
    {
        let frame = std::time::Duration::from_secs_f64(1.0 / settings.frame_cap as f64);
        if let Some(rest) = frame.checked_sub(now - prev) {
            std::thread::sleep(rest);
        }
    }
    *last = Some(std::time::Instant::now());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_settings_file_reads_back_as_written() {
        let mut s = Settings { window: WindowMode::Borderless, resolution: (1920, 1080), vsync: false, frame_cap: 144, ..Settings::default() };
        s.set_preset(Preset::Low);
        s.fog = false;
        s.join_ip = "192.168.1.20".into();
        s.host_port = 40_000;
        assert_eq!(Settings::from_text(&s.to_text()), s);
        assert_eq!(s.preset(), Preset::Low);
    }

    #[test]
    fn the_defaults_are_the_medium_preset_and_bad_lines_keep_them() {
        let d = Settings::default();
        assert_eq!(d.preset(), Preset::Medium);
        assert_eq!(Settings::from_text("window = sideways\nresolution = 5x5\ntextures = 300\nnonsense\n# a comment\nhost_port = 0"), d);
        let mut s = d.clone();
        s.textures = 1024;
        assert_eq!(s.preset(), Preset::Custom);
    }
}
