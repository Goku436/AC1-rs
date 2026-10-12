//! The player's sounds, as AC1 plays them: the event keys of the clips playing (`animation::Playing`) start sound
//! events from the install (`forge::sound`). A clip's `AudioEvent` names its sound (grunts, cloth, hands on stone); a
//! `ContactEvent` (a step, a landing, a roll) goes through Altaïr's contact table (`ContactTable_AlTair`) to the
//! surface's sounds. The events' switches pick by the surface's sound material (switch 0) where the level knows it
//! (`Level::sound_material_at`); the other switches play their default case (what sets them in AC1 is not known).
//! Decoded to PCM once per resource and played with Bevy's audio, at the listener (not placed in the world).
//!
//! One move has no keys in AC1's data: the free jump's take-off (`TAKEOFFS`); it plays the contact table's
//! `ft_jump_start` (picked here, as AC1's code-side sound for it is not traced).
//!
//! In a city, the ambience: the loops of the ambience regions linked from the grid cells over the player
//! (`forge::sound::Ambience`, picked by `ambience_sets`), faded in and out as he moves between them. The crowd murmur
//! sets are left out (no crowds talk here), and so are the sets' one-off sounds (gusts, birds: when AC1 plays them is
//! not known).
//!
//! The behaviour follows Banned445's AC1-Movement-Rewritten (MIT), which first played these sounds.
//!
//! M mutes and unmutes; `AC1_MUTE=1` starts muted (nothing is loaded until unmuted), `AC1_VOLUME=dB` turns every sound
//! up or down. Headless runs have no audio at all (`main` leaves the plugin out).

use crate::animation::Playing;
use crate::character::{Character, Player};
use crate::level::Level;
use bevy::audio::{AddAudioSource, AudioSinkPlayback, ChannelCount, Decodable, PlaybackSettings, SampleRate, Source, Volume};
use bevy::prelude::*;
use forge::anim::{AnimEvent, CONTACT_HUMAN, EventKind};
use forge::sound::{Ambience, Bank, ContactTable, Picker, Voice};
use std::collections::HashMap;
use std::num::NonZero;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Decoded samples, interleaved.
#[derive(Asset, TypePath, Clone)]
pub struct PcmClip {
    channels: u16,
    rate: u32,
    samples: Arc<[i16]>,
}

pub struct PcmDecoder {
    clip: PcmClip,
    at: usize,
}

impl Iterator for PcmDecoder {
    type Item = bevy::audio::Sample;
    fn next(&mut self) -> Option<Self::Item> {
        let s = *self.clip.samples.get(self.at)?;
        self.at += 1;
        Some(s as f32 / 32768.0)
    }
}

impl Source for PcmDecoder {
    fn current_span_len(&self) -> Option<usize> {
        Some(self.clip.samples.len() - self.at)
    }
    fn channels(&self) -> ChannelCount {
        NonZero::new(self.clip.channels.max(1)).unwrap_or(NonZero::<u16>::MIN)
    }
    fn sample_rate(&self) -> SampleRate {
        NonZero::new(self.clip.rate.max(1)).unwrap_or(NonZero::<u32>::MIN)
    }
    fn total_duration(&self) -> Option<Duration> {
        let frames = self.clip.samples.len() / self.clip.channels.max(1) as usize;
        Some(Duration::from_secs_f64(frames as f64 / self.clip.rate.max(1) as f64))
    }
}

impl Decodable for PcmClip {
    type Decoder = PcmDecoder;
    fn decoder(&self) -> PcmDecoder {
        PcmDecoder { clip: self.clone(), at: 0 }
    }
}

/// What the player's sounds are made from: the install's sound objects and Altaïr's contact table.
struct SoundData {
    bank: Bank,
    contacts: ContactTable,
}

/// The data files of `DataPC.forge` that hold the player's sounds.
const SOUND_FILES: [&str; 3] = ["Game Fix", "Game Bootstrap Settings", "Rank 9"];

fn load(game_dir: &Path) -> anyhow::Result<SoundData> {
    let forge = forge::Forge::open(game_dir.join("DataPC.forge"))?;
    let mut bank = Bank::default();
    let mut contacts = None;
    for name in SOUND_FILES {
        let Some(e) = forge.entries.iter().find(|e| e.name == name) else { anyhow::bail!("no data file {name}") };
        let data = forge.read(e)?;
        let bad = bank.add_data_file(&data)?;
        if bad > 0 {
            warn!("sound: {bad} sound objects of {name} do not parse");
        }
        if let Some(o) = forge::parse_objects(&data)?.iter().find(|o| o.class == forge::sound::CLASS_CONTACT_TABLE && o.name == "ContactTable_AlTair") {
            contacts = Some(forge::sound::parse_contact_table(o.body)?);
        }
    }
    let contacts = contacts.ok_or_else(|| anyhow::anyhow!("no ContactTable_AlTair"))?;
    Ok(SoundData { bank, contacts })
}

/// A city's ambience and the sound objects its sets play.
struct AmbienceData {
    bank: Bank,
    amb: Ambience,
}

fn load_ambience(game_dir: &Path, city: &str) -> anyhow::Result<AmbienceData> {
    let forge = forge::Forge::open(game_dir.join(format!("DataPC_{city}.forge")))?;
    let mut bank = Bank::default();
    let amb = forge::sound::load_ambience(&forge, city, &mut bank)?;
    Ok(AmbienceData { bank, amb })
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

/// Clips whose take-off AC1's data gives no key: they play `ft_jump_start` as they begin (see the module header).
const TAKEOFFS: [&str; 2] = ["_to_jumpstraight_clear_", "xx_h_jumpstraight_clear_foot"];
const FT_JUMP_START: u32 = 1;

/// An ambience loop fades in and out over this (s).
const AMBIENCE_FADE: f32 = 2.0;
/// How often (s) the player's ambience regions are looked up.
const AMBIENCE_CHECK: f32 = 0.5;

/// A playing ambience loop: its stream, its gain (dB) and how far faded in (0..1); `leaving` fades it out.
#[derive(Component)]
struct AmbientLoop {
    stream: u32,
    gain_db: f32,
    level: f32,
    leaving: bool,
}

#[derive(Resource)]
pub struct Sounds {
    data: Option<Arc<SoundData>>,
    loading: Option<Slot<SoundData>>,
    picker: Picker,
    /// Decoded resources by BAO id (`None`: not IMA-ADPCM, not played).
    clips: HashMap<u32, Option<Handle<PcmClip>>>,
    cursors: HashMap<u8, Cursor>,
    /// When the last take-off played (game time, s).
    last_takeoff: f32,
    /// The city's ambience (`AC1_LEVEL`), loading or loaded, and its loops: decoded, or decoding.
    city: Option<String>,
    ambience: Option<Arc<AmbienceData>>,
    ambience_loading: Option<Slot<AmbienceData>>,
    loops: HashMap<u32, Handle<PcmClip>>,
    decoding: HashMap<u32, (u32, Slot<Vec<i16>>)>,
    next_check: f32,
    pub muted: bool,
    /// Added to every sound's gain (`AC1_VOLUME`).
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
            ambience: None,
            ambience_loading: None,
            loops: HashMap::new(),
            decoding: HashMap::new(),
            next_check: 0.0,
            muted: std::env::var("AC1_MUTE").is_ok_and(|v| v != "0"),
            volume_db: std::env::var("AC1_VOLUME").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0),
        }
    }
}

pub struct SoundPlugin;

impl Plugin for SoundPlugin {
    fn build(&self, app: &mut App) {
        app.add_audio_source::<PcmClip>()
            .init_resource::<Sounds>()
            .add_systems(Update, (toggle_mute, load_sounds, play_clip_events.after(crate::character::animate), ambience).chain());
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
                // (The city's ambience after the player's sounds: it reads every grid cell of the city, seconds.)
                if let Some(city) = s.city.clone() {
                    let dir = dir.0.clone();
                    s.ambience_loading = Some(spawn_load(move || load_ambience(&dir, &city)));
                }
            }
            Err(e) => {
                warn!("sound: not loaded: {e:#}");
                s.muted = true;
            }
        }
    }
    if let Some(slot) = s.ambience_loading.clone()
        && let Some(r) = take(&slot)
    {
        s.ambience_loading = None;
        match r {
            Ok(a) => {
                info!("sound: ambience of {} regions, {} sets", a.amb.regions.len(), a.amb.sets.len());
                s.ambience = Some(Arc::new(a));
            }
            Err(e) => warn!("sound: no ambience: {e:#}"),
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

fn play_clip_events(
    mut commands: Commands,
    mut s: ResMut<Sounds>,
    mut pcm: ResMut<Assets<PcmClip>>,
    time: Res<Time>,
    level: Res<Level>,
    players: Query<(&Character, &Transform), With<Player>>,
) {
    if s.muted {
        return;
    }
    let Some(data) = s.data.clone() else { return };
    let Ok((ch, root)) = players.single() else { return };
    let now = time.elapsed_secs();
    let mut fired: Vec<(String, f32, EventKind)> = vec![];
    let slots: Vec<u8> = ch.playing.iter().map(|p| p.slot).collect();
    s.cursors.retain(|k, _| slots.contains(k));
    for p in &ch.playing {
        let from = from_time(s.cursors.get(&p.slot), p);
        fired.extend(passed(&p.clip.anim.events, from, p.t, p.looping).into_iter().map(|e| (p.clip.name.clone(), e.time, e.kind.clone())));
        if from.is_none() && p.clip.anim.events.is_empty() && TAKEOFFS.iter().any(|n| p.clip.name.contains(n)) && now - s.last_takeoff > 0.3 {
            s.last_takeoff = now;
            fired.push((p.clip.name.clone(), 0.0, EventKind::Contact { ty: FT_JUMP_START }));
        }
        s.cursors.insert(p.slot, Cursor { clip: p.clip.clone(), t: p.t, synced: p.synced });
    }
    if fired.is_empty() {
        return;
    }
    let material = level.sound_material_at(root.translation);
    let switch = |var: u32| (var == 0).then_some(material).flatten();
    for (clip, t, kind) in fired {
        let (label, sound) = match kind {
            EventKind::Contact { ty } => (CONTACT_HUMAN.get(ty as usize).copied().unwrap_or("contact"), data.contacts.sound(material, ty)),
            EventKind::Audio { sound } => ("audio", Some(sound)),
            EventKind::Other(_) => continue,
        };
        let Some(event) = sound.and_then(|o| data.bank.bao_of(o)) else { continue };
        let voices = data.bank.resolve(event, &switch, &mut s.picker);
        debug!("sound: {clip} t={t:.3} {label} -> event {event:08x}, material {material:?}, {} voice(s)", voices.len());
        for v in voices {
            let Some(handle) = clip_for(&mut s, &data.bank, &mut pcm, &v) else { continue };
            let gain = v.gain_db + s.volume_db;
            commands.spawn((AudioPlayer::<PcmClip>(handle), PlaybackSettings::DESPAWN.with_volume(Volume::Decibels(gain)).with_speed(v.speed)));
        }
    }
}

/// A voice's decoded resource (decoded once).
fn clip_for(s: &mut Sounds, bank: &Bank, pcm: &mut Assets<PcmClip>, v: &Voice) -> Option<Handle<PcmClip>> {
    s.clips
        .entry(v.resource)
        .or_insert_with(|| match bank.decode(v) {
            Some(samples) => Some(pcm.add(PcmClip { channels: v.channels, rate: v.rate, samples: samples.into() })),
            None => {
                debug!("sound: resource {:08x} not IMA-ADPCM, not played", v.resource);
                None
            }
        })
        .clone()
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

/// Bevy (x, _, z) to the game's ground plane (x, y).
fn game_xy(p: Vec3) -> (f32, f32) {
    (p.x, -p.z)
}

/// The ambience loops over the player: started (decoded on a thread first), faded in, and faded out once he has left
/// their regions.
#[allow(clippy::too_many_arguments)]
fn ambience(
    mut commands: Commands,
    mut s: ResMut<Sounds>,
    mut pcm: ResMut<Assets<PcmClip>>,
    time: Res<Time>,
    dir: Res<crate::GameDir>,
    players: Query<&Transform, With<Player>>,
    mut playing: Query<(Entity, &mut AmbientLoop, Option<&mut AudioSink>)>,
) {
    let dt = time.delta_secs();
    if s.muted {
        return;
    }
    // Fades.
    for (e, mut l, sink) in &mut playing {
        l.level = if l.leaving { l.level - dt / AMBIENCE_FADE } else { (l.level + dt / AMBIENCE_FADE).min(1.0) };
        if l.leaving && l.level <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        if let Some(mut sink) = sink {
            sink.set_volume(Volume::Linear(Volume::Decibels(l.gain_db + s.volume_db).to_linear() * l.level.max(0.0)));
        }
    }
    let Some(amb) = s.ambience.clone() else { return };
    // Decoded loops: start them.
    let done: Vec<u32> = s.decoding.iter().filter(|(_, (_, slot))| slot.lock().is_ok_and(|o| o.is_some())).map(|(k, _)| *k).collect();
    for stream in done {
        let Some((rate, slot)) = s.decoding.remove(&stream) else { continue };
        match take(&slot) {
            Some(Ok(samples)) => {
                let h = pcm.add(PcmClip { channels: 2, rate, samples: samples.into() });
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
    let mut want: HashMap<u32, forge::sound::StreamLoop> = HashMap::new();
    for set in ambience_sets(&amb.amb, x, y) {
        let Some((_, events)) = amb.amb.sets.get(&set) else { continue };
        for l in events.iter().filter_map(|&e| amb.bank.stream_loop(e)) {
            want.entry(l.stream).or_insert(l);
        }
    }
    for (_, mut l, _) in &mut playing {
        l.leaving = !want.contains_key(&l.stream);
    }
    for (stream, l) in want {
        if playing.iter().any(|(_, p, _)| p.stream == stream && !p.leaving) {
            continue;
        }
        match s.loops.get(&stream) {
            Some(h) => {
                debug!("sound: ambience loop {stream:08x} at ({x:.0}, {y:.0}), {:.0} dB", l.gain_db);
                commands.spawn((
                    AudioPlayer::<PcmClip>(h.clone()),
                    PlaybackSettings::LOOP.with_volume(Volume::Linear(0.0)),
                    AmbientLoop { stream, gain_db: l.gain_db, level: 0.0, leaving: false },
                ));
            }
            None if !s.decoding.contains_key(&stream) => {
                let d: PathBuf = dir.0.clone();
                s.decoding.insert(stream, (l.rate, spawn_load(move || load_stream(&d, stream))));
            }
            None => {}
        }
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
}
