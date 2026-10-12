//! The player's sounds, as AC1 plays them: the event keys of the clips playing (`animation::Playing`) start sound
//! events from the install (`forge::sound`). A clip's `AudioEvent` names its sound (grunts, cloth, hands on stone); a
//! `ContactEvent` (a step, a landing, a roll) goes through Altaïr's contact table (`ContactTable_AlTair`) to the
//! surface's sounds. The events' switches pick by the surface's sound material (switch 0) where the level knows it
//! (`Level::sound_material_at`); the other switches play their default case (what sets them in AC1 is not known).
//! Decoded to PCM once per resource and played with Bevy's audio, at the listener (not placed in the world).
//!
//! The behaviour follows Banned445's AC1-Movement-Rewritten (MIT), which first played these sounds.
//!
//! M mutes and unmutes; `AC1_MUTE=1` starts muted (nothing is loaded until unmuted), `AC1_VOLUME=dB` turns every sound
//! up or down. Headless runs have no audio at all (`main` leaves the plugin out).

use crate::animation::Playing;
use crate::character::{Character, Player};
use bevy::audio::{AddAudioSource, ChannelCount, Decodable, PlaybackSettings, SampleRate, Source, Volume};
use bevy::prelude::*;
use forge::anim::{AnimEvent, CONTACT_HUMAN, EventKind};
use forge::sound::{Bank, ContactTable, Picker, Voice};
use std::collections::HashMap;
use std::num::NonZero;
use std::path::Path;
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

/// What the sounds are made from: the install's sound objects and Altaïr's contact table.
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

type Loading = Arc<Mutex<Option<anyhow::Result<SoundData>>>>;

/// Where a slot's clip was last frame (see `passed`).
struct Cursor {
    clip: Arc<crate::animation::Clip>,
    t: f32,
    synced: bool,
}

#[derive(Resource)]
pub struct Sounds {
    data: Option<Arc<SoundData>>,
    loading: Option<Loading>,
    picker: Picker,
    /// Decoded resources by BAO id (`None`: not IMA-ADPCM, not played).
    clips: HashMap<u32, Option<Handle<PcmClip>>>,
    cursors: HashMap<u8, Cursor>,
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
            muted: std::env::var("AC1_MUTE").is_ok_and(|v| v != "0"),
            volume_db: std::env::var("AC1_VOLUME").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0),
        }
    }
}

pub struct SoundPlugin;

impl Plugin for SoundPlugin {
    fn build(&self, app: &mut App) {
        app.add_audio_source::<PcmClip>().init_resource::<Sounds>().add_systems(Update, (toggle_mute, play_clip_events.after(crate::character::animate)));
    }
}

fn start_loading(s: &mut Sounds, dir: &Path) {
    if s.data.is_some() || s.loading.is_some() {
        return;
    }
    let slot: Loading = Arc::new(Mutex::new(None));
    let out = slot.clone();
    let dir = dir.to_path_buf();
    std::thread::spawn(move || {
        let r = load(&dir);
        if let Ok(mut o) = out.lock() {
            *o = Some(r);
        }
    });
    s.loading = Some(slot);
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
    dir: Res<crate::GameDir>,
    players: Query<(&Character, &Transform), With<Player>>,
) {
    if s.muted {
        s.cursors.clear();
        return;
    }
    start_loading(&mut s, &dir.0);
    if let Some(slot) = s.loading.clone() {
        let done = slot.lock().ok().and_then(|mut o| o.take());
        match done {
            Some(Ok(d)) => {
                info!("sound: {} sound objects, Altaïr's contact table of {} surfaces", d.bank.len(), d.contacts.groups.len());
                s.data = Some(Arc::new(d));
                s.loading = None;
            }
            Some(Err(e)) => {
                warn!("sound: not loaded: {e:#}");
                s.loading = None;
                s.muted = true;
            }
            None => {}
        }
    }
    let Some(data) = s.data.clone() else { return };
    let Ok((ch, _root)) = players.single() else { return };
    let material: Option<u32> = None;
    let switch = |var: u32| (var == 0).then_some(material).flatten();
    let mut fired: Vec<(String, f32, String, u32)> = vec![];
    let slots: Vec<u8> = ch.playing.iter().map(|p| p.slot).collect();
    s.cursors.retain(|k, _| slots.contains(k));
    for p in &ch.playing {
        let from = from_time(s.cursors.get(&p.slot), p);
        for e in passed(&p.clip.anim.events, from, p.t, p.looping) {
            let (label, sound) = match e.kind {
                EventKind::Contact { ty } => (CONTACT_HUMAN.get(ty as usize).copied().unwrap_or("contact").to_string(), data.contacts.sound(material, ty)),
                EventKind::Audio { sound } => ("audio".to_string(), Some(sound)),
                EventKind::Other(_) => continue,
            };
            if let Some(event) = sound.and_then(|o| data.bank.bao_of(o)) {
                fired.push((p.clip.name.clone(), e.time, label, event));
            }
        }
        s.cursors.insert(p.slot, Cursor { clip: p.clip.clone(), t: p.t, synced: p.synced });
    }
    for (clip, t, label, event) in fired {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(times: &[f32]) -> Vec<AnimEvent> {
        times.iter().map(|&time| AnimEvent { time, kind: EventKind::Contact { ty: 12 } }).collect()
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
