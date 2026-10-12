//! The game's sounds, as AC1 plays them from its data (`forge::sound`).
//!
//! Altaïr's: the event keys of the clips playing (`animation::Playing`) start sound events. A clip's `AudioEvent` names
//! its sound (grunts, cloth, hands on stone, the haystack's straw, the leap of faith's rush of air); a `ContactEvent` (a
//! step, a landing, a roll) goes through Altaïr's contact table (`ContactTable_AlTair`) to the surface's sounds. The
//! events' switches are set as AC1's switch callback sets them (the exe's object type callback, found in Ghidra; see
//! docs/NOTES.md "Sound"): 0 the surface's sound material under him (`Level::sound_material_at`), 1 the character kind
//! (0: the player as Altaïr; the default case is a townsman's), 3 whether his weapon is drawn (0: sheathed, so the
//! scabbard and gear rattle under the steps), 7 the city (AC1's list of world ids). The rest play their default case.
//! The free jump's take-off has no key in AC1's data: it plays the `GlobalSoundSet`'s `SoundJumpStart`, the sound AC1's
//! code plays (the same event the beam take-off clip and the contact table's `ft_jump_start` name).
//!
//! In a city (`AC1_LEVEL`), the city's own sounds:
//! - Placed sounds (`forge::sound::Emitter`, 1292 in Damascus: fountains, braziers and lamps, flags, kiosks, gardens
//!   and other hiding places, awnings, palms and olive trees): each a trigger, its sounds starting as the player
//!   enters its zone (the loops until he leaves it, the rest once).
//! - Ambience: the loops of the ambience regions linked from the grid cells over the player (`ambience_sets`), faded in
//!   and out as he moves between them, and their sets' one-off sounds (gusts, birds, calls) at AC1's intervals and
//!   distances from the listener. The crowd murmur sets are left out (no crowds talk here).
//!
//! Every sound but the ambience loops is placed: it sounds from where it is (Altaïr's from his body), panned left and
//! right from the camera, the listener, and fading with distance by its event's roll-off (`forge::sound::RollOff`, AC1's
//! curve). The ambience loops are AC1's four-track streams folded to stereo, unplaced.
//!
//! The behaviour follows Banned445's AC1-Movement-Rewritten (MIT), which first played these sounds.
//!
//! M mutes and unmutes; `AC1_MUTE=1` starts muted (nothing is loaded until unmuted), `AC1_VOLUME=dB` turns every sound
//! up or down, and so does the graphics menu's (F8) volume row. Headless runs have no audio at all (`main` leaves the
//! plugin out).

use crate::animation::Playing;
use crate::character::{Character, Player};
use crate::level::Level;
use bevy::audio::{AddAudioSource, AudioSinkPlayback, ChannelCount, Decodable, PlaybackSettings, SampleRate, Source, Volume};
use bevy::prelude::*;
use forge::anim::{AnimEvent, CONTACT_HUMAN, EventKind};
use forge::sound::{Ambience, Bank, ContactTable, Emitter, Picker, RollOff, Voice};
use std::collections::HashMap;
use std::num::NonZero;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// A placed sound's left and right gains, set each frame from where it is against the listener (f32 bits).
#[derive(Default)]
pub struct Pan([AtomicU32; 2]);

impl Pan {
    fn set(&self, l: f32, r: f32) {
        self.0[0].store(l.to_bits(), Ordering::Relaxed);
        self.0[1].store(r.to_bits(), Ordering::Relaxed);
    }
    fn get(&self) -> (f32, f32) {
        (f32::from_bits(self.0[0].load(Ordering::Relaxed)), f32::from_bits(self.0[1].load(Ordering::Relaxed)))
    }
}

/// Decoded samples, interleaved. A placed sound plays them in stereo through its `pan` (a loop over and over, until
/// it is despawned).
#[derive(Asset, TypePath, Clone)]
pub struct PcmClip {
    channels: u16,
    rate: u32,
    samples: Arc<[i16]>,
    pan: Option<Arc<Pan>>,
    looped: bool,
}

pub struct PcmDecoder {
    clip: PcmClip,
    at: usize,
    /// The output channel next (placed sounds: 0 left, 1 right), and the gains of this frame.
    side: usize,
    gains: (f32, f32),
}

impl PcmDecoder {
    fn sample(&mut self) -> Option<f32> {
        if self.at >= self.clip.samples.len() {
            if !self.clip.looped || self.clip.samples.is_empty() {
                return None;
            }
            self.at = 0;
        }
        Some(self.clip.samples[self.at] as f32 / 32768.0)
    }
}

impl Iterator for PcmDecoder {
    type Item = bevy::audio::Sample;
    fn next(&mut self) -> Option<Self::Item> {
        let Some(pan) = self.clip.pan.clone() else {
            let s = self.sample()?;
            self.at += 1;
            return Some(s);
        };
        if self.side == 0 {
            self.gains = pan.get();
        }
        let s = self.sample()?;
        let out = if self.side == 0 { s * self.gains.0 } else { s * self.gains.1 };
        // Mono: the sample to both sides; stereo: left then right.
        if self.side == 1 || self.clip.channels > 1 {
            self.at += 1;
        }
        self.side ^= 1;
        Some(out)
    }
}

impl Source for PcmDecoder {
    fn current_span_len(&self) -> Option<usize> {
        if self.clip.looped {
            return None;
        }
        let left = self.clip.samples.len().saturating_sub(self.at);
        Some(if self.clip.pan.is_some() && self.clip.channels == 1 { left * 2 - self.side } else { left })
    }
    fn channels(&self) -> ChannelCount {
        let ch = if self.clip.pan.is_some() { 2 } else { self.clip.channels.max(1) };
        NonZero::new(ch).unwrap_or(NonZero::<u16>::MIN)
    }
    fn sample_rate(&self) -> SampleRate {
        NonZero::new(self.clip.rate.max(1)).unwrap_or(NonZero::<u32>::MIN)
    }
    fn total_duration(&self) -> Option<Duration> {
        if self.clip.looped {
            return None;
        }
        let frames = self.clip.samples.len() / self.clip.channels.max(1) as usize;
        Some(Duration::from_secs_f64(frames as f64 / self.clip.rate.max(1) as f64))
    }
}

impl Decodable for PcmClip {
    type Decoder = PcmDecoder;
    fn decoder(&self) -> PcmDecoder {
        PcmDecoder { clip: self.clone(), at: 0, side: 0, gains: (0.0, 0.0) }
    }
}

/// What the player's sounds are made from: the install's sound objects, Altaïr's contact table, and the sounds AC1's
/// code plays (`GlobalSoundSet`, by slot).
struct SoundData {
    bank: Bank,
    contacts: ContactTable,
    global: Vec<Option<u32>>,
}

/// The data files of `DataPC.forge` that hold the player's sounds.
const SOUND_FILES: [&str; 3] = ["Game Fix", "Game Bootstrap Settings", "Rank 9"];

fn load(game_dir: &Path) -> anyhow::Result<SoundData> {
    let forge = forge::Forge::open(game_dir.join("DataPC.forge"))?;
    let mut bank = Bank::default();
    let mut contacts = None;
    let mut global = vec![];
    for name in SOUND_FILES {
        let Some(e) = forge.entries.iter().find(|e| e.name == name) else { anyhow::bail!("no data file {name}") };
        let data = forge.read(e)?;
        let bad = bank.add_data_file(&data)?;
        if bad > 0 {
            warn!("sound: {bad} sound objects of {name} do not parse");
        }
        for o in forge::parse_objects(&data)? {
            if o.class == forge::sound::CLASS_CONTACT_TABLE && o.name == "ContactTable_AlTair" {
                contacts = Some(forge::sound::parse_contact_table(o.body)?);
            } else if o.class == forge::sound::CLASS_GLOBAL_SOUND_SET {
                global = forge::sound::global_sound_set(o.body);
            }
        }
    }
    let contacts = contacts.ok_or_else(|| anyhow::anyhow!("no ContactTable_AlTair"))?;
    Ok(SoundData { bank, contacts, global })
}

/// A city's ambience and placed sounds, and the sound objects they play.
struct CityData {
    bank: Bank,
    amb: Ambience,
}

fn load_city(game_dir: &Path, city: &str) -> anyhow::Result<CityData> {
    let forge = forge::Forge::open(game_dir.join(format!("DataPC_{city}.forge")))?;
    let mut bank = Bank::default();
    let amb = forge::sound::load_ambience(&forge, city, &mut bank)?;
    // (The placed sounds' samples are in the common forge, each in a data file of its own.)
    let common = forge::Forge::open(game_dir.join("DataPC_Common.forge"))?;
    forge::sound::load_resources(&common, &mut bank);
    Ok(CityData { bank, amb })
}

/// A streamed loop decoded to stereo.
fn load_stream(game_dir: &Path, stream: u32) -> anyhow::Result<Vec<i16>> {
    let forge = forge::Forge::open(game_dir.join("DataPC_StreamedSounds.forge"))?;
    let name = forge::sound::stream_file_name(stream);
    let Some(e) = forge.entries.iter().find(|e| e.name == name) else { anyhow::bail!("no streamed file {name}") };
    let data = forge.read(e)?;
    let (tracks, samples) = forge::sound::decode(forge::sound::parse_bao(&data)?.object)?;
    Ok(forge::sound::tracks_to_stereo(tracks, &samples))
}

/// Decoded samples: channels, rate, the samples interleaved.
type Samples = (u16, u32, Arc<[i16]>);

/// A result a thread fills in.
type Slot<T> = Arc<Mutex<Option<anyhow::Result<T>>>>;

fn spawn_load<T: Send + 'static>(f: impl FnOnce() -> anyhow::Result<T> + Send + 'static) -> Slot<T> {
    let slot: Slot<T> = Arc::new(Mutex::new(None));
    let out = slot.clone();
    std::thread::spawn(move || {
        let r = f();
        if let Ok(mut o) = out.lock() {
            *o = Some(r);
        }
    });
    slot
}

fn take<T>(slot: &Slot<T>) -> Option<anyhow::Result<T>> {
    slot.lock().ok().and_then(|mut o| o.take())
}

/// Where a slot's clip was last frame (see `passed`).
struct Cursor {
    clip: Arc<crate::animation::Clip>,
    t: f32,
    synced: bool,
}

/// Clips whose take-off AC1's data gives no key: they play `SoundJumpStart` as they begin (see the module header).
const TAKEOFFS: [&str; 2] = ["_to_jumpstraight_clear_", "xx_h_jumpstraight_clear_foot"];
/// The contact table's `ft_jump_start`: the take-off when the `GlobalSoundSet` is not read.
const FT_JUMP_START: u32 = 1;

/// An ambience loop fades in and out over this (s).
const AMBIENCE_FADE: f32 = 2.0;
/// How often (s) the player's ambience regions are looked up.
const AMBIENCE_CHECK: f32 = 0.5;
/// How often (s) the placed sounds' trigger zones are looked at.
const TRIGGER_CHECK: f32 = 0.2;
/// A placed loop fades in and out over this (s) as its zone is entered and left.
const PLACED_FADE: f32 = 0.5;
/// A zone is left this far (m) past its radius (no flicker on its rim).
const ZONE_SLACK: f32 = 0.5;
/// A placed sound with no zone read starts within this (m).
const ZONE_FALLBACK: f32 = 10.0;
/// Altaïr's sounds come from this high over his root (m).
const BODY_HEIGHT: f32 = 0.9;
/// How far a sound straight to one side leans to it (0: not at all, 1: wholly).
const PAN_WIDTH: f32 = 0.6;

/// AC1's switch 7: the world's index in the list its switch callback walks (Acre first, then the list at 0x16DA9D0);
/// worlds not in it get its length.
fn world_index(city: &str) -> u32 {
    const WORLDS: [&str; 10] =
        ["acre", "arsuf", "loadingroom2", "assassination_talal", "damascus", "jerusalem", "kingdom", "map_menu", "masyaf", "present_room"];
    WORLDS.iter().position(|w| w.eq_ignore_ascii_case(city)).unwrap_or(WORLDS.len()) as u32
}

/// The switches' values for the player (see the module header): `None` plays a switch's default case.
fn player_switch(var: u32, material: Option<u32>, city: Option<&str>) -> Option<u32> {
    match var {
        0 => material,
        // The character kind: the player as Altaïr (the entity's kind bits 1, sub-kind 0).
        1 => Some(0),
        // His weapon module's "drawn": never here.
        3 => Some(0),
        7 => city.map(world_index),
        _ => None,
    }
}

/// Where a placed sound sounds from.
#[derive(Clone, Copy, Debug)]
enum Where {
    Player,
    /// Bevy space.
    Point(Vec3),
}

/// A placed sound: where it is, its roll-off, its gain (linear), and (loops) how far faded in.
#[derive(Component)]
struct Placed {
    pan: Arc<Pan>,
    at: Where,
    rolloff: RollOff,
    gain: f32,
    level: f32,
    /// Fading in (`Some(false)`) or out to be despawned (`Some(true)`); `None`: a one-shot, at full level.
    fade: Option<bool>,
}

/// A playing ambience loop: its key (a stream, or a looped resource), its gain (dB) and how far faded in (0..1);
/// `leaving` fades it out.
#[derive(Component)]
struct AmbientLoop {
    key: u32,
    gain_db: f32,
    level: f32,
    leaving: bool,
}

/// A placed sound's trigger: inside its zone, and the loops it started.
#[derive(Default, Clone)]
struct Trigger {
    inside: bool,
    loops: Vec<Entity>,
}

/// The listener: the camera (its place and its right).
#[derive(Clone, Copy)]
struct Listener {
    pos: Vec3,
    right: Vec3,
}

impl Listener {
    /// Left and right gains of a sound at `p` (its own gain and roll-off in).
    fn gains(&self, p: Vec3, rolloff: &RollOff, gain: f32) -> (f32, f32) {
        let to = p - self.pos;
        let d = to.length();
        let amp = gain * rolloff.gain(d);
        let side = if d > 1e-3 { to.dot(self.right) / d * PAN_WIDTH } else { 0.0 };
        (amp * (1.0 - side).max(0.0).sqrt(), amp * (1.0 + side).max(0.0).sqrt())
    }
}

#[derive(Resource)]
pub struct Sounds {
    data: Option<Arc<SoundData>>,
    loading: Option<Slot<SoundData>>,
    picker: Picker,
    /// Decoded resources by BAO id (`None`: they do not decode).
    clips: HashMap<u32, Option<Samples>>,
    cursors: HashMap<u8, Cursor>,
    /// When the last take-off played (game time, s).
    last_takeoff: f32,
    /// The city (`AC1_LEVEL`), its sounds loading or loaded, its ambience loops (decoded, or decoding), the placed
    /// sounds' triggers, the one-off sounds' next times.
    city: Option<String>,
    city_data: Option<Arc<CityData>>,
    city_loading: Option<Slot<CityData>>,
    loops: HashMap<u32, Handle<PcmClip>>,
    decoding: HashMap<u32, (u32, Slot<Vec<i16>>)>,
    next_check: f32,
    next_trigger: f32,
    triggers: Vec<Trigger>,
    one_offs: HashMap<(u32, usize), f32>,
    rng: u32,
    pub muted: bool,
    /// Added to every sound's gain (`AC1_VOLUME`, dB).
    volume_db: f32,
}

impl Default for Sounds {
    fn default() -> Self {
        Sounds {
            data: None,
            loading: None,
            picker: Picker::new(0x2007_1113),
            clips: HashMap::new(),
            cursors: HashMap::new(),
            last_takeoff: f32::NEG_INFINITY,
            city: std::env::var("AC1_LEVEL").ok().filter(|c| !c.is_empty()),
            city_data: None,
            city_loading: None,
            loops: HashMap::new(),
            decoding: HashMap::new(),
            next_check: 0.0,
            next_trigger: 0.0,
            triggers: vec![],
            one_offs: HashMap::new(),
            rng: 0x1191_2007,
            muted: std::env::var("AC1_MUTE").is_ok_and(|v| v != "0"),
            volume_db: std::env::var("AC1_VOLUME").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0),
        }
    }
}

impl Sounds {
    /// 0..1 (an xorshift).
    fn frand(&mut self) -> f32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        (x >> 8) as f32 / (1u32 << 24) as f32
    }

    /// Every sound's master gain (linear): `AC1_VOLUME` and the settings' volume.
    fn master(&self, settings: Option<&crate::settings::Settings>) -> f32 {
        Volume::Decibels(self.volume_db).to_linear() * settings.map_or(1.0, |s| s.volume as f32 / 100.0)
    }
}

pub struct SoundPlugin;

impl Plugin for SoundPlugin {
    fn build(&self, app: &mut App) {
        app.add_audio_source::<PcmClip>()
            .init_resource::<Sounds>()
            .add_systems(Update, (toggle_mute, load_sounds, play_clip_events.after(crate::character::animate), placed_sounds, ambience, place_voices).chain());
    }
}

fn toggle_mute(mut commands: Commands, keys: Res<ButtonInput<KeyCode>>, mut s: ResMut<Sounds>, playing: Query<Entity, With<AudioPlayer<PcmClip>>>) {
    if !keys.just_pressed(KeyCode::KeyM) {
        return;
    }
    s.muted = !s.muted;
    info!("sound: {}", if s.muted { "muted" } else { "on" });
    if s.muted {
        for e in &playing {
            commands.entity(e).despawn();
        }
        s.cursors.clear();
        // (Unmuted, the zones he stands in start their sounds again.)
        s.triggers.iter_mut().for_each(|t| *t = Trigger::default());
        s.one_offs.clear();
    }
}

/// Start loading on threads once unmuted, and take what has loaded.
fn load_sounds(mut s: ResMut<Sounds>, dir: Res<crate::GameDir>) {
    if s.muted {
        return;
    }
    if s.data.is_none() && s.loading.is_none() {
        let dir = dir.0.clone();
        s.loading = Some(spawn_load(move || load(&dir)));
    }
    if let Some(slot) = s.loading.clone()
        && let Some(r) = take(&slot)
    {
        s.loading = None;
        match r {
            Ok(d) => {
                info!("sound: {} sound objects, Altaïr's contact table of {} surfaces", d.bank.len(), d.contacts.groups.len());
                s.data = Some(Arc::new(d));
                // (The city's sounds after the player's: they read every grid cell of the city, seconds.)
                if let Some(city) = s.city.clone() {
                    let dir = dir.0.clone();
                    s.city_loading = Some(spawn_load(move || load_city(&dir, &city)));
                }
            }
            Err(e) => {
                warn!("sound: not loaded: {e:#}");
                s.muted = true;
            }
        }
    }
    if let Some(slot) = s.city_loading.clone()
        && let Some(r) = take(&slot)
    {
        s.city_loading = None;
        match r {
            Ok(a) => {
                info!(
                    "sound: ambience of {} regions, {} sets; {} placed sounds; {} sound objects",
                    a.amb.regions.len(),
                    a.amb.sets.len(),
                    a.amb.emitters.len(),
                    a.bank.len()
                );
                s.triggers = vec![Trigger::default(); a.amb.emitters.len()];
                s.city_data = Some(Arc::new(a));
            }
            Err(e) => warn!("sound: no city sounds: {e:#}"),
        }
    }
}

/// The event keys a clip passed since last frame: (from, to], or from its start when it has just begun (`from`
/// `None`); a looping clip whose time went back wrapped round, (from, end] and [0, to]; any other went back to its
/// start, [0, to].
fn passed(events: &[AnimEvent], from: Option<f32>, to: f32, looping: bool) -> Vec<&AnimEvent> {
    match from {
        Some(f) if to >= f => events.iter().filter(|e| e.time > f && e.time <= to).collect(),
        Some(f) if looping => events.iter().filter(|e| e.time > f).chain(events.iter().filter(|e| e.time <= to)).collect(),
        _ => events.iter().filter(|e| e.time <= to).collect(),
    }
}

/// Where the slot's last clip was, on the clip now: the same clip, its time; the step before in a gait (both
/// `synced`), the same fraction of this one; else none (it has just begun).
fn from_time(prev: Option<&Cursor>, now: &Playing) -> Option<f32> {
    let prev = prev?;
    if Arc::ptr_eq(&prev.clip, &now.clip) {
        Some(prev.t)
    } else if prev.synced && now.synced {
        Some(prev.t / prev.clip.anim.duration.max(1e-3) * now.clip.anim.duration)
    } else {
        None
    }
}

/// The listener: the camera; else Altaïr.
fn listener(cams: &Query<&GlobalTransform, With<Camera3d>>, player: Option<Vec3>) -> Option<Listener> {
    if let Some(c) = cams.iter().next() {
        return Some(Listener { pos: c.translation(), right: c.right().as_vec3() });
    }
    player.map(|p| Listener { pos: p, right: Vec3::X })
}

/// A voice's samples (decoded once).
fn samples_for(s: &mut Sounds, bank: &Bank, v: &Voice) -> Option<Samples> {
    s.clips
        .entry(v.resource)
        .or_insert_with(|| match bank.decode(v) {
            Some(samples) => Some((v.channels, v.rate, samples.into())),
            None => {
                debug!("sound: resource {:08x} does not decode, not played", v.resource);
                None
            }
        })
        .clone()
}

/// Start a placed voice: its own clip asset (the samples shared) with its own pan, set for where it is now.
#[allow(clippy::too_many_arguments)]
fn spawn_placed(
    commands: &mut Commands,
    pcm: &mut Assets<PcmClip>,
    s: &mut Sounds,
    bank: &Bank,
    v: &Voice,
    at: Where,
    here: Vec3,
    ear: Option<Listener>,
    master: f32,
) -> Option<Entity> {
    let (channels, rate, samples) = samples_for(s, bank, v)?;
    let pan = Arc::new(Pan::default());
    let gain = Volume::Decibels(v.gain_db).to_linear();
    let fade = v.looped.then_some(false);
    let level = if v.looped { 0.0 } else { 1.0 };
    if let Some(ear) = ear {
        let (l, r) = ear.gains(here, &v.rolloff, gain * master * level);
        pan.set(l, r);
    }
    let clip = pcm.add(PcmClip { channels, rate, samples, pan: Some(pan.clone()), looped: v.looped });
    let play = if v.looped { PlaybackSettings::ONCE } else { PlaybackSettings::DESPAWN };
    Some(commands.spawn((AudioPlayer::<PcmClip>(clip), play.with_speed(v.speed), Placed { pan, at, rolloff: v.rolloff, gain, level, fade })).id())
}

/// What a clip key (or the take-off) starts: a contact type, a `SoundBao` object, an event by its BAO id.
enum Fired {
    Contact(u32),
    Audio(u32),
    Event(u32),
}

#[allow(clippy::too_many_arguments)]
fn play_clip_events(
    mut commands: Commands,
    mut s: ResMut<Sounds>,
    mut pcm: ResMut<Assets<PcmClip>>,
    time: Res<Time>,
    level: Res<Level>,
    settings: Option<Res<crate::settings::Settings>>,
    players: Query<(&Character, &Transform), With<Player>>,
    cams: Query<&GlobalTransform, With<Camera3d>>,
) {
    if s.muted {
        return;
    }
    let Some(data) = s.data.clone() else { return };
    let Ok((ch, root)) = players.single() else { return };
    let now = time.elapsed_secs();
    let mut fired: Vec<(String, f32, Fired)> = vec![];
    let slots: Vec<u8> = ch.playing.iter().map(|p| p.slot).collect();
    s.cursors.retain(|k, _| slots.contains(k));
    let jump_start = data.global.get(forge::sound::SOUND_JUMP_START).copied().flatten();
    for p in &ch.playing {
        let from = from_time(s.cursors.get(&p.slot), p);
        fired.extend(passed(&p.clip.anim.events, from, p.t, p.looping).into_iter().filter_map(|e| {
            let f = match e.kind {
                EventKind::Contact { ty } => Fired::Contact(ty),
                EventKind::Audio { sound } => Fired::Audio(sound),
                EventKind::Other(_) => return None,
            };
            Some((p.clip.name.clone(), e.time, f))
        }));
        if from.is_none() && p.clip.anim.events.is_empty() && TAKEOFFS.iter().any(|n| p.clip.name.contains(n)) && now - s.last_takeoff > 0.3 {
            s.last_takeoff = now;
            fired.push((p.clip.name.clone(), 0.0, jump_start.map_or(Fired::Contact(FT_JUMP_START), Fired::Event)));
        }
        s.cursors.insert(p.slot, Cursor { clip: p.clip.clone(), t: p.t, synced: p.synced });
    }
    if fired.is_empty() {
        return;
    }
    let material = level.sound_material_at(root.translation);
    let city = s.city.clone();
    let switch = |var: u32| player_switch(var, material, city.as_deref());
    let body = root.translation + Vec3::Y * BODY_HEIGHT;
    let ear = listener(&cams, Some(body));
    let master = s.master(settings.as_deref());
    for (clip, t, kind) in fired {
        let (label, event) = match kind {
            Fired::Contact(ty) => {
                (CONTACT_HUMAN.get(ty as usize).copied().unwrap_or("contact"), data.contacts.sound(material, ty).and_then(|o| data.bank.bao_of(o)))
            }
            Fired::Audio(sound) => ("audio", data.bank.bao_of(sound)),
            Fired::Event(e) => ("SoundJumpStart", Some(e)),
        };
        let Some(event) = event else { continue };
        let voices = data.bank.resolve(event, &switch, &mut s.picker);
        debug!("sound: {clip} t={t:.3} {label} -> event {event:08x}, material {material:?}, {} voice(s)", voices.len());
        for v in voices {
            spawn_placed(&mut commands, &mut pcm, &mut s, &data.bank, &v, Where::Player, body, ear, master);
        }
    }
}

/// Game space (Z up) to Bevy (Y up).
fn bevy_of(p: [f32; 3]) -> Vec3 {
    Vec3::new(p[0], p[2], -p[1])
}

/// Bevy (x, _, z) to the game's ground plane (x, y).
fn game_xy(p: Vec3) -> (f32, f32) {
    (p.x, -p.z)
}

/// Whether `p` (Bevy) is in an emitter's trigger zone; `slack` past its rim counts as in.
fn in_zone(e: &Emitter, p: Vec3, slack: f32) -> bool {
    let (centre, r) = e.zone.map_or((bevy_of(e.pos), ZONE_FALLBACK), |(c, r)| (bevy_of(c), r));
    p.distance(centre) < r + slack
}

/// The placed sounds' triggers: entering a zone starts its sounds (its stop events and the like are not plays), leaving
/// it fades its loops out.
#[allow(clippy::too_many_arguments)]
fn placed_sounds(
    mut commands: Commands,
    mut s: ResMut<Sounds>,
    mut pcm: ResMut<Assets<PcmClip>>,
    time: Res<Time>,
    level: Res<Level>,
    settings: Option<Res<crate::settings::Settings>>,
    players: Query<&Transform, With<Player>>,
    cams: Query<&GlobalTransform, With<Camera3d>>,
    mut placed: Query<&mut Placed>,
) {
    if s.muted {
        return;
    }
    s.next_trigger -= time.delta_secs();
    if s.next_trigger > 0.0 {
        return;
    }
    s.next_trigger = TRIGGER_CHECK;
    let Some(city) = s.city_data.clone() else { return };
    let Ok(root) = players.single() else { return };
    let p = root.translation + Vec3::Y * BODY_HEIGHT;
    let ear = listener(&cams, Some(p));
    let master = s.master(settings.as_deref());
    let material = level.sound_material_at(root.translation);
    let city_name = s.city.clone();
    for (i, e) in city.amb.emitters.iter().enumerate() {
        let Some(was) = s.triggers.get(i).map(|t| t.inside) else { break };
        let inside = in_zone(e, p, if was { ZONE_SLACK } else { 0.0 });
        if inside == was {
            continue;
        }
        if !inside {
            for l in std::mem::take(&mut s.triggers[i].loops) {
                if let Ok(mut pl) = placed.get_mut(l) {
                    pl.fade = Some(true);
                }
            }
            s.triggers[i].inside = false;
            continue;
        }
        s.triggers[i].inside = true;
        let here = bevy_of(e.pos);
        let switch = |var: u32| player_switch(var, (var == 0).then_some(material).flatten(), city_name.as_deref());
        for &ev in &e.events {
            if matches!(city.bank.event_type(ev), Some(2 | 3 | 4 | 10) | None) {
                continue;
            }
            for v in city.bank.resolve(ev, &switch, &mut s.picker) {
                if let Some(id) = spawn_placed(&mut commands, &mut pcm, &mut s, &city.bank, &v, Where::Point(here), here, ear, master)
                    && v.looped
                {
                    s.triggers[i].loops.push(id);
                }
            }
        }
        debug!("sound: entered {}'s zone at ({:.0}, {:.0}, {:.0})", e.name, e.pos[0], e.pos[1], e.pos[2]);
    }
}

/// The ambience sets to play at game-space (x, y). A region's area is not read (only the grid cell linking it), so: of
/// the wind sets (`A_Wind_*`) and of the others, each the ones linked from the finest cell over the point (a city's
/// own wind over the countryside's, a market over the whole quarter); the crowd murmur left out.
fn ambience_sets(amb: &Ambience, x: f32, y: f32) -> Vec<u32> {
    let named = |set: u32| amb.sets.get(&set).map_or(String::new(), |s| s.0.to_lowercase());
    let found: Vec<(u32, u32, bool)> = amb
        .sets_at(x, y)
        .into_iter()
        .filter(|&(set, _)| !named(set).contains("walla") && !named(set).contains("crowd"))
        .map(|(set, level)| (set, level, named(set).starts_with("a_wind")))
        .collect();
    let finest = |wind: bool| found.iter().filter(|f| f.2 == wind).map(|f| f.1).min();
    found.iter().filter(|f| Some(f.1) == finest(f.2)).map(|f| f.0).collect()
}

/// What an ambience loop plays: a streamed file (its rate) or a looped voice of the city's bank.
enum LoopKind {
    Stream(u32),
    Voice(Voice),
}

/// The ambience over the player: the loops of his regions' sets (started, decoded on a thread first if streamed;
/// faded in, and out once he has left their regions) and their one-off sounds, each at its own random interval.
#[allow(clippy::too_many_arguments)]
fn ambience(
    mut commands: Commands,
    mut s: ResMut<Sounds>,
    mut pcm: ResMut<Assets<PcmClip>>,
    time: Res<Time>,
    dir: Res<crate::GameDir>,
    settings: Option<Res<crate::settings::Settings>>,
    players: Query<&Transform, With<Player>>,
    cams: Query<&GlobalTransform, With<Camera3d>>,
    mut playing: Query<(Entity, &mut AmbientLoop, Option<&mut AudioSink>)>,
) {
    let dt = time.delta_secs();
    if s.muted {
        return;
    }
    let master = s.master(settings.as_deref());
    // Fades.
    for (e, mut l, sink) in &mut playing {
        l.level = if l.leaving { l.level - dt / AMBIENCE_FADE } else { (l.level + dt / AMBIENCE_FADE).min(1.0) };
        if l.leaving && l.level <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        if let Some(mut sink) = sink {
            sink.set_volume(Volume::Linear(Volume::Decibels(l.gain_db).to_linear() * master * l.level.max(0.0)));
        }
    }
    let Some(city) = s.city_data.clone() else { return };
    // Decoded loops: keep them.
    let done: Vec<u32> = s.decoding.iter().filter(|(_, (_, slot))| slot.lock().is_ok_and(|o| o.is_some())).map(|(k, _)| *k).collect();
    for stream in done {
        let Some((rate, slot)) = s.decoding.remove(&stream) else { continue };
        match take(&slot) {
            Some(Ok(samples)) => {
                let h = pcm.add(PcmClip { channels: 2, rate, samples: samples.into(), pan: None, looped: false });
                s.loops.insert(stream, h);
            }
            Some(Err(e)) => warn!("sound: ambience stream {stream:08x}: {e:#}"),
            None => {}
        }
    }
    s.next_check -= dt;
    if s.next_check > 0.0 {
        return;
    }
    s.next_check = AMBIENCE_CHECK;
    let Ok(root) = players.single() else { return };
    let (x, y) = game_xy(root.translation);
    let sets = ambience_sets(&city.amb, x, y);
    // The loops wanted: (key, kind, gain dB).
    let mut want: HashMap<u32, (LoopKind, f32)> = HashMap::new();
    let city_name = s.city.clone();
    let switch = |var: u32| player_switch(var, None, city_name.as_deref());
    for &set in &sets {
        let Some((_, events)) = city.amb.sets.get(&set) else { continue };
        for &e in events {
            if let Some(l) = city.bank.stream_loop(e) {
                want.entry(l.stream).or_insert((LoopKind::Stream(l.rate), l.gain_db));
            }
        }
        // (A set whose loop is no stream: its first sound, if it loops, plays from the bank, unplaced.)
        if let Some(&first) = events.first()
            && city.bank.stream_loop(first).is_none()
        {
            for v in city.bank.resolve(first, &switch, &mut s.picker).into_iter().filter(|v| v.looped) {
                let gain = v.gain_db;
                want.entry(v.resource).or_insert((LoopKind::Voice(v), gain));
            }
        }
    }
    for (_, mut l, _) in &mut playing {
        l.leaving = !want.contains_key(&l.key);
    }
    for (key, (kind, gain_db)) in want {
        if playing.iter().any(|(_, p, _)| p.key == key && !p.leaving) {
            continue;
        }
        if let (LoopKind::Voice(v), None) = (&kind, s.loops.get(&key)) {
            // (Decoded here: a small looped resource.)
            if let Some((ch, rate, samples)) = samples_for(&mut s, &city.bank, v) {
                let h = pcm.add(PcmClip { channels: ch, rate, samples, pan: None, looped: false });
                s.loops.insert(key, h);
            }
        }
        match (s.loops.get(&key), kind) {
            (Some(h), _) => {
                debug!("sound: ambience loop {key:08x} at ({x:.0}, {y:.0}), {gain_db:.0} dB");
                commands.spawn((
                    AudioPlayer::<PcmClip>(h.clone()),
                    PlaybackSettings::LOOP.with_volume(Volume::Linear(0.0)),
                    AmbientLoop { key, gain_db, level: 0.0, leaving: false },
                ));
            }
            (None, LoopKind::Stream(rate)) if !s.decoding.contains_key(&key) => {
                let d: PathBuf = dir.0.clone();
                s.decoding.insert(key, (rate, spawn_load(move || load_stream(&d, key))));
            }
            _ => {}
        }
    }
    // The sets' one-off sounds: each due at a random time in its interval, then again; placed round the listener.
    let now = time.elapsed_secs();
    s.one_offs.retain(|k, _| sets.contains(&k.0));
    let ear = listener(&cams, Some(root.translation));
    for &set in &sets {
        for (i, fx) in city.amb.random_fx.get(&set).into_iter().flatten().enumerate() {
            let next = match s.one_offs.get(&(set, i)) {
                Some(&t) => t,
                None => {
                    let t = now + fx.repeat.0 + s.frand() * (fx.repeat.1 - fx.repeat.0);
                    s.one_offs.insert((set, i), t);
                    continue;
                }
            };
            if now < next {
                continue;
            }
            let again = now + fx.repeat.0.max(0.5) + s.frand() * (fx.repeat.1 - fx.repeat.0);
            s.one_offs.insert((set, i), again);
            let Some(ear) = ear else { continue };
            let (angle, dist) = (s.frand() * std::f32::consts::TAU, fx.radius.0 + s.frand() * (fx.radius.1 - fx.radius.0));
            let here = ear.pos + Vec3::new(angle.cos() * dist, fx.height, angle.sin() * dist);
            for v in city.bank.resolve(fx.event, &switch, &mut s.picker).into_iter().filter(|v| !v.looped) {
                spawn_placed(&mut commands, &mut pcm, &mut s, &city.bank, &v, Where::Point(here), here, Some(ear), master);
            }
            debug!("sound: one-off {:08x} of set {set:08x} {dist:.0} m away", fx.event);
        }
    }
}

/// Each frame, every placed sound's gains from where it is against the listener; loops fade in and out (and are let go
/// once faded out).
fn place_voices(
    mut commands: Commands,
    s: Res<Sounds>,
    time: Res<Time>,
    settings: Option<Res<crate::settings::Settings>>,
    players: Query<&Transform, With<Player>>,
    cams: Query<&GlobalTransform, With<Camera3d>>,
    mut placed: Query<(Entity, &mut Placed)>,
) {
    let body = players.single().ok().map(|t| t.translation + Vec3::Y * BODY_HEIGHT);
    let Some(ear) = listener(&cams, body) else { return };
    let master = s.master(settings.as_deref());
    let dt = time.delta_secs();
    for (e, mut p) in &mut placed {
        match p.fade {
            Some(false) => p.level = (p.level + dt / PLACED_FADE).min(1.0),
            Some(true) => {
                p.level -= dt / PLACED_FADE;
                if p.level <= 0.0 {
                    commands.entity(e).despawn();
                    continue;
                }
            }
            None => {}
        }
        let here = match p.at {
            Where::Player => body.unwrap_or(ear.pos),
            Where::Point(v) => v,
        };
        let (l, r) = ear.gains(here, &p.rolloff, p.gain * master * p.level);
        p.pan.set(l, r);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(times: &[f32]) -> Vec<AnimEvent> {
        times.iter().map(|&time| AnimEvent { time, kind: EventKind::Contact { ty: 12 } }).collect()
    }

    #[test]
    fn ambience_takes_the_finest_wind_and_the_finest_rest() {
        let layout = forge::world::GridLayout { cell: 32.0, origin: [0.0, 0.0], width: 4, outside_radius: 0 };
        let named = |n: &str| (n.to_string(), vec![]);
        let amb = Ambience {
            layout,
            levels: 2,
            // cell 0: level 0 (0, 0); cell 1: level 0 (1, 0); cell 16: level 1 (0, 0), 64 m
            regions: vec![(0, 1), (16, 2), (16, 3), (1, 4), (1, 5)],
            sets: HashMap::from([
                (1, named("A_Wind_Damas_City")),
                (2, named("A_Wind_Damas_Country")),
                (3, named("A_City_Poor_Area")),
                (4, named("A_City_Market")),
                (5, named("snd_a_walla_x")),
            ]),
            random_fx: HashMap::new(),
            emitters: vec![],
        };
        let at = |x, y| {
            let mut v = ambience_sets(&amb, x, y);
            v.sort();
            v
        };
        assert_eq!(at(10.0, 10.0), vec![1, 3]);
        assert_eq!(at(40.0, 10.0), vec![2, 4], "the country's wind; the market over the quarter; no murmur");
        assert_eq!(at(200.0, 10.0), Vec::<u32>::new());
    }

    #[test]
    fn passed_keys_cover_the_frame_the_wrap_and_a_restart() {
        let k = keys(&[0.0, 0.2, 0.5]);
        let at = |from, to, looping| passed(&k, from, to, looping).iter().map(|e| e.time).collect::<Vec<_>>();
        assert_eq!(at(None, 0.1, false), vec![0.0], "a clip just begun fires its first key");
        assert_eq!(at(Some(0.1), 0.3, false), vec![0.2]);
        assert_eq!(at(Some(0.2), 0.3, false), Vec::<f32>::new(), "a key fires once");
        assert_eq!(at(Some(0.4), 0.1, true), vec![0.5, 0.0], "a loop's wrap");
        assert_eq!(at(Some(0.4), 0.1, false), vec![0.0], "a move played again starts over");
    }

    #[test]
    fn switches_follow_ac1s_callback_for_the_player() {
        assert_eq!(player_switch(0, Some(9), None), Some(9), "the surface");
        assert_eq!(player_switch(1, None, None), Some(0), "Altaïr, not the townsman default");
        assert_eq!(player_switch(3, None, None), Some(0), "weapon sheathed");
        assert_eq!(player_switch(7, None, Some("Damascus")), Some(4));
        assert_eq!(player_switch(7, None, Some("masyaf")), Some(8));
        assert_eq!(player_switch(7, None, Some("Acre")), Some(0));
        assert_eq!(player_switch(7, None, Some("test")), Some(10), "not in AC1's list: its length");
        assert_eq!(player_switch(18, None, None), None);
    }

    #[test]
    fn placed_sounds_pan_to_their_side_and_fade_with_distance() {
        let ear = Listener { pos: Vec3::ZERO, right: Vec3::X };
        let r = RollOff { saturation: 2.0, clamp: -1.0, fade_start: 2.0, fade_end: 15.0 };
        let (l, rr) = ear.gains(Vec3::new(1.0, 0.0, 0.0), &r, 1.0);
        assert!(rr > l && (l * l + rr * rr - 2.0).abs() < 1e-4, "to the right, the power kept: {l} {rr}");
        let (l, rr) = ear.gains(Vec3::new(0.0, 0.0, -1.0), &r, 1.0);
        assert!((l - 1.0).abs() < 1e-5 && (rr - 1.0).abs() < 1e-5, "ahead: both sides whole");
        let (l, _) = ear.gains(Vec3::new(0.0, 0.0, -8.5), &r, 1.0);
        assert!((l - 0.5).abs() < 1e-4, "half way down the fade: {l}");
        assert_eq!(ear.gains(Vec3::new(0.0, 0.0, -20.0), &r, 1.0), (0.0, 0.0), "past the fade's end");
    }

    #[test]
    fn a_placed_mono_clip_plays_in_stereo_through_its_pan_and_loops() {
        let pan = Arc::new(Pan::default());
        pan.set(0.5, 1.0);
        let clip = PcmClip { channels: 1, rate: 8000, samples: Arc::from([16384i16, -16384]), pan: Some(pan.clone()), looped: false };
        let out: Vec<f32> = clip.decoder().collect();
        assert_eq!(out, vec![0.25, 0.5, -0.25, -0.5]);
        let looped = PcmClip { looped: true, ..clip };
        assert_eq!(looped.decoder().take(6).count(), 6, "a loop goes on");
        assert_eq!(looped.decoder().total_duration(), None);
    }
}
